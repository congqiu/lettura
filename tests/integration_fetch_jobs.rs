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
    let row = fetch_job::find_by_id(&app.pool, id1).await.unwrap().unwrap();
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

    let picked = fetch_job::dequeue_one(&app.pool, "w").await.unwrap().unwrap();
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
    let _ = fetch_job::dequeue_one(&app.pool, "w-1").await.unwrap().unwrap();

    fetch_job::complete(&app.pool, id, "w-1").await.unwrap();

    assert!(fetch_job::find_by_id(&app.pool, id).await.unwrap().is_none());
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
    let job = fetch_job::dequeue_one(&app.pool, "w-1").await.unwrap().unwrap();

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
        let job = fetch_job::dequeue_one(&app.pool, "w-1").await.unwrap().unwrap();
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
    let job = fetch_job::dequeue_one(&app.pool, "w-1").await.unwrap().unwrap();
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
    fetch_job::complete(&app.pool, job_a.id, "worker-a").await.unwrap();
    let row = fetch_job::find_by_id(&app.pool, job_a.id).await.unwrap().unwrap();
    assert_eq!(row.status, FetchJobStatus::Running);
    assert_eq!(row.leased_by.as_deref(), Some("worker-b"));

    app.cleanup().await;
}
