use std::sync::atomic::Ordering;
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
    lettura::db::run_migrations(&pool).await.unwrap_or_else(|e| {
        eprintln!("Migration error: {e}");
        std::process::exit(1);
    });

    let (app, search_index, fetch_queue, storage) =
        lettura::api::router_with_handles(pool.clone(), config.clone());

    // Start image processor
    lettura::tasks::start_image_processor(pool.clone(), storage.clone());

    // Background task: flush search index every 3 seconds. Documents become
    // searchable within this window after a write. Critical paths (e.g.
    // permanent delete) call commit() directly for stronger guarantees.
    {
        let idx = search_index.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(3));
            let mut consecutive_failures: u64 = 0;
            loop {
                interval.tick().await;
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
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
            loop {
                interval.tick().await;
                match lettura::models::user::cleanup_expired_refresh_tokens(&cleanup_pool).await {
                    Ok(count) if count > 0 => tracing::info!(removed = count, "cleaned up expired refresh tokens"),
                    Err(e) => tracing::warn!("refresh token cleanup failed: {e}"),
                    _ => {}
                }
            }
        });
    }

    let app = if config.metrics_enabled {
        let recorder_handle = metrics_exporter_prometheus::PrometheusBuilder::new()
            .install_recorder()
            .expect("failed to install metrics recorder");

        let metrics_route = axum::Router::new().route(
            "/metrics",
            axum::routing::get(move || async move { recorder_handle.render() }),
        );

        // Background task to periodically report gauge metrics
        let fetch_depth = fetch_queue.queue_depth.clone();
        let search_idx = search_index.clone();
        let pool_for_metrics = pool.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(15));
            loop {
                interval.tick().await;
                let depth = fetch_depth.load(Ordering::Relaxed) as f64;
                metrics::gauge!("fetch_queue_depth").set(depth);
                if let Ok(count) = search_idx.doc_count() {
                    metrics::gauge!("search_index_documents").set(count as f64);
                }
                metrics::gauge!("db_pool_size").set(pool_for_metrics.size() as f64);
                metrics::gauge!("db_pool_idle").set(pool_for_metrics.num_idle() as f64);
            }
        });

        tracing::info!("Prometheus metrics enabled at /metrics");
        metrics_route
            .merge(app)
            .layer(axum::middleware::from_fn(
                lettura::metrics::track_metrics,
            ))
    } else {
        app
    };

    let listener = tokio::net::TcpListener::bind(&config.listen_addr)
        .await
        .expect("failed to bind listener");

    tracing::info!("listening on {}", config.listen_addr);
    let shutdown_idx = search_index.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
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
