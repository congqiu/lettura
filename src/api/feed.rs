use axum::extract::{Path, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};

use crate::api::error::ApiError;
use crate::state::AppState;

pub async fn feed_unread(
    State(state): State<AppState>,
    Path(user_token): Path<String>,
) -> Result<Response, ApiError> {
    let user = find_user_by_feed_token(&state, &user_token).await?;
    let entries: Vec<FeedEntry> = sqlx::query_as(
        "SELECT id, url, title, content, created_at FROM entries WHERE user_id = $1 AND is_archived = false AND deleted_at IS NULL ORDER BY created_at DESC LIMIT 50",
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(build_rss("Lettura - Unread", &entries))
}

pub async fn feed_starred(
    State(state): State<AppState>,
    Path(user_token): Path<String>,
) -> Result<Response, ApiError> {
    let user = find_user_by_feed_token(&state, &user_token).await?;
    let entries: Vec<FeedEntry> = sqlx::query_as(
        "SELECT id, url, title, content, created_at FROM entries WHERE user_id = $1 AND is_starred = true AND deleted_at IS NULL ORDER BY starred_at DESC LIMIT 50",
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(build_rss("Lettura - Starred", &entries))
}

pub async fn feed_archive(
    State(state): State<AppState>,
    Path(user_token): Path<String>,
) -> Result<Response, ApiError> {
    let user = find_user_by_feed_token(&state, &user_token).await?;
    let entries: Vec<FeedEntry> = sqlx::query_as(
        "SELECT id, url, title, content, created_at FROM entries WHERE user_id = $1 AND is_archived = true AND deleted_at IS NULL ORDER BY archived_at DESC LIMIT 50",
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(build_rss("Lettura - Archive", &entries))
}

#[derive(sqlx::FromRow)]
struct FeedEntry {
    id: uuid::Uuid,
    url: String,
    title: Option<String>,
    content: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

async fn find_user_by_feed_token(
    state: &AppState,
    token: &str,
) -> Result<crate::models::user::User, ApiError> {
    sqlx::query_as::<_, crate::models::user::User>("SELECT * FROM users WHERE feed_token = $1")
        .bind(token)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("invalid feed token".to_string()))
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn build_rss(channel_title: &str, entries: &[FeedEntry]) -> Response {
    let mut items = String::new();
    for entry in entries {
        let title = entry.title.as_deref().unwrap_or("Untitled");
        let content = entry.content.as_deref().unwrap_or("");
        let date = entry.created_at.to_rfc2822();
        let escaped_url = xml_escape(&entry.url);
        // Escape ]]> inside CDATA sections to prevent injection
        let safe_title = title.replace("]]>", "]]&gt;");
        let safe_content = content.replace("]]>", "]]&gt;");
        items.push_str(&format!(
            "<item><title><![CDATA[{}]]></title><link>{}</link><guid>{}</guid><pubDate>{}</pubDate><description><![CDATA[{}]]></description></item>",
            safe_title, escaped_url, entry.id, date, safe_content
        ));
    }

    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><rss version="2.0"><channel><title>{}</title><description>Lettura RSS Feed</description>{}</channel></rss>"#,
        channel_title, items
    );

    (
        [
            (header::CONTENT_TYPE, "application/rss+xml; charset=utf-8"),
            (header::REFERRER_POLICY, "no-referrer"),
        ],
        xml,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    fn make_entry(
        id: &str,
        url: &str,
        title: Option<&str>,
        content: Option<&str>,
        created_at: chrono::DateTime<chrono::Utc>,
    ) -> FeedEntry {
        FeedEntry {
            id: uuid::Uuid::parse_str(id).unwrap(),
            url: url.to_string(),
            title: title.map(|s| s.to_string()),
            content: content.map(|s| s.to_string()),
            created_at,
        }
    }

    // --- xml_escape tests ---

    #[test]
    fn xml_escape_ampersand() {
        assert_eq!(xml_escape("&"), "&amp;");
    }

    #[test]
    fn xml_escape_less_than() {
        assert_eq!(xml_escape("<"), "&lt;");
    }

    #[test]
    fn xml_escape_greater_than() {
        assert_eq!(xml_escape(">"), "&gt;");
    }

    #[test]
    fn xml_escape_double_quote() {
        assert_eq!(xml_escape("\""), "&quot;");
    }

    #[test]
    fn xml_escape_single_quote() {
        assert_eq!(xml_escape("'"), "&apos;");
    }

    #[test]
    fn xml_escape_no_special_chars() {
        assert_eq!(xml_escape("hello world"), "hello world");
    }

    #[test]
    fn xml_escape_multiple_special_chars() {
        assert_eq!(
            xml_escape("a&b<c>d\"e'f"),
            "a&amp;b&lt;c&gt;d&quot;e&apos;f"
        );
    }

    #[test]
    fn xml_escape_empty_string() {
        assert_eq!(xml_escape(""), "");
    }

    // --- build_rss tests ---

    #[tokio::test]
    async fn build_rss_empty_entries() {
        let response = build_rss("Lettura - Unread", &[]);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let xml = String::from_utf8(body.to_vec()).unwrap();
        assert!(xml.contains("<title>Lettura - Unread</title>"));
        assert!(xml.contains("<rss version=\"2.0\">"));
        assert!(!xml.contains("<item>"));
    }

    #[tokio::test]
    async fn build_rss_single_entry() {
        let now = chrono::Utc::now();
        let entry = make_entry(
            "550e8400-e29b-41d4-a716-446655440000",
            "https://example.com/article",
            Some("Test Article"),
            Some("<p>Hello</p>"),
            now,
        );
        let response = build_rss("Lettura - Unread", &[entry]);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let xml = String::from_utf8(body.to_vec()).unwrap();
        assert!(xml.contains("<item>"));
        assert!(xml.contains("<title><![CDATA[Test Article]]></title>"));
        assert!(xml.contains("<link>https://example.com/article</link>"));
        assert!(xml.contains("<guid>550e8400-e29b-41d4-a716-446655440000</guid>"));
        assert!(xml.contains("<pubDate>"));
        assert!(xml.contains("<description><![CDATA[<p>Hello</p>]]></description>"));
    }

    #[tokio::test]
    async fn build_rss_cdata_injection_title() {
        let now = chrono::Utc::now();
        let entry = make_entry(
            "550e8400-e29b-41d4-a716-446655440001",
            "https://example.com/xss",
            Some("Evil ]]> Title"),
            None,
            now,
        );
        let response = build_rss("Feed", &[entry]);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let xml = String::from_utf8(body.to_vec()).unwrap();
        assert!(xml.contains("Evil ]]&gt; Title"));
        assert!(!xml.contains("Evil ]]> Title"));
    }

    #[tokio::test]
    async fn build_rss_cdata_injection_content() {
        let now = chrono::Utc::now();
        let entry = make_entry(
            "550e8400-e29b-41d4-a716-446655440002",
            "https://example.com/xss2",
            Some("Safe Title"),
            Some("Evil ]]> Content"),
            now,
        );
        let response = build_rss("Feed", &[entry]);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let xml = String::from_utf8(body.to_vec()).unwrap();
        assert!(xml.contains("Evil ]]&gt; Content"));
        assert!(!xml.contains("Evil ]]> Content"));
    }

    #[tokio::test]
    async fn build_rss_date_rfc2822_format() {
        let now = chrono::Utc::now();
        let entry = make_entry(
            "550e8400-e29b-41d4-a716-446655440003",
            "https://example.com/date-test",
            Some("Date Test"),
            None,
            now,
        );
        let response = build_rss("Feed", &[entry]);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let xml = String::from_utf8(body.to_vec()).unwrap();
        // Extract pubDate content and verify it parses as RFC2822
        let start = xml.find("<pubDate>").unwrap() + "<pubDate>".len();
        let end = xml.find("</pubDate>").unwrap();
        let date_str = &xml[start..end];
        // chrono's to_rfc2822 produces parseable RFC2822 dates
        let parsed = chrono::DateTime::parse_from_rfc2822(date_str);
        assert!(
            parsed.is_ok(),
            "pubDate '{}' is not valid RFC2822",
            date_str
        );
    }
}
