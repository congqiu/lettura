# Plan 2a: 数据库 + 认证系统

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 搭建 Axum Web 服务，接入 PostgreSQL 数据库，实现完整的 JWT 认证流程（注册、登录、刷新、登出）。

**Architecture:** Axum HTTP 服务 + SQLx 异步 PostgreSQL 连接池。认证使用 argon2 密码哈希 + JWT (access token 15min + refresh token 30 days in DB)。首个注册用户自动成为 admin。集成测试使用 Docker PostgreSQL。

**Tech Stack:** Rust 2024, Axum 0.8, SQLx 0.8 (postgres), tokio, argon2, jsonwebtoken, uuid, chrono, tower-http

**编译/测试方式:** 本地写代码，通过远程 Docker 编译测试。编译命令模板：
```bash
# 仅编译检查
docker run --rm -v "$HOME/workspace/lettura":/app -v lettura-cargo-registry:/usr/local/cargo/registry -v lettura-cargo-target:/app/target -w /app rust:1.87-slim cargo check 2>&1

# 运行单元测试（不需要 DB）
docker run --rm -v "$HOME/workspace/lettura":/app -v lettura-cargo-registry:/usr/local/cargo/registry -v lettura-cargo-target:/app/target -w /app rust:1.87-slim cargo test --lib 2>&1

# 运行集成测试（需要 DB，用 --network=host 连接宿主机上的 PG）
docker run --rm --network=host -v "$HOME/workspace/lettura":/app -v lettura-cargo-registry:/usr/local/cargo/registry -v lettura-cargo-target:/app/target -w /app -e DATABASE_URL=postgres://lettura:lettura@127.0.0.1:5432/lettura_test rust:1.87-slim cargo test --test integration_auth 2>&1
```

---

## 文件结构

```
lettura/
├── Cargo.toml                    — 更新依赖
├── docker-compose.yml            — 开发用 PostgreSQL
├── .env.example                  — 环境变量模板
├── migrations/
│   ├── 001_create_users.sql
│   └── 002_create_refresh_tokens.sql
├── src/
│   ├── main.rs                   — Axum 服务入口
│   ├── lib.rs                    — 库入口（更新，re-export 新模块）
│   ├── config.rs                 — 环境变量配置
│   ├── db.rs                     — 数据库连接池 + 迁移
│   ├── auth/
│   │   ├── mod.rs                — re-export
│   │   ├── password.rs           — argon2 哈希/验证
│   │   ├── jwt.rs                — JWT 创建/验证
│   │   └── middleware.rs         — Axum 认证中间件 extractor
│   ├── api/
│   │   ├── mod.rs                — 路由组装
│   │   ├── error.rs              — 统一 API 错误类型
│   │   └── auth.rs               — 注册/登录/刷新/登出 handler
│   ├── models/
│   │   ├── mod.rs                — re-export
│   │   └── user.rs               — User + RefreshToken 模型与查询
│   └── extract/                  — (已有，不改动)
├── tests/
│   ├── common/
│   │   └── mod.rs                — 测试辅助（TestApp 启动、DB 清理）
│   └── integration_auth.rs       — 认证集成测试
```

---

### Task 1: 依赖更新 + 开发环境

**Files:**
- Modify: `Cargo.toml`
- Create: `docker-compose.yml`
- Create: `.env.example`

- [ ] **Step 1: 更新 Cargo.toml 添加新依赖**

```toml
[package]
name = "lettura"
version = "0.1.0"
edition = "2024"

[dependencies]
# Content extraction (existing)
scraper = "0.22"
ego-tree = "0.10"
ammonia = "4"
regex = "1"
once_cell = "1"
unicode-segmentation = "1.12"
url = "2"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Error handling
thiserror = "2"

# Web framework
axum = { version = "0.8", features = ["macros"] }
tokio = { version = "1", features = ["full"] }
tower = "0.5"
tower-http = { version = "0.6", features = ["cors", "trace"] }

# Database
sqlx = { version = "0.8", features = ["runtime-tokio-rustls", "postgres", "uuid", "chrono", "migrate"] }

# Auth
argon2 = "0.5"
jsonwebtoken = "9"
rand = "0.8"
sha2 = "0.10"
hex = "0.4"

# Types
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }

# Observability
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Config
dotenvy = "0.15"

[dev-dependencies]
pretty_assertions = "1"
reqwest = { version = "0.12", features = ["json"] }
```

- [ ] **Step 2: 创建 docker-compose.yml**

```yaml
services:
  postgres:
    image: postgres:16-alpine
    environment:
      POSTGRES_USER: lettura
      POSTGRES_PASSWORD: lettura
      POSTGRES_DB: lettura
    ports:
      - "5432:5432"
    volumes:
      - postgres_data:/var/lib/postgresql/data

volumes:
  postgres_data:
```

- [ ] **Step 3: 创建 .env.example**

```
DATABASE_URL=postgres://lettura:lettura@127.0.0.1:5432/lettura
JWT_SECRET=change-me-in-production-use-at-least-32-chars
LISTEN_ADDR=0.0.0.0:3000
```

- [ ] **Step 4: 验证编译通过**

Run: `cargo check`
Expected: 编译通过（可能有 unused 警告）

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml docker-compose.yml .env.example
git commit -m "chore: add web/db/auth dependencies and dev docker-compose"
```

---

### Task 2: 配置模块 + 服务骨架

**Files:**
- Create: `src/config.rs`
- Create: `src/main.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: 创建 src/config.rs**

```rust
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub listen_addr: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: env::var("DATABASE_URL")
                .expect("DATABASE_URL must be set"),
            jwt_secret: env::var("JWT_SECRET")
                .expect("JWT_SECRET must be set"),
            listen_addr: env::var("LISTEN_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:3000".to_string()),
        }
    }
}
```

- [ ] **Step 2: 创建 src/main.rs**

```rust
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    dotenvy::dotenv().ok();
    let config = lettura::config::Config::from_env();

    let pool = lettura::db::create_pool(&config.database_url).await;
    lettura::db::run_migrations(&pool).await;

    let app = lettura::api::router(pool.clone(), config.clone());

    let listener = tokio::net::TcpListener::bind(&config.listen_addr)
        .await
        .expect("failed to bind listener");

    tracing::info!("listening on {}", config.listen_addr);
    axum::serve(listener, app).await.expect("server error");
}
```

- [ ] **Step 3: 更新 src/lib.rs**

```rust
pub mod api;
pub mod auth;
pub mod config;
pub mod db;
pub mod extract;
pub mod models;
```

- [ ] **Step 4: 创建占位模块使其编译通过**

`src/db.rs`:
```rust
use sqlx::PgPool;

pub async fn create_pool(database_url: &str) -> PgPool {
    PgPool::connect(database_url)
        .await
        .expect("failed to connect to database")
}

pub async fn run_migrations(pool: &PgPool) {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .expect("failed to run migrations");
}
```

`src/auth/mod.rs`:
```rust
pub mod jwt;
pub mod middleware;
pub mod password;
```

`src/auth/password.rs`:
```rust
// Placeholder
```

`src/auth/jwt.rs`:
```rust
// Placeholder
```

`src/auth/middleware.rs`:
```rust
// Placeholder
```

`src/models/mod.rs`:
```rust
pub mod user;
```

`src/models/user.rs`:
```rust
// Placeholder
```

`src/api/mod.rs`:
```rust
use axum::Router;
use sqlx::PgPool;
use crate::config::Config;

pub mod auth;
pub mod error;

pub fn router(pool: PgPool, config: Config) -> Router {
    Router::new()
}
```

`src/api/error.rs`:
```rust
// Placeholder
```

`src/api/auth.rs`:
```rust
// Placeholder
```

- [ ] **Step 5: 创建空 migrations 目录**

```bash
mkdir -p migrations
```

创建 `migrations/.gitkeep` (空文件) 使 `sqlx::migrate!` 宏不报错。

- [ ] **Step 6: 验证编译通过**

Run: `cargo check`
Expected: 编译通过

- [ ] **Step 7: Commit**

```bash
git add src/ migrations/
git commit -m "feat: add config, server skeleton, and module structure"
```

---

### Task 3: 密码哈希模块

**Files:**
- Modify: `src/auth/password.rs`

- [ ] **Step 1: 写失败测试**

```rust
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PasswordError {
    #[error("failed to hash password")]
    HashError,
    #[error("invalid password")]
    VerifyError,
}

pub fn hash_password(password: &str) -> Result<String, PasswordError> {
    todo!()
}

pub fn verify_password(password: &str, hash: &str) -> Result<(), PasswordError> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_verify_password() {
        let password = "test_password_123";
        let hash = hash_password(password).unwrap();
        assert!(hash.starts_with("$argon2"));
        verify_password(password, &hash).unwrap();
    }

    #[test]
    fn wrong_password_fails() {
        let hash = hash_password("correct").unwrap();
        let result = verify_password("wrong", &hash);
        assert!(result.is_err());
    }

    #[test]
    fn different_hashes_for_same_password() {
        let h1 = hash_password("same").unwrap();
        let h2 = hash_password("same").unwrap();
        assert_ne!(h1, h2, "salts should differ");
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --lib auth::password`
Expected: FAIL — todo!() panic

- [ ] **Step 3: 实现密码哈希**

替换 `src/auth/password.rs` 全部内容:

```rust
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PasswordError {
    #[error("failed to hash password")]
    HashError,
    #[error("invalid password")]
    VerifyError,
}

pub fn hash_password(password: &str) -> Result<String, PasswordError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| PasswordError::HashError)?;
    Ok(hash.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> Result<(), PasswordError> {
    let parsed = PasswordHash::new(hash).map_err(|_| PasswordError::VerifyError)?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .map_err(|_| PasswordError::VerifyError)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_verify_password() {
        let password = "test_password_123";
        let hash = hash_password(password).unwrap();
        assert!(hash.starts_with("$argon2"));
        verify_password(password, &hash).unwrap();
    }

    #[test]
    fn wrong_password_fails() {
        let hash = hash_password("correct").unwrap();
        let result = verify_password("wrong", &hash);
        assert!(result.is_err());
    }

    #[test]
    fn different_hashes_for_same_password() {
        let h1 = hash_password("same").unwrap();
        let h2 = hash_password("same").unwrap();
        assert_ne!(h1, h2, "salts should differ");
    }
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test --lib auth::password`
Expected: 3 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/auth/password.rs
git commit -m "feat: implement argon2 password hashing and verification"
```

---

### Task 4: JWT 令牌管理

**Files:**
- Modify: `src/auth/jwt.rs`

- [ ] **Step 1: 写失败测试**

```rust
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: Uuid,       // user_id
    pub exp: i64,        // expiry timestamp
    pub iat: i64,        // issued at
    pub is_admin: bool,
}

#[derive(Error, Debug)]
pub enum JwtError {
    #[error("failed to create token")]
    CreationError,
    #[error("invalid token")]
    ValidationError,
    #[error("token expired")]
    Expired,
}

pub fn create_access_token(user_id: Uuid, is_admin: bool, secret: &str) -> Result<String, JwtError> {
    todo!()
}

pub fn validate_token(token: &str, secret: &str) -> Result<Claims, JwtError> {
    todo!()
}

pub fn generate_refresh_token() -> String {
    todo!()
}

pub fn hash_refresh_token(token: &str) -> String {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_validate_access_token() {
        let user_id = Uuid::new_v4();
        let secret = "test-secret-at-least-32-characters-long";
        let token = create_access_token(user_id, false, secret).unwrap();
        let claims = validate_token(&token, secret).unwrap();
        assert_eq!(claims.sub, user_id);
        assert!(!claims.is_admin);
    }

    #[test]
    fn admin_claim_roundtrips() {
        let user_id = Uuid::new_v4();
        let secret = "test-secret-at-least-32-characters-long";
        let token = create_access_token(user_id, true, secret).unwrap();
        let claims = validate_token(&token, secret).unwrap();
        assert!(claims.is_admin);
    }

    #[test]
    fn wrong_secret_fails() {
        let user_id = Uuid::new_v4();
        let token = create_access_token(user_id, false, "secret-one-at-least-32-characters").unwrap();
        let result = validate_token(&token, "secret-two-at-least-32-characters");
        assert!(result.is_err());
    }

    #[test]
    fn refresh_token_is_random() {
        let t1 = generate_refresh_token();
        let t2 = generate_refresh_token();
        assert_ne!(t1, t2);
        assert!(t1.len() >= 32);
    }

    #[test]
    fn refresh_token_hash_is_deterministic() {
        let token = "some-refresh-token";
        let h1 = hash_refresh_token(token);
        let h2 = hash_refresh_token(token);
        assert_eq!(h1, h2);
        assert_ne!(h1, token);
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --lib auth::jwt`
Expected: FAIL — todo!() panic

- [ ] **Step 3: 实现 JWT 管理**

替换 `src/auth/jwt.rs` 全部内容:

```rust
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

const ACCESS_TOKEN_DURATION_MINUTES: i64 = 15;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: Uuid,
    pub exp: i64,
    pub iat: i64,
    pub is_admin: bool,
}

#[derive(Error, Debug)]
pub enum JwtError {
    #[error("failed to create token")]
    CreationError,
    #[error("invalid token")]
    ValidationError,
    #[error("token expired")]
    Expired,
}

pub fn create_access_token(
    user_id: Uuid,
    is_admin: bool,
    secret: &str,
) -> Result<String, JwtError> {
    let now = Utc::now();
    let claims = Claims {
        sub: user_id,
        exp: (now + Duration::minutes(ACCESS_TOKEN_DURATION_MINUTES)).timestamp(),
        iat: now.timestamp(),
        is_admin,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|_| JwtError::CreationError)
}

pub fn validate_token(token: &str, secret: &str) -> Result<Claims, JwtError> {
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|e| match e.kind() {
        jsonwebtoken::errors::ErrorKind::ExpiredSignature => JwtError::Expired,
        _ => JwtError::ValidationError,
    })?;
    Ok(data.claims)
}

pub fn generate_refresh_token() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; 32] = rng.gen();
    hex::encode(bytes)
}

pub fn hash_refresh_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_validate_access_token() {
        let user_id = Uuid::new_v4();
        let secret = "test-secret-at-least-32-characters-long";
        let token = create_access_token(user_id, false, secret).unwrap();
        let claims = validate_token(&token, secret).unwrap();
        assert_eq!(claims.sub, user_id);
        assert!(!claims.is_admin);
    }

    #[test]
    fn admin_claim_roundtrips() {
        let user_id = Uuid::new_v4();
        let secret = "test-secret-at-least-32-characters-long";
        let token = create_access_token(user_id, true, secret).unwrap();
        let claims = validate_token(&token, secret).unwrap();
        assert!(claims.is_admin);
    }

    #[test]
    fn wrong_secret_fails() {
        let user_id = Uuid::new_v4();
        let token =
            create_access_token(user_id, false, "secret-one-at-least-32-characters").unwrap();
        let result = validate_token(&token, "secret-two-at-least-32-characters");
        assert!(result.is_err());
    }

    #[test]
    fn refresh_token_is_random() {
        let t1 = generate_refresh_token();
        let t2 = generate_refresh_token();
        assert_ne!(t1, t2);
        assert!(t1.len() >= 32);
    }

    #[test]
    fn refresh_token_hash_is_deterministic() {
        let token = "some-refresh-token";
        let h1 = hash_refresh_token(token);
        let h2 = hash_refresh_token(token);
        assert_eq!(h1, h2);
        assert_ne!(h1, token);
    }
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test --lib auth::jwt`
Expected: 5 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/auth/jwt.rs
git commit -m "feat: implement JWT access token and refresh token management"
```

---

### Task 5: 数据库迁移 + 连接池

**Files:**
- Create: `migrations/001_create_users.sql`
- Create: `migrations/002_create_refresh_tokens.sql`
- Modify: `src/db.rs`

- [ ] **Step 1: 创建 users 迁移**

`migrations/001_create_users.sql`:
```sql
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username VARCHAR(50) NOT NULL UNIQUE,
    email VARCHAR(255) NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    is_admin BOOLEAN NOT NULL DEFAULT false,
    feed_token VARCHAR(64) NOT NULL DEFAULT encode(gen_random_bytes(32), 'hex'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_users_email ON users (email);
```

- [ ] **Step 2: 创建 refresh_tokens 迁移**

`migrations/002_create_refresh_tokens.sql`:
```sql
CREATE TABLE refresh_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash VARCHAR(64) NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_refresh_tokens_user_id ON refresh_tokens (user_id);
CREATE INDEX idx_refresh_tokens_token_hash ON refresh_tokens (token_hash);
```

- [ ] **Step 3: 更新 src/db.rs**

```rust
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

pub async fn create_pool(database_url: &str) -> PgPool {
    PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
        .expect("failed to connect to database")
}

pub async fn run_migrations(pool: &PgPool) {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .expect("failed to run migrations");
}
```

- [ ] **Step 4: 验证编译通过**

Run: `cargo check`
Expected: 编译通过

- [ ] **Step 5: Commit**

```bash
git add migrations/ src/db.rs
git commit -m "feat: add database migrations for users and refresh_tokens"
```

---

### Task 6: API 错误类型

**Files:**
- Modify: `src/api/error.rs`

- [ ] **Step 1: 实现 API 错误类型**

```rust
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    Unauthorized(String),
    Forbidden(String),
    NotFound(String),
    Conflict(String),
    Internal(String),
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
    message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_type, message) = match self {
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "bad_request", msg),
            ApiError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, "unauthorized", msg),
            ApiError::Forbidden(msg) => (StatusCode::FORBIDDEN, "forbidden", msg),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, "not_found", msg),
            ApiError::Conflict(msg) => (StatusCode::CONFLICT, "conflict", msg),
            ApiError::Internal(msg) => {
                tracing::error!("internal error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "internal server error".to_string(),
                )
            }
        };

        let body = ErrorBody {
            error: error_type.to_string(),
            message,
        };

        (status, axum::Json(body)).into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        ApiError::Internal(e.to_string())
    }
}
```

- [ ] **Step 2: 验证编译通过**

Run: `cargo check`

- [ ] **Step 3: Commit**

```bash
git add src/api/error.rs
git commit -m "feat: add API error types with JSON response"
```

---

### Task 7: User 模型与数据库操作

**Files:**
- Modify: `src/models/user.rs`

- [ ] **Step 1: 实现 User 模型**

```rust
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::error::ApiError;
use crate::auth::password;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub is_admin: bool,
    pub feed_token: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RefreshToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

pub async fn create_user(
    pool: &PgPool,
    username: &str,
    email: &str,
    password: &str,
    is_admin: bool,
) -> Result<User, ApiError> {
    let password_hash =
        password::hash_password(password).map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (username, email, password_hash, is_admin)
        VALUES ($1, $2, $3, $4)
        RETURNING *
        "#,
    )
    .bind(username)
    .bind(email)
    .bind(&password_hash)
    .bind(is_admin)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref db_err) if db_err.constraint().is_some() => {
            ApiError::Conflict("username or email already exists".to_string())
        }
        _ => ApiError::Internal(e.to_string()),
    })
}

pub async fn find_user_by_email(pool: &PgPool, email: &str) -> Result<Option<User>, ApiError> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
        .bind(email)
        .fetch_optional(pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn count_users(pool: &PgPool) -> Result<i64, ApiError> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(row.0)
}

pub async fn store_refresh_token(
    pool: &PgPool,
    user_id: Uuid,
    token_hash: &str,
    expires_at: DateTime<Utc>,
) -> Result<(), ApiError> {
    sqlx::query(
        "INSERT INTO refresh_tokens (user_id, token_hash, expires_at) VALUES ($1, $2, $3)",
    )
    .bind(user_id)
    .bind(token_hash)
    .bind(expires_at)
    .execute(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(())
}

pub async fn find_refresh_token(
    pool: &PgPool,
    token_hash: &str,
) -> Result<Option<RefreshToken>, ApiError> {
    sqlx::query_as::<_, RefreshToken>(
        "SELECT * FROM refresh_tokens WHERE token_hash = $1 AND expires_at > now()",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn delete_refresh_token(pool: &PgPool, token_hash: &str) -> Result<(), ApiError> {
    sqlx::query("DELETE FROM refresh_tokens WHERE token_hash = $1")
        .bind(token_hash)
        .execute(pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(())
}

pub async fn delete_user_refresh_tokens(pool: &PgPool, user_id: Uuid) -> Result<(), ApiError> {
    sqlx::query("DELETE FROM refresh_tokens WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(())
}
```

- [ ] **Step 2: 验证编译通过**

Run: `cargo check`

- [ ] **Step 3: Commit**

```bash
git add src/models/user.rs
git commit -m "feat: implement User and RefreshToken models with DB queries"
```

---

### Task 8: Auth 中间件

**Files:**
- Modify: `src/auth/middleware.rs`

- [ ] **Step 1: 实现 JWT 认证 extractor**

```rust
use axum::{
    extract::FromRequestParts,
    http::request::Parts,
};
use sqlx::PgPool;

use crate::api::error::ApiError;
use crate::auth::jwt::{self, Claims};
use crate::config::Config;

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: uuid::Uuid,
    pub is_admin: bool,
}

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Config,
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| ApiError::Unauthorized("missing authorization header".to_string()))?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or_else(|| ApiError::Unauthorized("invalid authorization format".to_string()))?;

        let claims = jwt::validate_token(token, &state.config.jwt_secret)
            .map_err(|e| ApiError::Unauthorized(e.to_string()))?;

        Ok(AuthUser {
            user_id: claims.sub,
            is_admin: claims.is_admin,
        })
    }
}
```

- [ ] **Step 2: 验证编译通过**

Run: `cargo check`

- [ ] **Step 3: Commit**

```bash
git add src/auth/middleware.rs
git commit -m "feat: implement JWT auth middleware extractor"
```

---

### Task 9: Auth API 端点

**Files:**
- Modify: `src/api/auth.rs`
- Modify: `src/api/mod.rs`

- [ ] **Step 1: 实现 auth handler**

`src/api/auth.rs`:
```rust
use axum::extract::State;
use axum::Json;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::api::error::ApiError;
use crate::auth::jwt;
use crate::auth::middleware::{AppState, AuthUser};
use crate::auth::password;
use crate::models::user;

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

#[derive(Serialize)]
pub struct MessageResponse {
    pub message: String,
}

pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    if req.username.is_empty() || req.email.is_empty() || req.password.len() < 8 {
        return Err(ApiError::BadRequest(
            "username, email required; password must be >= 8 chars".to_string(),
        ));
    }

    // First user becomes admin
    let user_count = user::count_users(&state.pool).await?;
    let is_admin = user_count == 0;

    let new_user = user::create_user(&state.pool, &req.username, &req.email, &req.password, is_admin).await?;

    issue_tokens(&state, new_user.id, new_user.is_admin).await
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    let found = user::find_user_by_email(&state.pool, &req.email)
        .await?
        .ok_or_else(|| ApiError::Unauthorized("invalid credentials".to_string()))?;

    password::verify_password(&req.password, &found.password_hash)
        .map_err(|_| ApiError::Unauthorized("invalid credentials".to_string()))?;

    issue_tokens(&state, found.id, found.is_admin).await
}

pub async fn refresh(
    State(state): State<AppState>,
    Json(req): Json<RefreshRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    let token_hash = jwt::hash_refresh_token(&req.refresh_token);

    let stored = user::find_refresh_token(&state.pool, &token_hash)
        .await?
        .ok_or_else(|| ApiError::Unauthorized("invalid refresh token".to_string()))?;

    // Delete old refresh token (rotation)
    user::delete_refresh_token(&state.pool, &token_hash).await?;

    // Find user to get current is_admin status
    let found = sqlx::query_as::<_, crate::models::user::User>(
        "SELECT * FROM users WHERE id = $1",
    )
    .bind(stored.user_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| ApiError::Unauthorized("user not found".to_string()))?;

    issue_tokens(&state, found.id, found.is_admin).await
}

pub async fn logout(
    State(state): State<AppState>,
    _auth: AuthUser,
    Json(req): Json<RefreshRequest>,
) -> Result<Json<MessageResponse>, ApiError> {
    let token_hash = jwt::hash_refresh_token(&req.refresh_token);
    user::delete_refresh_token(&state.pool, &token_hash).await?;
    Ok(Json(MessageResponse {
        message: "logged out".to_string(),
    }))
}

async fn issue_tokens(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
) -> Result<Json<AuthResponse>, ApiError> {
    let access_token = jwt::create_access_token(user_id, is_admin, &state.config.jwt_secret)
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let refresh_token_raw = jwt::generate_refresh_token();
    let refresh_token_hash = jwt::hash_refresh_token(&refresh_token_raw);
    let expires_at = Utc::now() + Duration::days(30);

    user::store_refresh_token(&state.pool, user_id, &refresh_token_hash, expires_at).await?;

    Ok(Json(AuthResponse {
        access_token,
        refresh_token: refresh_token_raw,
        token_type: "Bearer".to_string(),
        expires_in: 900, // 15 minutes in seconds
    }))
}
```

- [ ] **Step 2: 更新路由**

`src/api/mod.rs`:
```rust
use axum::{routing::post, Router};
use sqlx::PgPool;

use crate::auth::middleware::AppState;
use crate::config::Config;

pub mod auth;
pub mod error;

pub fn router(pool: PgPool, config: Config) -> Router {
    let state = AppState { pool, config };

    Router::new()
        .route("/api/auth/register", post(auth::register))
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/refresh", post(auth::refresh))
        .route("/api/auth/logout", post(auth::logout))
        .with_state(state)
}
```

- [ ] **Step 3: 验证编译通过**

Run: `cargo check`

- [ ] **Step 4: Commit**

```bash
git add src/api/auth.rs src/api/mod.rs
git commit -m "feat: implement auth API endpoints (register, login, refresh, logout)"
```

---

### Task 10: 集成测试

**Files:**
- Create: `tests/common/mod.rs`
- Create: `tests/integration_auth.rs`

- [ ] **Step 1: 创建测试辅助模块**

`tests/common/mod.rs`:
```rust
use lettura::auth::middleware::AppState;
use lettura::config::Config;
use sqlx::PgPool;
use uuid::Uuid;

pub struct TestApp {
    pub addr: String,
    pub pool: PgPool,
    pub client: reqwest::Client,
    pub db_name: String,
}

impl TestApp {
    pub async fn new() -> Self {
        let base_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://lettura:lettura@127.0.0.1:5432/lettura".to_string());

        // Create a unique test database
        let db_name = format!("lettura_test_{}", Uuid::new_v4().simple());
        let base_pool = PgPool::connect(&base_url).await.unwrap();
        sqlx::query(&format!("CREATE DATABASE \"{}\"", db_name))
            .execute(&base_pool)
            .await
            .unwrap();
        base_pool.close().await;

        // Connect to test database
        let test_url = base_url.rsplit_once('/').unwrap().0;
        let test_db_url = format!("{}/{}", test_url, db_name);
        let pool = PgPool::connect(&test_db_url).await.unwrap();

        // Run migrations
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        let config = Config {
            database_url: test_db_url,
            jwt_secret: "test-secret-at-least-32-characters-long-for-testing".to_string(),
            listen_addr: "127.0.0.1:0".to_string(),
        };

        let app = lettura::api::router(pool.clone(), config.clone());
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
        }
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.addr, path)
    }
}

impl Drop for TestApp {
    fn drop(&mut self) {
        // Database cleanup happens at end of test via Drop
        // In practice, test DBs are cleaned up by subsequent test runs or CI
    }
}
```

- [ ] **Step 2: 创建认证集成测试**

`tests/integration_auth.rs`:
```rust
mod common;

use serde_json::json;

#[tokio::test]
async fn register_first_user_becomes_admin() {
    let app = common::TestApp::new().await;

    let res = app
        .client
        .post(app.url("/api/auth/register"))
        .json(&json!({
            "username": "admin",
            "email": "admin@example.com",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["access_token"].is_string());
    assert!(body["refresh_token"].is_string());
    assert_eq!(body["token_type"], "Bearer");
}

#[tokio::test]
async fn register_duplicate_email_fails() {
    let app = common::TestApp::new().await;

    app.client
        .post(app.url("/api/auth/register"))
        .json(&json!({
            "username": "user1",
            "email": "dup@example.com",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();

    let res = app
        .client
        .post(app.url("/api/auth/register"))
        .json(&json!({
            "username": "user2",
            "email": "dup@example.com",
            "password": "password456"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 409);
}

#[tokio::test]
async fn login_with_valid_credentials() {
    let app = common::TestApp::new().await;

    app.client
        .post(app.url("/api/auth/register"))
        .json(&json!({
            "username": "testuser",
            "email": "test@example.com",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();

    let res = app
        .client
        .post(app.url("/api/auth/login"))
        .json(&json!({
            "email": "test@example.com",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["access_token"].is_string());
}

#[tokio::test]
async fn login_with_wrong_password_fails() {
    let app = common::TestApp::new().await;

    app.client
        .post(app.url("/api/auth/register"))
        .json(&json!({
            "username": "testuser",
            "email": "test@example.com",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();

    let res = app
        .client
        .post(app.url("/api/auth/login"))
        .json(&json!({
            "email": "test@example.com",
            "password": "wrongpassword"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn refresh_token_rotates() {
    let app = common::TestApp::new().await;

    let res = app
        .client
        .post(app.url("/api/auth/register"))
        .json(&json!({
            "username": "testuser",
            "email": "test@example.com",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    let refresh_token = body["refresh_token"].as_str().unwrap();

    // Use refresh token
    let res = app
        .client
        .post(app.url("/api/auth/refresh"))
        .json(&json!({ "refresh_token": refresh_token }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body2: serde_json::Value = res.json().await.unwrap();
    let new_refresh = body2["refresh_token"].as_str().unwrap();
    assert_ne!(refresh_token, new_refresh, "refresh token should rotate");

    // Old refresh token should no longer work
    let res = app
        .client
        .post(app.url("/api/auth/refresh"))
        .json(&json!({ "refresh_token": refresh_token }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn logout_revokes_refresh_token() {
    let app = common::TestApp::new().await;

    let res = app
        .client
        .post(app.url("/api/auth/register"))
        .json(&json!({
            "username": "testuser",
            "email": "test@example.com",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    let access_token = body["access_token"].as_str().unwrap();
    let refresh_token = body["refresh_token"].as_str().unwrap();

    // Logout
    let res = app
        .client
        .post(app.url("/api/auth/logout"))
        .header("Authorization", format!("Bearer {}", access_token))
        .json(&json!({ "refresh_token": refresh_token }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);

    // Refresh token should no longer work
    let res = app
        .client
        .post(app.url("/api/auth/refresh"))
        .json(&json!({ "refresh_token": refresh_token }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn short_password_rejected() {
    let app = common::TestApp::new().await;

    let res = app
        .client
        .post(app.url("/api/auth/register"))
        .json(&json!({
            "username": "testuser",
            "email": "test@example.com",
            "password": "short"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 400);
}
```

- [ ] **Step 3: 启动测试 PostgreSQL 并运行集成测试**

先在远程启动 PG:
```bash
cd ~/workspace/lettura && docker compose up -d postgres
```

然后运行集成测试:
```bash
docker run --rm --network=host \
  -v "$HOME/workspace/lettura":/app \
  -v lettura-cargo-registry:/usr/local/cargo/registry \
  -v lettura-cargo-target:/app/target \
  -w /app \
  -e DATABASE_URL=postgres://lettura:lettura@127.0.0.1:5432/lettura \
  rust:1.87-slim \
  cargo test --test integration_auth 2>&1
```

Expected: 7 tests PASS

- [ ] **Step 4: 运行全部测试（单元 + 集成）**

```bash
docker run --rm --network=host \
  -v "$HOME/workspace/lettura":/app \
  -v lettura-cargo-registry:/usr/local/cargo/registry \
  -v lettura-cargo-target:/app/target \
  -w /app \
  -e DATABASE_URL=postgres://lettura:lettura@127.0.0.1:5432/lettura \
  rust:1.87-slim \
  cargo test 2>&1
```

Expected: 所有单元测试 (26 existing + 8 new) + 7 集成测试 PASS

- [ ] **Step 5: Commit**

```bash
git add tests/
git commit -m "feat: add auth integration tests (register, login, refresh, logout)"
```
