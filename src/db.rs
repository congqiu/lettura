pub async fn create_pool(config: &crate::config::Config) -> Result<sqlx::PgPool, String> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(config.db_max_connections)
        .min_connections(config.db_min_connections)
        .acquire_timeout(std::time::Duration::from_secs(config.db_acquire_timeout_secs))
        .connect(&config.database_url)
        .await
        .map_err(|e| format!("failed to create database pool: {e}"))?;
    Ok(pool)
}

pub async fn run_migrations(pool: &sqlx::PgPool) -> Result<(), String> {
    sqlx::migrate!()
        .run(pool)
        .await
        .map_err(|e| format!("failed to run database migrations: {e}"))?;
    Ok(())
}
