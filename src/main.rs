use axum::response::IntoResponse;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    // Initialize structured JSON logging
    // Set RUST_LOG=lettura=info,audit=info to control log levels
    let env_filter = EnvFilter::from_default_env()
        .add_directive("lettura=info".parse().expect("valid tracing directive"))
        .add_directive("audit=info".parse().expect("valid tracing directive"));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .json()
        .with_target(false)
        .with_file(false)
        .with_line_number(false)
        .init();

    dotenvy::dotenv().ok();
    let config = lettura::config::Config::from_env().unwrap_or_else(|e| {
        eprintln!("Configuration error: {e}");
        std::process::exit(1);
    });

    let pool = lettura::db::create_pool(&config).await.unwrap_or_else(|e| {
        eprintln!("Database error: {e}");
        std::process::exit(1);
    });
    lettura::db::run_migrations(&pool)
        .await
        .unwrap_or_else(|e| {
            eprintln!("Migration error: {e}");
            std::process::exit(1);
        });

    // `_fetch_queue` is intentionally unused at the binary level: the router
    // owns its own clone for handlers, and the queue's depth is reported via
    // the `fetch_queue_size{status=...}` gauge family sampled directly from
    // `fetch_jobs` below — no in-process counter needed.
    //
    // `caches` is the *same* Arc that the router puts into AppState; passing
    // it to spawn_workers below makes cache invalidations from background
    // tagging visible to HTTP handlers immediately.
    let (app, search_index, _fetch_queue, storage, caches) =
        lettura::api::router_with_handles(pool.clone(), config.clone());

    // CancellationToken drives graceful shutdown of every long-running task.
    // SIGTERM / Ctrl-C flips it; every periodic task wakes immediately from
    // its sleep/tick, breaks out of its loop, and the axum server stops
    // accepting new requests via `with_graceful_shutdown` below.
    let cancel = tokio_util::sync::CancellationToken::new();
    {
        let cancel = cancel.clone();
        tokio::spawn(async move {
            shutdown_signal().await;
            tracing::info!("shutdown signal received");
            cancel.cancel();
        });
    }

    // Build fetch worker dependencies once and spawn N workers. The workers
    // attach to the same pg pool, LISTEN for `fetch_jobs_new`, and dequeue
    // with FOR UPDATE SKIP LOCKED.
    let http_client = lettura::fetch::http::build_client(&config);
    #[cfg(feature = "rendering")]
    let render_service = if config.rendering_runtime_enabled() {
        Some(std::sync::Arc::new(
            lettura::fetch::render::RenderService::new(
                config.chromium_path.clone(),
                config.render_concurrency,
                config.render_timeout_ms,
            ),
        ))
    } else {
        tracing::info!("render fallback disabled via LETTURA_RENDERING_ENABLED");
        None
    };

    lettura::tasks::fetch_worker::spawn_workers(
        lettura::tasks::fetch_worker::WorkerConfig {
            pool: pool.clone(),
            image_storage: storage.clone(),
            search_index: search_index.clone(),
            client: http_client,
            max_retries: config.fetch_max_retries,
            caches: caches.clone(),
            #[cfg(feature = "rendering")]
            render_service,
            // `skip_ssrf` only exists when test-utils is enabled. Production
            // release builds (default features) drop the field entirely so
            // SSRF validation cannot be turned off by mistake.
            #[cfg(any(test, feature = "test-utils"))]
            skip_ssrf: false,
        },
        config.fetch_concurrency,
        cancel.clone(),
    );

    // Start image processor
    lettura::tasks::start_image_processor(
        pool.clone(),
        storage.clone(),
        config.max_image_size,
        cancel.clone(),
    );

    // Background task: flush search index every 3 seconds. Documents become
    // searchable within this window after a write. Critical paths (e.g.
    // permanent delete) call commit() directly for stronger guarantees.
    // On shutdown, breaks out of the loop; the final flush happens in the
    // axum `with_graceful_shutdown` callback below so any writes after the
    // last tick are still committed.
    {
        let idx = search_index.clone();
        let cancel = cancel.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(
                config.search_commit_interval_secs,
            ));
            let mut consecutive_failures: u64 = 0;
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = interval.tick() => {}
                }
                match idx.commit().await {
                    Ok(()) => consecutive_failures = 0,
                    Err(e) => {
                        consecutive_failures += 1;
                        metrics::counter!("search_index_commit_failures_total").increment(1);
                        if consecutive_failures.is_power_of_two() {
                            tracing::error!(
                                consecutive_failures,
                                "search index commit failed: {e}"
                            );
                        } else {
                            tracing::warn!("search index commit failed: {e}");
                        }
                    }
                }
            }
        });
    }

    {
        let cleanup_pool = pool.clone();
        let cancel = cancel.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(
                config.token_cleanup_interval_secs,
            ));
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = interval.tick() => {}
                }
                match lettura::models::user::cleanup_expired_refresh_tokens(&cleanup_pool).await {
                    Ok(count) if count > 0 => {
                        tracing::info!(removed = count, "cleaned up expired refresh tokens")
                    }
                    Err(e) => tracing::warn!("refresh token cleanup failed: {e}"),
                    _ => {}
                }
            }
        });
    }

    // Periodic dead-letter cleanup. Hourly DELETE of fetch_jobs whose status
    // is 'dead' and whose last_error_at is older than
    // LETTURA_FETCH_DEAD_TTL_DAYS. Cancel-aware so graceful shutdown does not
    // wait a full hour.
    {
        let pool = pool.clone();
        let cancel = cancel.clone();
        let ttl_days = config.fetch_dead_ttl_days;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = interval.tick() => {}
                }
                let r = sqlx::query(
                    "DELETE FROM fetch_jobs WHERE status = 'dead' \
                     AND last_error_at < NOW() - ($1 || ' days')::interval",
                )
                .bind(ttl_days.to_string())
                .execute(&pool)
                .await;
                match r {
                    Ok(r) if r.rows_affected() > 0 => {
                        tracing::info!(
                            deleted = r.rows_affected(),
                            "cleaned up dead fetch jobs"
                        );
                        metrics::counter!("fetch_jobs_purged_total")
                            .increment(r.rows_affected());
                    }
                    Err(e) => tracing::warn!("dead fetch_jobs cleanup failed: {e}"),
                    _ => {}
                }
            }
        });
    }

    let app = if config.metrics_enabled {
        let recorder_handle = metrics_exporter_prometheus::PrometheusBuilder::new()
            .install_recorder()
            .expect("failed to install metrics recorder");

        let metrics_route = if let Some(ref token) = config.metrics_bearer_token {
            let token_clone = token.clone();
            axum::Router::new()
                .route(
                    "/metrics",
                    axum::routing::get(move || async move { recorder_handle.render() }),
                )
                .layer(axum::middleware::from_fn(
                    move |req: axum::extract::Request, next: axum::middleware::Next| {
                        let expected = token_clone.clone();
                        async move {
                            let auth = req
                                .headers()
                                .get("authorization")
                                .and_then(|v| v.to_str().ok())
                                .and_then(|s| s.strip_prefix("Bearer "));
                            match auth {
                                Some(provided)
                                    if subtle::ConstantTimeEq::ct_eq(
                                        provided.as_bytes(),
                                        expected.as_bytes(),
                                    )
                                    .into() =>
                                {
                                    next.run(req).await
                                }
                                _ => (axum::http::StatusCode::UNAUTHORIZED, "unauthorized")
                                    .into_response(),
                            }
                        }
                    },
                ))
        } else {
            // No bearer token configured — metrics endpoint is not exposed
            axum::Router::new()
        };

        // Background task to periodically report gauge metrics
        let search_idx = search_index.clone();
        let pool_for_metrics = pool.clone();
        let cancel = cancel.clone();
        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(std::time::Duration::from_secs(config.metrics_interval_secs));
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = interval.tick() => {}
                }
                if let Ok(count) = search_idx.doc_count() {
                    metrics::gauge!("search_index_documents").set(count as f64);
                }
                metrics::gauge!("db_pool_size").set(pool_for_metrics.size() as f64);
                metrics::gauge!("db_pool_idle").set(pool_for_metrics.num_idle() as f64);

                // Per-status queue depth sampled directly from fetch_jobs.
                // Statuses with zero rows are reset to 0 so a label that
                // becomes empty does not stick at its last non-zero value.
                if let Ok(counts) =
                    lettura::models::fetch_job::count_by_status(&pool_for_metrics).await
                {
                    let mut seen = std::collections::HashSet::new();
                    for (status, n) in counts {
                        let label = fetch_job_status_label(status);
                        seen.insert(label);
                        metrics::gauge!("fetch_queue_size", "status" => label).set(n as f64);
                    }
                    for label in ["pending", "running", "failed", "dead"] {
                        if !seen.contains(label) {
                            metrics::gauge!("fetch_queue_size", "status" => label).set(0.0);
                        }
                    }
                }
            }
        });

        tracing::info!("Prometheus metrics enabled at /metrics");
        metrics_route
            .merge(app)
            .layer(axum::middleware::from_fn(lettura::metrics::track_metrics))
    } else {
        app
    };

    let listener = tokio::net::TcpListener::bind(&config.listen_addr)
        .await
        .expect("failed to bind listener");

    tracing::info!("listening on {}", config.listen_addr);
    let shutdown_idx = search_index.clone();
    let shutdown_cancel = cancel.clone();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        shutdown_cancel.cancelled().await;
        // Final flush so writes buffered since the last periodic commit
        // are not lost when the process exits.
        if let Err(e) = shutdown_idx.commit().await {
            tracing::warn!("final search index commit failed: {e}");
        } else {
            tracing::info!("search index flushed on shutdown");
        }
    })
    .await
    .expect("server error");
}

/// Stable prometheus label for each [`FetchJobStatus`] variant. Kept in sync
/// with the enum so changes to the type require touching the label table.
fn fetch_job_status_label(s: lettura::models::fetch_job::FetchJobStatus) -> &'static str {
    use lettura::models::fetch_job::FetchJobStatus::*;
    match s {
        Pending => "pending",
        Running => "running",
        Failed => "failed",
        Dead => "dead",
    }
}

async fn shutdown_signal() {
    use tokio::signal;
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };
    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
