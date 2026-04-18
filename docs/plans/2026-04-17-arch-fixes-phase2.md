# Phase 2: Backend Architecture Improvements

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Decouple model layer from API errors, add model-level error types, improve config validation, and add `expect` messages for startup panics.

**Architecture:** Introduce a `ModelError` enum in a shared location that models return. The API layer converts `ModelError` to `ApiError`. This breaks the circular dependency where models import from the API layer.

**Tech Stack:** Rust (Axum, sqlx, thiserror)

**Depends on:** Phase 1 (AppState has been moved to `state.rs`)

---

## Task 1: Create `ModelError` and decouple models from `ApiError`

**Files:**
- Create: `src/models/error.rs`
- Modify: `src/models/mod.rs`
- Modify: All model files (`src/models/user.rs`, `src/models/entry.rs`, `src/models/tag.rs`, `src/models/annotation.rs`, `src/models/memo.rs`, `src/models/tagging_rule.rs`, `src/models/site_rule.rs`, `src/models/page.rs`)
- Modify: `src/api/error.rs` (add `From<ModelError>`)

- [ ] **Step 1: Create `src/models/error.rs`**

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ModelError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("{0}")]
    Database(String),
}

impl From<sqlx::Error> for ModelError {
    fn from(e: sqlx::Error) -> Self {
        match &e {
            sqlx::Error::Database(db_err) => {
                if let Some(constraint) = db_err.constraint() {
                    let msg = match constraint {
                        "users_email_key" => "email already exists",
                        "users_username_key" => "username already exists",
                        "idx_entries_user_hashed_url" => "URL already saved",
                        _ => "duplicate record",
                    };
                    return ModelError::Conflict(msg.to_string());
                }
                ModelError::Database(e.to_string())
            }
            _ => ModelError::Database(e.to_string()),
        }
    }
}
```

- [ ] **Step 2: Register module in `src/models/mod.rs`**

Add `pub mod error;` to the module declarations.

- [ ] **Step 3: Update `src/api/error.rs` to convert from `ModelError`**

```rust
impl From<crate::models::error::ModelError> for ApiError {
    fn from(e: crate::models::error::ModelError) -> Self {
        match e {
            crate::models::error::ModelError::NotFound(msg) => ApiError::NotFound(msg),
            crate::models::error::ModelError::Conflict(msg) => ApiError::Conflict(msg),
            crate::models::error::ModelError::Database(msg) => {
                tracing::error!("database error: {msg}");
                ApiError::Internal("internal server error".to_string())
            }
        }
    }
}
```

The existing `From<sqlx::Error> for ApiError` impl can remain for any direct sqlx usage in API handlers, but model files should now use `ModelError`.

- [ ] **Step 4: Update each model file**

For each model file, replace:
- `use crate::api::error::ApiError;` → `use super::error::ModelError;`
- All `Result<_, ApiError>` → `Result<_, ModelError>`
- All `ApiError::NotFound(...)` → `ModelError::NotFound(...)`
- All `ApiError::Internal(e.to_string())` → `ModelError::Database(e.to_string())`
- All `.map_err(|e| ApiError::Internal(...))` → use `?` with the `From<sqlx::Error> for ModelError` impl, or `.map_err(|e| ModelError::Database(e.to_string()))`

Do this for each file:
1. `src/models/user.rs`
2. `src/models/entry.rs`
3. `src/models/tag.rs`
4. `src/models/annotation.rs`
5. `src/models/memo.rs`
6. `src/models/tagging_rule.rs` (note: `evaluate_rule` and `EntryFields` don't use errors, keep them as-is)
7. `src/models/site_rule.rs`
8. `src/models/page.rs`

- [ ] **Step 5: Run tests**

Run: `cargo test --lib`
Expected: all tests pass

- [ ] **Step 6: Commit**

```bash
git add src/models/
git commit -m "refactor: introduce ModelError to decouple models from API layer"
```

---

## Task 2: Replace `expect()` panics in config and db with proper error handling

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Replace `expect()` in `Config::from_env()` with `Err()`**

In `src/config.rs`, line 47:

```rust
// Before:
database_url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),

// After:
database_url: env::var("DATABASE_URL").map_err(|_| "DATABASE_URL must be set".to_string())?,
```

- [ ] **Step 2: Run tests**

Run: `cargo test --lib config`
Expected: all tests pass

- [ ] **Step 3: Commit**

```bash
git add src/config.rs
git commit -m "fix: replace expect() panic with proper error in Config::from_env"
```

---

## Task 3: Improve `db.rs` error handling

**Files:**
- Modify: `src/db.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Return `Result` from `create_pool` and `run_migrations`**

```rust
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
```

Update `src/main.rs` to use `?` with proper error printing:

```rust
let pool = lettura::db::create_pool(&config).await.unwrap_or_else(|e| {
    eprintln!("Database error: {e}");
    std::process::exit(1);
});
lettura::db::run_migrations(&pool).await.unwrap_or_else(|e| {
    eprintln!("Migration error: {e}");
    std::process::exit(1);
});
```

- [ ] **Step 2: Run tests**

Run: `cargo test --lib`
Expected: all tests pass

- [ ] **Step 3: Commit**

```bash
git add src/db.rs src/main.rs
git commit -m "fix: return Result from db pool creation and migration functions"
```
