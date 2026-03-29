use std::sync::atomic::Ordering;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    dotenvy::dotenv().ok();
    let config = lettura::config::Config::from_env().unwrap_or_else(|e| {
        eprintln!("Configuration error: {e}");
        std::process::exit(1);
    });

    let pool = lettura::db::create_pool(&config).await;
    lettura::db::run_migrations(&pool).await;

    let (app, search_index, fetch_queue) =
        lettura::api::router_with_handles(pool.clone(), config.clone());

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
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(15));
            loop {
                interval.tick().await;
                let depth = fetch_depth.load(Ordering::Relaxed) as f64;
                metrics::gauge!("fetch_queue_depth").set(depth);
                if let Ok(count) = search_idx.doc_count() {
                    metrics::gauge!("search_index_documents").set(count as f64);
                }
            }
        });

        tracing::info!("Prometheus metrics enabled at /metrics");
        app.merge(metrics_route)
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
    axum::serve(listener, app).await.expect("server error");
}
