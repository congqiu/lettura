use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use chrono::Utc;

use crate::state::AppState;
use crate::models::page;

type HmacSha256 = Hmac<Sha256>;

fn sign_cookie(jwt_secret: &str, slug: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(jwt_secret.as_bytes()).unwrap();
    mac.update(slug.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn verify_cookie(jwt_secret: &str, slug: &str, value: &str) -> bool {
    let mut mac = HmacSha256::new_from_slice(jwt_secret.as_bytes()).unwrap();
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
    headers: HeaderMap,
) -> Response {
    serve_page_file_inner(&state, &slug, None, &headers).await
}

pub async fn serve_page_file(
    State(state): State<AppState>,
    Path((slug, file)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    serve_page_file_inner(&state, &slug, Some(&file), &headers).await
}

async fn serve_page_file_inner(
    state: &AppState,
    slug: &str,
    sub_path: Option<&str>,
    headers: &HeaderMap,
) -> Response {
    let page_record = match page::find_page_by_slug(&state.pool, slug).await {
        Ok(Some(p)) => p,
        _ => return (StatusCode::NOT_FOUND, "not found").into_response(),
    };

    if let Some(expires) = page_record.expires_at {
        if expires < chrono::Utc::now() {
            return (StatusCode::GONE, "this page has expired").into_response();
        }
    }

    if page_record.password.is_some() {
        let authenticated = get_cookie_value(headers, slug)
            .map(|v| verify_cookie(&state.config.jwt_secret, slug, &v))
            .unwrap_or(false);
        if !authenticated {
            return render_password_page(slug, false);
        }
    }

    let file_name = match sub_path {
        Some(p) => p,
        None => &page_record.entry_file,
    };

    if file_name.contains("..") || file_name.contains('\0') {
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
        Some(hash) => {
            if crate::auth::password::verify_password(&form.password, hash).is_ok() {
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
