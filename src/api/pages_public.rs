use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use serde::Deserialize;
use sqlx;

use crate::state::AppState;
use crate::models::page;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Deserialize)]
pub struct ShareQueryParams {
    #[serde(rename = "p")]
    password: Option<String>,
}

fn sign_cookie(jwt_secret: &str, slug: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(jwt_secret.as_bytes())
        .expect("HMAC key must be valid (JWT secret is always non-empty)");
    mac.update(slug.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn verify_cookie(jwt_secret: &str, slug: &str, value: &str) -> bool {
    let mut mac = HmacSha256::new_from_slice(jwt_secret.as_bytes())
        .expect("HMAC key must be valid (JWT secret is always non-empty)");
    mac.update(slug.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());
    expected == value
}

fn get_cookie_value(headers: &HeaderMap, slug: &str) -> Option<String> {
    let cookie_name = format!("page_auth_{}", slug);
    headers.get("cookie").and_then(|v| v.to_str().ok()).and_then(|cookies| {
        cookies.split(';')
            .map(|c| c.trim())
            .find(|c| c.starts_with(&format!("{}=", cookie_name)))
            .map(|c| c[cookie_name.len() + 1..].to_string())
    })
}

pub async fn serve_page(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Query(params): Query<ShareQueryParams>,
    headers: HeaderMap,
) -> Response {
    serve_page_file_inner(&state, &slug, None, params.password.as_deref(), &headers).await
}

pub async fn serve_page_file(
    State(state): State<AppState>,
    Path((slug, file)): Path<(String, String)>,
    Query(params): Query<ShareQueryParams>,
    headers: HeaderMap,
) -> Response {
    serve_page_file_inner(&state, &slug, Some(&file), params.password.as_deref(), &headers).await
}

async fn serve_page_file_inner(
    state: &AppState,
    slug: &str,
    sub_path: Option<&str>,
    query_password: Option<&str>,
    headers: &HeaderMap,
) -> Response {
    let page_record = match page::find_page_by_slug(&state.pool, slug).await {
        Ok(Some(p)) => p,
        _ => return render_status_page(StatusCode::NOT_FOUND, "页面不存在", "该分享链接无效或已被删除"),
    };

    if let Some(expires) = page_record.expires_at {
        if expires < chrono::Utc::now() {
            return render_status_page(StatusCode::GONE, "分享已过期", "该页面的分享有效期已结束");
        }
    }

    if page_record.password.is_some() {
        let (authenticated, needs_upgrade) = if let Some(pw) = query_password {
            let stored = page_record.password.as_ref().expect("password is Some when is_some() is true");
            let ok = crate::auth::password::verify_page_password(pw, stored).is_ok();
            // Lazy upgrade: mark if stored password is plaintext and auth succeeded
            (ok, ok && !stored.starts_with("$argon2"))
        } else {
            (get_cookie_value(headers, slug)
                .map(|v| verify_cookie(&state.config.jwt_secret, slug, &v))
                .unwrap_or(false), false)
        };
        if !authenticated {
            return render_password_page(slug, false);
        }
        // Lazy upgrade: hash plaintext passwords on successful authentication
        if needs_upgrade {
            if let Some(pw) = query_password {
                if let Ok(hashed) = crate::auth::password::hash_page_password(pw) {
                    let _ = sqlx::query("UPDATE pages SET password = $1 WHERE id = $2")
                        .bind(&hashed).bind(page_record.id)
                        .execute(&state.pool).await;
                }
            }
        }
    }

    let file_name = match sub_path {
        Some(p) => p,
        None => &page_record.entry_file,
    };

    // Reject path traversal attempts. Axum's Path extractor already decodes
    // percent-encoding, so %2e%2e becomes ".." and is caught here.
    if file_name.contains("..") || file_name.contains('\0') || file_name.starts_with('/') {
        return (StatusCode::FORBIDDEN, "forbidden").into_response();
    }

    let key = format!("pages/{}/{}", slug, file_name);

    if state.config.storage_type == "local" {
        let base_path = std::path::PathBuf::from(&state.config.pages_storage_path).join(slug);
        let canonical_base = match std::fs::canonicalize(&base_path) {
            Ok(p) => p,
            Err(_) => return (StatusCode::NOT_FOUND, "not found").into_response(),
        };
        let file_path = canonical_base.join(file_name);
        match std::fs::canonicalize(&file_path) {
            Ok(canonical_file) if canonical_file.starts_with(&canonical_base) => {
                match tokio::fs::read(&canonical_file).await {
                    Ok(data) => {
                        let mime = mime_for_file(file_name);
                        (StatusCode::OK, [("content-type", mime)], data).into_response()
                    }
                    Err(_) => (StatusCode::NOT_FOUND, "not found").into_response(),
                }
            }
            _ => (StatusCode::FORBIDDEN, "forbidden").into_response(),
        }
    } else {
        match state.storage.get(&key).await {
            Ok(Some(data)) => {
                let mime = mime_for_file(file_name);
                (StatusCode::OK, [("content-type", mime)], data).into_response()
            }
            _ => (StatusCode::NOT_FOUND, "not found").into_response(),
        }
    }
}

#[derive(serde::Deserialize)]
pub struct AuthRequest {
    password: String,
}

pub async fn auth_page(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    axum::Form(form): axum::Form<AuthRequest>,
) -> Response {
    let page_record = match page::find_page_by_slug(&state.pool, &slug).await {
        Ok(Some(p)) => p,
        _ => return (StatusCode::NOT_FOUND, "not found").into_response(),
    };

    match &page_record.password {
        Some(stored_password) => {
            if crate::auth::password::verify_page_password(&form.password, stored_password).is_ok() {
                // Lazy upgrade: if the stored password is plaintext, hash it now
                if !stored_password.starts_with("$argon2") {
                    if let Ok(hashed) = crate::auth::password::hash_page_password(&form.password) {
                        let _ = sqlx::query("UPDATE pages SET password = $1 WHERE id = $2")
                            .bind(&hashed).bind(page_record.id)
                            .execute(&state.pool).await;
                    }
                }
                let sig = sign_cookie(&state.config.jwt_secret, &slug);
                let cookie = format!(
                    "page_auth_{}={}; Path=/p/{}; Max-Age=86400; HttpOnly; SameSite=Lax",
                    slug, sig, slug
                );
                (
                    StatusCode::FOUND,
                    [
                        ("location", format!("/p/{}", slug)),
                        ("set-cookie", cookie),
                    ],
                ).into_response()
            } else {
                render_password_page(&slug, true)
            }
        }
        None => (
            StatusCode::FOUND,
            [("location", format!("/p/{}", slug))],
        ).into_response(),
    }
}

fn render_status_page(status: StatusCode, title: &str, message: &str) -> Response {
    let status_code = status.as_u16();
    let html = format!(
        r#"<!DOCTYPE html>
<html lang="zh"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>{title}</title>
<style>
body{{font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif;display:flex;justify-content:center;align-items:center;min-height:100vh;margin:0;background:#f9fafb;color:#111827}}
.card{{background:#fff;border-radius:12px;box-shadow:0 1px 3px rgba(0,0,0,.1);padding:32px;width:100%;max-width:360px;text-align:center}}
.icon{{width:48px;height:48px;border-radius:50%;margin:0 auto 16px;display:flex;align-items:center;justify-content:center}}
.icon-missing{{background:#f3f4f6}}
.icon-expired{{background:#fef3c7}}
h1{{font-size:18px;font-weight:600;margin:0 0 8px}}
p{{font-size:14px;color:#6b7280;margin:0}}
.code{{font-size:12px;color:#9ca3af;margin-top:16px}}
</style></head><body>
<div class="card">
<div class="icon {}">{}</div>
<h1>{title}</h1>
<p>{message}</p>
<p class="code">{status_code}</p>
</div></body></html>"#,
        if status_code == 410 { "icon-expired" } else { "icon-missing" },
        if status_code == 410 {
            r##"<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#d97706" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>"##
        } else {
            r##"<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#6b7280" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/><line x1="8" y1="11" x2="14" y2="11"/></svg>"##
        },
        title = title,
        message = message,
        status_code = status_code,
    );
    (
        status,
        [("content-type", "text/html; charset=utf-8")],
        html,
    ).into_response()
}

fn render_password_page(slug: &str, error: bool) -> Response {
    let error_html = if error {
        r#"<p style="color:#ef4444;margin-top:8px;font-size:14px;">密码错误</p>"#
    } else {
        ""
    };
    let html = format!(
        r#"<!DOCTYPE html>
<html lang="zh"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>需要密码</title>
<style>
body{{font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif;display:flex;justify-content:center;align-items:center;min-height:100vh;margin:0;background:#f9fafb;color:#111827}}
.card{{background:#fff;border-radius:12px;box-shadow:0 1px 3px rgba(0,0,0,.1);padding:32px;width:100%;max-width:360px;text-align:center}}
h1{{font-size:18px;font-weight:600;margin:0 0 16px}}
input[type=password]{{width:100%;padding:10px 12px;border:1px solid #d1d5db;border-radius:8px;font-size:15px;box-sizing:border-box;outline:none}}
input[type=password]:focus{{border-color:#3b82f6;box-shadow:0 0 0 3px rgba(59,130,246,.1)}}
button{{margin-top:12px;width:100%;padding:10px;background:#3b82f6;color:#fff;border:none;border-radius:8px;font-size:15px;font-weight:500;cursor:pointer}}
button:hover{{background:#2563eb}}
</style></head><body>
<div class="card"><h1>此页面需要密码</h1>
<form method="POST" action="/p/{}/auth">
<input type="password" name="password" placeholder="请输入密码" autofocus required>{}
<button type="submit">确认</button>
</form></div></body></html>"#,
        slug, error_html
    );
    (
        StatusCode::OK,
        [("content-type", "text/html; charset=utf-8")],
        html,
    ).into_response()
}

fn mime_for_file(name: &str) -> &'static str {
    match name.rsplit('.').next().unwrap_or("") {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "application/javascript",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "eot" => "application/vnd.ms-fontobject",
        "webmanifest" => "application/manifest+json",
        "xml" => "application/xml",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        _ => "application/octet-stream",
    }
}
