use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

use crate::config::Config;

pub async fn create_pool(config: &Config) -> PgPool {
    PgPoolOptions::new()
        .max_connections(config.db_max_connections)
        .min_connections(config.db_min_connections)
        .acquire_timeout(std::time::Duration::from_secs(config.db_acquire_timeout_secs))
        .connect(&config.database_url)
        .await
        .expect("failed to connect to database")
}

pub async fn run_migrations(pool: &PgPool) {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .expect("failed to run migrations");
}
