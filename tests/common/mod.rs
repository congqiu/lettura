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
    pub config: Config,
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
            fetch_concurrency: 5,
            fetch_max_attempts: 5,
            fetch_lease_secs: 300,
            fetch_dead_ttl_days: 30,
            rendering_enabled: "false".to_string(),
            chromium_path: None,
            render_concurrency: 1,
            render_timeout_ms: 15000,
            public_base_url: None,
            production: false,
            trust_proxy: false,
            disable_registration: false,
            metrics_bearer_token: None,
            import_max_body_bytes: 50 * 1024 * 1024,
            pages_max_upload_bytes: 10 * 1024 * 1024,
            max_image_size: 10 * 1024 * 1024,
            auth_rate_limit: 10,
            global_rate_limit: 100,
            search_commit_interval_secs: 3,
            token_cleanup_interval_secs: 3600,
            metrics_interval_secs: 15,
        };

        let (app, _, _, _) = lettura::api::router_with_search(
            pool.clone(),
            config.clone(),
            Some(search_index.clone()),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = format!("http://{}", listener.local_addr().unwrap());

        tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .await
            .unwrap();
        });

        TestApp {
            addr,
            pool,
            client: reqwest::Client::new(),
            db_name,
            search_index,
            config: config.clone(),
            base_url,
        }
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.addr, path)
    }

    /// Insert a user directly via SQL, bypassing the auth API.
    /// Returns the new user's ID. Use this in DAO tests that don't need
    /// a real password hash or JWT.
    pub async fn create_user(&self, username: &str) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO users (id, username, email, password_hash) \
             VALUES ($1, $2, $3, 'x-not-a-real-hash')",
        )
        .bind(id)
        .bind(username)
        .bind(format!("{}@test.local", username))
        .execute(&self.pool)
        .await
        .expect("create_user insert");
        id
    }

    /// Insert a minimal entry row for the given user, returning the entry ID.
    pub async fn create_entry(&self, user_id: Uuid, url: &str) -> Uuid {
        use sha1::{Digest, Sha1};
        let hashed = hex::encode(Sha1::digest(url.as_bytes()));
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO entries \
             (id, user_id, url, given_url, hashed_url, hashed_given_url) \
             VALUES ($1, $2, $3, $3, $4, $4)",
        )
        .bind(id)
        .bind(user_id)
        .bind(url)
        .bind(hashed)
        .execute(&self.pool)
        .await
        .expect("create_entry insert");
        id
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
