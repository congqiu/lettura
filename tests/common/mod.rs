use lettura::config::Config;
use lettura::search::SearchIndex;
use sqlx::PgPool;
use uuid::Uuid;

pub struct TestApp {
    pub addr: String,
    pub pool: PgPool,
    pub client: reqwest::Client,
    pub db_name: String,
    pub search_index: SearchIndex,
    base_url: String,
}

impl TestApp {
    pub async fn new() -> Self {
        let base_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://lettura:lettura@127.0.0.1:5436/lettura".to_string());

        let db_name = format!("lettura_test_{}", Uuid::new_v4().simple());
        let base_pool = PgPool::connect(&base_url).await.unwrap();
        sqlx::query(&format!("CREATE DATABASE \"{}\"", db_name))
            .execute(&base_pool)
            .await
            .unwrap();
        base_pool.close().await;

        let test_url_base = base_url.rsplit_once('/').unwrap().0;
        let test_db_url = format!("{}/{}", test_url_base, db_name);
        let pool = PgPool::connect(&test_db_url).await.unwrap();

        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        let search_index = SearchIndex::in_memory().unwrap();

        let config = Config {
            database_url: test_db_url,
            jwt_secret: "test-secret-at-least-32-characters-long-for-testing".to_string(),
            listen_addr: "127.0.0.1:0".to_string(),
            index_path: "/tmp/lettura-test-index".to_string(),
            storage_type: "local".to_string(),
            storage_local_path: "/tmp/lettura-test-storage".to_string(),
            pages_storage_path: "/tmp/lettura-test-pages".to_string(),
            oss_endpoint: String::new(),
            oss_region: "auto".to_string(),
            oss_bucket: String::new(),
            oss_access_key: String::new(),
            oss_secret_key: String::new(),
            oss_public_url: String::new(),
            db_max_connections: 10,
            db_min_connections: 2,
            db_acquire_timeout_secs: 30,
            cors_origins: "*".to_string(),
            metrics_enabled: false,
            user_agent: "Mozilla/5.0 (test)".to_string(),
            fetch_timeout_secs: 30,
            fetch_max_retries: 3,
            proxy: None,
            site_configs_path: None,
            rendering_enabled: "false".to_string(),
            chromium_path: None,
            render_concurrency: 1,
            render_timeout_ms: 15000,
            public_base_url: None,
        };

        let (app, _, _, _) = lettura::api::router_with_search(
            pool.clone(),
            config.clone(),
            Some(search_index.clone()),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = format!("http://{}", listener.local_addr().unwrap());

        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        TestApp {
            addr,
            pool,
            client: reqwest::Client::new(),
            db_name,
            search_index,
            base_url,
        }
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.addr, path)
    }

    pub async fn cleanup(self) {
        self.pool.close().await;
        let base_pool = PgPool::connect(&self.base_url).await.unwrap();
        sqlx::query(&format!("DROP DATABASE IF EXISTS \"{}\"", self.db_name))
            .execute(&base_pool)
            .await
            .ok();
        base_pool.close().await;
    }
}
