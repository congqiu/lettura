mod common;

use lettura::models::fetch_job::{self, FetchJobStatus};

#[tokio::test]
async fn enqueue_inserts_pending_job() {
    let app = common::TestApp::new().await;
    let user_id = app.create_user("alice").await;
    let entry_id = app.create_entry(user_id, "https://example.com/a").await;

    let id = fetch_job::enqueue(&app.pool, entry_id, user_id, "https://example.com/a", 0)
        .await
        .unwrap();

    let row = fetch_job::find_by_id(&app.pool, id).await.unwrap().unwrap();
    assert_eq!(row.status, FetchJobStatus::Pending);
    assert_eq!(row.attempts, 0);
    assert_eq!(row.priority, 0);
    assert_eq!(row.max_attempts, 5);
    assert!(row.refetch_requested_at.is_none());

    app.cleanup().await;
}

#[tokio::test]
async fn enqueue_same_entry_pending_upserts_max_priority() {
    let app = common::TestApp::new().await;
    let user_id = app.create_user("bob").await;
    let entry_id = app.create_entry(user_id, "https://example.com/b").await;

    let id1 = fetch_job::enqueue(&app.pool, entry_id, user_id, "https://example.com/b", 0)
        .await
        .unwrap();
    let id2 = fetch_job::enqueue(&app.pool, entry_id, user_id, "https://example.com/b", 10)
        .await
        .unwrap();

    assert_eq!(id1, id2, "ON CONFLICT should target same row");
    let row = fetch_job::find_by_id(&app.pool, id1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.priority, 10);
    assert!(
        row.refetch_requested_at.is_none(),
        "refetch_requested_at only set when conflicting status='running'"
    );

    app.cleanup().await;
}

#[tokio::test]
async fn enqueue_against_running_sets_refetch_signal() {
    let app = common::TestApp::new().await;
    let user_id = app.create_user("refetch").await;
    let entry_id = app.create_entry(user_id, "https://example.com/r").await;

    let id = fetch_job::enqueue(&app.pool, entry_id, user_id, "https://example.com/r", 0)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE fetch_jobs SET status='running', leased_until=NOW() + INTERVAL '5 minutes', \
         leased_by='worker-x', attempts=1 WHERE id=$1",
    )
    .bind(id)
    .execute(&app.pool)
    .await
    .unwrap();

    let id2 = fetch_job::enqueue(&app.pool, entry_id, user_id, "https://example.com/r", 10)
        .await
        .unwrap();
    assert_eq!(id, id2);

    let row = fetch_job::find_by_id(&app.pool, id).await.unwrap().unwrap();
    assert_eq!(row.status, FetchJobStatus::Running, "running not disturbed");
    assert_eq!(row.priority, 10);
    assert!(
        row.refetch_requested_at.is_some(),
        "refetch signal recorded"
    );

    app.cleanup().await;
}

#[tokio::test]
async fn enqueue_does_not_block_against_dead_row() {
    let app = common::TestApp::new().await;
    let user_id = app.create_user("dead").await;
    let entry_id = app.create_entry(user_id, "https://example.com/d").await;

    let id = fetch_job::enqueue(&app.pool, entry_id, user_id, "https://example.com/d", 0)
        .await
        .unwrap();
    sqlx::query("UPDATE fetch_jobs SET status='dead' WHERE id=$1")
        .bind(id)
        .execute(&app.pool)
        .await
        .unwrap();

    let id2 = fetch_job::enqueue(&app.pool, entry_id, user_id, "https://example.com/d", 0)
        .await
        .unwrap();
    assert_ne!(id, id2, "new pending row coexists with dead row");

    let counts: Vec<(FetchJobStatus, i64)> = sqlx::query_as(
        "SELECT status, COUNT(*) FROM fetch_jobs WHERE entry_id=$1 \
         GROUP BY status ORDER BY status",
    )
    .bind(entry_id)
    .fetch_all(&app.pool)
    .await
    .unwrap();
    assert_eq!(counts.len(), 2);

    app.cleanup().await;
}

#[tokio::test]
async fn dequeue_skip_locked_no_double_consumption() {
    let app = common::TestApp::new().await;
    let user_id = app.create_user("scaler").await;

    for i in 0..100 {
        let url = format!("https://x.test/{i}");
        let eid = app.create_entry(user_id, &url).await;
        fetch_job::enqueue(&app.pool, eid, user_id, &url, 0)
            .await
            .unwrap();
    }

    let mut handles = vec![];
    for w in 0..5 {
        let p = app.pool.clone();
        handles.push(tokio::spawn(async move {
            let worker_id = format!("worker-{w}");
            let mut consumed = vec![];
            while let Some(job) = fetch_job::dequeue_one(&p, &worker_id).await.unwrap() {
                consumed.push(job.id);
            }
            consumed
        }));
    }

    let mut all_ids = vec![];
    for h in handles {
        all_ids.extend(h.await.unwrap());
    }

    assert_eq!(all_ids.len(), 100);
    let unique: std::collections::HashSet<_> = all_ids.iter().collect();
    assert_eq!(unique.len(), 100, "no duplicates across workers");

    app.cleanup().await;
}

#[tokio::test]
async fn dequeue_skips_jobs_with_future_run_after() {
    let app = common::TestApp::new().await;
    let user_id = app.create_user("schedule").await;
    let entry_id = app.create_entry(user_id, "https://x.test/").await;
    let id = fetch_job::enqueue(&app.pool, entry_id, user_id, "https://x.test/", 0)
        .await
        .unwrap();

    sqlx::query("UPDATE fetch_jobs SET run_after = NOW() + INTERVAL '1 hour' WHERE id = $1")
        .bind(id)
        .execute(&app.pool)
        .await
        .unwrap();

    assert!(
        fetch_job::dequeue_one(&app.pool, "worker-1")
            .await
            .unwrap()
            .is_none()
    );
    app.cleanup().await;
}

#[tokio::test]
async fn dequeue_orders_by_priority_then_run_after() {
    let app = common::TestApp::new().await;
    let user_id = app.create_user("prio").await;

    // Older low-priority job
    let e1 = app.create_entry(user_id, "https://x.test/1").await;
    let id1 = fetch_job::enqueue(&app.pool, e1, user_id, "https://x.test/1", 0)
        .await
        .unwrap();
    sqlx::query("UPDATE fetch_jobs SET created_at = NOW() - INTERVAL '10 minutes' WHERE id=$1")
        .bind(id1)
        .execute(&app.pool)
        .await
        .unwrap();

    // Newer high-priority job
    let e2 = app.create_entry(user_id, "https://x.test/2").await;
    let id2 = fetch_job::enqueue(&app.pool, e2, user_id, "https://x.test/2", 10)
        .await
        .unwrap();

    let picked = fetch_job::dequeue_one(&app.pool, "w")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(picked.id, id2, "higher priority dequeued first");

    app.cleanup().await;
}

/// `attempts` is SMALLINT (i16, max 32767). A pathological retry storm —
/// repeated lease takeovers compounding with admin-driven retries — could
/// theoretically push attempts past the ceiling and trigger a Postgres
/// `numeric_value_out_of_range`. `dequeue_one` clamps with
/// `LEAST(attempts + 1, max_attempts)` so the increment becomes a no-op
/// once the cap is reached.
#[tokio::test]
async fn dequeue_attempts_capped_at_max() {
    let app = common::TestApp::new().await;
    let user_id = app.create_user("ovf").await;
    let entry_id = app.create_entry(user_id, "https://x.test/cap").await;
    let id = fetch_job::enqueue(&app.pool, entry_id, user_id, "https://x.test/cap", 0)
        .await
        .unwrap();

    // Force attempts up to max_attempts (default 5) and flip back to failed
    // so the next dequeue is eligible — exercising the +1 path that would
    // otherwise overflow.
    sqlx::query(
        "UPDATE fetch_jobs SET attempts = max_attempts, status = 'failed', \
         run_after = NOW(), leased_until = NULL, leased_by = NULL WHERE id = $1",
    )
    .bind(id)
    .execute(&app.pool)
    .await
    .unwrap();

    let job = fetch_job::dequeue_one(&app.pool, "w-cap")
        .await
        .unwrap()
        .expect("job should dequeue");
    assert_eq!(
        job.attempts, job.max_attempts,
        "attempts should be clamped to max_attempts, not incremented past it"
    );

    app.cleanup().await;
}

#[tokio::test]
async fn complete_without_refetch_deletes_row() {
    let app = common::TestApp::new().await;
    let user_id = app.create_user("c1").await;
    let entry_id = app.create_entry(user_id, "https://x.test/").await;
    let id = fetch_job::enqueue(&app.pool, entry_id, user_id, "https://x.test/", 0)
        .await
        .unwrap();
    let _ = fetch_job::dequeue_one(&app.pool, "w-1")
        .await
        .unwrap()
        .unwrap();

    fetch_job::complete(&app.pool, id, "w-1").await.unwrap();

    assert!(
        fetch_job::find_by_id(&app.pool, id)
            .await
            .unwrap()
            .is_none()
    );
    app.cleanup().await;
}

#[tokio::test]
async fn complete_with_refetch_signal_resets_to_pending_preserving_priority() {
    let app = common::TestApp::new().await;
    let user_id = app.create_user("c2").await;
    let entry_id = app.create_entry(user_id, "https://x.test/").await;
    let id = fetch_job::enqueue(&app.pool, entry_id, user_id, "https://x.test/", 0)
        .await
        .unwrap();
    let _ = fetch_job::dequeue_one(&app.pool, "w-1").await.unwrap();
    // Refetch arrives during processing.
    fetch_job::enqueue(&app.pool, entry_id, user_id, "https://x.test/", 10)
        .await
        .unwrap();

    fetch_job::complete(&app.pool, id, "w-1").await.unwrap();

    let row = fetch_job::find_by_id(&app.pool, id).await.unwrap().unwrap();
    assert_eq!(row.status, FetchJobStatus::Pending);
    assert_eq!(row.attempts, 0);
    assert_eq!(row.priority, 10, "priority preserved for re-dispatch");
    assert!(
        row.refetch_requested_at.is_none(),
        "signal cleared after honoring"
    );
    app.cleanup().await;
}

#[tokio::test]
async fn complete_rejects_mismatched_worker_id() {
    let app = common::TestApp::new().await;
    let user_id = app.create_user("c3").await;
    let entry_id = app.create_entry(user_id, "https://x.test/").await;
    let id = fetch_job::enqueue(&app.pool, entry_id, user_id, "https://x.test/", 0)
        .await
        .unwrap();
    let _ = fetch_job::dequeue_one(&app.pool, "w-1").await.unwrap();

    fetch_job::complete(&app.pool, id, "w-OTHER").await.unwrap();

    let row = fetch_job::find_by_id(&app.pool, id).await.unwrap().unwrap();
    assert_eq!(
        row.status,
        FetchJobStatus::Running,
        "complete by wrong worker is no-op"
    );
    app.cleanup().await;
}

#[tokio::test]
async fn fail_under_max_uses_60s_min_backoff() {
    let app = common::TestApp::new().await;
    let user_id = app.create_user("f1").await;
    let entry_id = app.create_entry(user_id, "https://x.test/").await;
    let id = fetch_job::enqueue(&app.pool, entry_id, user_id, "https://x.test/", 0)
        .await
        .unwrap();
    let job = fetch_job::dequeue_one(&app.pool, "w-1")
        .await
        .unwrap()
        .unwrap();

    let before = chrono::Utc::now();
    fetch_job::fail(&app.pool, id, "w-1", "boom", job.attempts, job.max_attempts)
        .await
        .unwrap();

    let row = fetch_job::find_by_id(&app.pool, id).await.unwrap().unwrap();
    assert_eq!(row.status, FetchJobStatus::Failed);
    assert_eq!(row.last_error.as_deref(), Some("boom"));
    let backoff = (row.run_after - before).num_seconds();
    assert!(
        backoff >= 58 && backoff <= 62,
        "first failure ~60s, got {backoff}s"
    );
    app.cleanup().await;
}

#[tokio::test]
async fn fail_at_max_attempts_promotes_to_dead() {
    let app = common::TestApp::new().await;
    let user_id = app.create_user("f2").await;
    let entry_id = app.create_entry(user_id, "https://x.test/").await;
    let id = fetch_job::enqueue(&app.pool, entry_id, user_id, "https://x.test/", 0)
        .await
        .unwrap();

    for _ in 0..5 {
        sqlx::query("UPDATE fetch_jobs SET run_after = NOW() WHERE id = $1")
            .bind(id)
            .execute(&app.pool)
            .await
            .unwrap();
        let job = fetch_job::dequeue_one(&app.pool, "w-1")
            .await
            .unwrap()
            .unwrap();
        fetch_job::fail(&app.pool, id, "w-1", "boom", job.attempts, job.max_attempts)
            .await
            .unwrap();
    }

    let row = fetch_job::find_by_id(&app.pool, id).await.unwrap().unwrap();
    assert_eq!(row.status, FetchJobStatus::Dead);
    app.cleanup().await;
}

#[tokio::test]
async fn release_restores_to_pending_without_consuming_attempt() {
    let app = common::TestApp::new().await;
    let user_id = app.create_user("r1").await;
    let entry_id = app.create_entry(user_id, "https://x.test/").await;
    let id = fetch_job::enqueue(&app.pool, entry_id, user_id, "https://x.test/", 0)
        .await
        .unwrap();
    let job = fetch_job::dequeue_one(&app.pool, "w-1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(job.attempts, 1);

    fetch_job::release(&app.pool, id, "w-1").await.unwrap();

    let row = fetch_job::find_by_id(&app.pool, id).await.unwrap().unwrap();
    assert_eq!(row.status, FetchJobStatus::Pending);
    assert_eq!(row.attempts, 0);
    assert!(row.leased_until.is_none());
    app.cleanup().await;
}

#[tokio::test]
async fn release_rejects_mismatched_worker_id() {
    let app = common::TestApp::new().await;
    let user_id = app.create_user("r2").await;
    let entry_id = app.create_entry(user_id, "https://x.test/").await;
    let id = fetch_job::enqueue(&app.pool, entry_id, user_id, "https://x.test/", 0)
        .await
        .unwrap();
    let _ = fetch_job::dequeue_one(&app.pool, "w-1").await.unwrap();

    fetch_job::release(&app.pool, id, "w-WRONG").await.unwrap();

    let row = fetch_job::find_by_id(&app.pool, id).await.unwrap().unwrap();
    assert_eq!(
        row.status,
        FetchJobStatus::Running,
        "release by wrong worker is no-op"
    );
    app.cleanup().await;
}

#[tokio::test]
async fn lease_expired_job_taken_over_by_another_worker() {
    let app = common::TestApp::new().await;
    let user_id = app.create_user("lease").await;
    let entry_id = app.create_entry(user_id, "https://x.test/").await;
    let _ = fetch_job::enqueue(&app.pool, entry_id, user_id, "https://x.test/", 0)
        .await
        .unwrap();

    let job_a = fetch_job::dequeue_one(&app.pool, "worker-a")
        .await
        .unwrap()
        .unwrap();
    assert!(
        fetch_job::dequeue_one(&app.pool, "worker-b")
            .await
            .unwrap()
            .is_none()
    );

    sqlx::query("UPDATE fetch_jobs SET leased_until = NOW() - INTERVAL '1 second' WHERE id = $1")
        .bind(job_a.id)
        .execute(&app.pool)
        .await
        .unwrap();

    let job_b = fetch_job::dequeue_one(&app.pool, "worker-b")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(job_b.id, job_a.id);
    assert_eq!(job_b.attempts, 2);

    // worker-a tries to complete after losing lease — must be no-op due to leased_by check.
    fetch_job::complete(&app.pool, job_a.id, "worker-a")
        .await
        .unwrap();
    let row = fetch_job::find_by_id(&app.pool, job_a.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.status, FetchJobStatus::Running);
    assert_eq!(row.leased_by.as_deref(), Some("worker-b"));

    app.cleanup().await;
}

#[tokio::test]
async fn fetch_queue_send_persists_to_db() {
    let app = common::TestApp::new().await;
    let user_id = app.create_user("queue").await;
    let entry_id = app.create_entry(user_id, "https://x.test/").await;

    let queue = lettura::tasks::fetcher::FetchQueue::new(app.pool.clone());
    queue
        .send(lettura::tasks::fetcher::FetchJob {
            entry_id,
            user_id,
            url: "https://x.test/".into(),
        })
        .await
        .unwrap();

    let count: i64 =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM fetch_jobs WHERE entry_id=$1")
            .bind(entry_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
    app.cleanup().await;
}

/// End-to-end worker lifecycle test.
///
/// Uses an unreachable public-looking host so the pipeline returns
/// `FetchError::Transient` (DNS failure). The worker should:
///   1. dequeue the job (NOTIFY-driven, no poll wait)
///   2. invoke pipeline::process
///   3. on transient failure → call fetch_job::fail
///   4. at max_attempts → call mark_failed and write extract_method='failed'
///
/// This verifies the full worker → pipeline → error routing → DB writeback
/// chain that mpsc-based tests could not cover, without depending on a real
/// HTTP server (which would collide with the SSRF defense-in-depth check
/// that blocks all loopback targets).
#[tokio::test]
async fn db_worker_processes_job_and_routes_transient_failure() {
    use lettura::tasks::fetch_worker;
    use std::time::Duration;
    use tokio_util::sync::CancellationToken;

    let app = common::TestApp::new().await;
    let user_id = app.create_user("worker").await;
    // example.invalid is reserved by RFC 2606 — guaranteed NXDOMAIN.
    let url = "http://nonexistent.example.invalid/article";
    let entry_id = app.create_entry(user_id, url).await;

    let queue = lettura::tasks::fetcher::FetchQueue::new(app.pool.clone());
    queue
        .send(lettura::tasks::fetcher::FetchJob {
            entry_id,
            user_id,
            url: url.into(),
        })
        .await
        .unwrap();

    // Force max_attempts=1 so a single failure goes straight to dead-letter +
    // mark_failed, keeping the test under a few seconds.
    sqlx::query("UPDATE fetch_jobs SET max_attempts = 1 WHERE entry_id = $1")
        .bind(entry_id)
        .execute(&app.pool)
        .await
        .unwrap();

    let cancel = CancellationToken::new();
    fetch_worker::spawn_workers(
        fetch_worker::WorkerConfig {
            pool: app.pool.clone(),
            image_storage: std::sync::Arc::from(lettura::storage::create_storage(&app.config)),
            search_index: app.search_index.clone(),
            // Short timeout to keep the test snappy; the failure path doesn't
            // depend on retry depth past max_attempts.
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(2))
                .build()
                .unwrap(),
            max_retries: 0,
            caches: app.caches.clone(),
            #[cfg(feature = "rendering")]
            render_service: None,
            #[cfg(feature = "test-utils")]
            skip_ssrf: false,
        },
        1,
        cancel.clone(),
    );

    // Wait for the worker to flip the job to dead and run mark_failed.
    // Window must exceed the 5s LISTEN-or-poll fallback plus DNS resolution
    // and the 2s client timeout — comfortably under contention from other
    // worker-spawning tests in the same suite.
    let mut observed_dead = false;
    for _ in 0..125 {
        tokio::time::sleep(Duration::from_millis(200)).await;
        let row: Option<(String, Option<String>)> =
            sqlx::query_as("SELECT status::text, last_error FROM fetch_jobs WHERE entry_id = $1")
                .bind(entry_id)
                .fetch_optional(&app.pool)
                .await
                .unwrap();
        if let Some((status, _)) = row
            && status == "dead"
        {
            observed_dead = true;
            break;
        }
    }
    assert!(
        observed_dead,
        "job did not reach 'dead' status within 25s; worker may not be dequeuing"
    );

    // Worker should also have called mark_failed → extract_method = 'failed'.
    let method: Option<String> =
        sqlx::query_scalar("SELECT extract_method FROM entries WHERE id = $1")
            .bind(entry_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(
        method.as_deref(),
        Some("failed"),
        "mark_failed should have set extract_method='failed' after max_attempts"
    );

    cancel.cancel();
    tokio::time::sleep(Duration::from_millis(100)).await;
    app.cleanup().await;
}

/// Graceful shutdown test.
///
/// Verifies that when the worker is cancelled while `pipeline::process` is
/// in-flight, the in-progress job is released back to `pending` and the
/// `attempts` counter is rolled back so a future worker can pick it up
/// without spurious retry penalty.
///
/// We use httpmock with an artificially slow response to keep the worker
/// blocked inside `pipeline::process` long enough to fire the cancellation
/// token. Because httpmock binds to loopback (rejected by the SSRF defense),
/// the worker is started with `skip_ssrf = true` — a test-only flag gated
/// behind `feature = "test-utils"` so the bypass cannot exist in release
/// builds. This test therefore only compiles with `--features test-utils`.
#[cfg(feature = "test-utils")]
#[tokio::test]
async fn cancel_during_processing_releases_job() {
    use httpmock::prelude::*;
    use lettura::models::fetch_job::FetchJobStatus;
    use lettura::tasks::fetch_worker;
    use std::time::Duration;
    use tokio_util::sync::CancellationToken;

    // Mock server returns a slow response so we can interrupt mid-flight.
    let mock_server = MockServer::start();
    let _m = mock_server.mock(|when, then| {
        when.method(GET).path("/slow");
        then.status(200)
            .delay(Duration::from_secs(10))
            .body("<html><body>nope</body></html>");
    });

    let app = common::TestApp::new().await;
    let user_id = app.create_user("cancel").await;
    let url = mock_server.url("/slow");
    let entry_id = app.create_entry(user_id, &url).await;

    let queue = lettura::tasks::fetcher::FetchQueue::new(app.pool.clone());
    queue
        .send(lettura::tasks::fetcher::FetchJob {
            entry_id,
            user_id,
            url: url.clone(),
        })
        .await
        .unwrap();

    let cancel = CancellationToken::new();
    fetch_worker::spawn_workers(
        fetch_worker::WorkerConfig {
            pool: app.pool.clone(),
            image_storage: std::sync::Arc::from(lettura::storage::create_storage(&app.config)),
            search_index: app.search_index.clone(),
            client: reqwest::Client::new(),
            max_retries: 1,
            caches: app.caches.clone(),
            #[cfg(feature = "rendering")]
            render_service: None,
            // Loopback URLs from httpmock would otherwise be blocked by SSRF.
            #[cfg(feature = "test-utils")]
            skip_ssrf: true,
        },
        1,
        cancel.clone(),
    );

    // Wait until the worker has picked up the job (status=running). The
    // window must exceed the worker's 5s LISTEN-or-poll fallback so the
    // race where NOTIFY fires before PgListener is established still
    // resolves via the poll path.
    let mut picked = false;
    for _ in 0..80 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let status: Option<FetchJobStatus> = sqlx::query_scalar::<_, FetchJobStatus>(
            "SELECT status FROM fetch_jobs WHERE entry_id=$1",
        )
        .bind(entry_id)
        .fetch_optional(&app.pool)
        .await
        .unwrap();
        if matches!(status, Some(FetchJobStatus::Running)) {
            picked = true;
            break;
        }
    }
    assert!(picked, "worker did not pick up job within 8s");

    // Cancel — worker should release the job before the slow response returns.
    cancel.cancel();
    // Give the worker enough time to observe the cancellation, abort the
    // renewer, and run `release` before we read state back.
    tokio::time::sleep(Duration::from_secs(1)).await;

    let row: (FetchJobStatus, i16) =
        sqlx::query_as("SELECT status, attempts FROM fetch_jobs WHERE entry_id=$1")
            .bind(entry_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(row.0, FetchJobStatus::Pending);
    assert_eq!(row.1, 0, "released job's attempt count rolled back");

    app.cleanup().await;
}
