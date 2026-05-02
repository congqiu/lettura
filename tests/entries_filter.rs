mod common;
use common::TestApp;
use lettura::models::entry::{self, ListParams};
use serde_json::json;

async fn create_test_entry_with_domain(
    pool: &sqlx::PgPool,
    user_id: uuid::Uuid,
    url: &str,
) -> uuid::Uuid {
    let entry = entry::create_entry(pool, user_id, url).await.unwrap();
    entry.id
}

async fn add_tag(
    pool: &sqlx::PgPool,
    user_id: uuid::Uuid,
    entry_id: uuid::Uuid,
    label: &str,
) {
    let (tag_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO tags (user_id, label, slug) VALUES ($1,$2,$3) \
         ON CONFLICT (user_id, slug) DO UPDATE SET label = EXCLUDED.label RETURNING id",
    )
    .bind(user_id)
    .bind(label)
    .bind(label.to_lowercase())
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO entry_tags (entry_id, tag_id) VALUES ($1,$2) ON CONFLICT DO NOTHING",
    )
    .bind(entry_id)
    .bind(tag_id)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn filter_untagged_returns_only_entries_without_tags() {
    let app = TestApp::new().await;
    let (user_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO users (username, email, password_hash) \
         VALUES ('u','u@e.com','x') RETURNING id",
    )
    .fetch_one(&app.pool)
    .await
    .unwrap();
    let e1 = create_test_entry_with_domain(&app.pool, user_id, "https://a.test").await;
    let _e2 = create_test_entry_with_domain(&app.pool, user_id, "https://b.test").await;
    add_tag(&app.pool, user_id, e1, "tech").await;
    // e1 has tag, e2 is untagged

    let params = ListParams {
        page: None,
        per_page: None,
        is_archived: None,
        is_starred: None,
        is_read: None,
        domain: None,
        tag: None,
        exclude_tag: None,
        untagged: Some(true),
        since: None,
        before: None,
        search: None,
        fields: None,
        cursor: None,
    };
    let res = entry::list_entries(&app.pool, user_id, &params)
        .await
        .unwrap();
    // Only e2 has no tags
    assert_eq!(res.len(), 1, "expected 1 untagged entry, got {}", res.len());
    app.cleanup().await;
}

#[tokio::test]
async fn filter_by_tag_requires_match() {
    let app = TestApp::new().await;
    let (user_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO users (username, email, password_hash) \
         VALUES ('u','u@e.com','x') RETURNING id",
    )
    .fetch_one(&app.pool)
    .await
    .unwrap();
    let e1 = create_test_entry_with_domain(&app.pool, user_id, "https://a.test").await;
    let _e2 = create_test_entry_with_domain(&app.pool, user_id, "https://b.test").await;
    add_tag(&app.pool, user_id, e1, "tech").await;

    let mut params = ListParams {
        page: None,
        per_page: None,
        is_archived: None,
        is_starred: None,
        is_read: None,
        domain: None,
        tag: Some("tech".into()),
        exclude_tag: None,
        untagged: None,
        since: None,
        before: None,
        search: None,
        fields: None,
        cursor: None,
    };
    let res = entry::list_entries(&app.pool, user_id, &params)
        .await
        .unwrap();
    assert_eq!(res.len(), 1);
    assert_eq!(res[0].id, e1);

    // multi-tag AND semantics
    add_tag(&app.pool, user_id, e1, "rust").await;
    params.tag = Some("tech,rust".into());
    let res = entry::list_entries(&app.pool, user_id, &params)
        .await
        .unwrap();
    assert_eq!(res.len(), 1);

    params.tag = Some("tech,nonexistent".into());
    let res = entry::list_entries(&app.pool, user_id, &params)
        .await
        .unwrap();
    assert_eq!(res.len(), 0);

    app.cleanup().await;
}

#[tokio::test]
async fn filter_exclude_tag() {
    let app = TestApp::new().await;
    let (user_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO users (username, email, password_hash) \
         VALUES ('u','u@e.com','x') RETURNING id",
    )
    .fetch_one(&app.pool)
    .await
    .unwrap();
    let e1 = create_test_entry_with_domain(&app.pool, user_id, "https://a.test").await;
    let _e2 = create_test_entry_with_domain(&app.pool, user_id, "https://b.test").await;
    add_tag(&app.pool, user_id, e1, "archive").await;

    let params = ListParams {
        page: None,
        per_page: None,
        is_archived: None,
        is_starred: None,
        is_read: None,
        domain: None,
        tag: None,
        exclude_tag: Some("archive".into()),
        untagged: None,
        since: None,
        before: None,
        search: None,
        fields: None,
        cursor: None,
    };
    let res = entry::list_entries(&app.pool, user_id, &params)
        .await
        .unwrap();
    assert_eq!(res.len(), 1);
    assert!(!res.iter().any(|e| e.id == e1));
    app.cleanup().await;
}

#[tokio::test]
async fn filter_since_and_before() {
    let app = TestApp::new().await;
    let (user_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO users (username, email, password_hash) \
         VALUES ('u','u@e.com','x') RETURNING id",
    )
    .fetch_one(&app.pool)
    .await
    .unwrap();
    let e = create_test_entry_with_domain(&app.pool, user_id, "https://a.test").await;
    // Backdate this entry's created_at
    sqlx::query(
        "UPDATE entries SET created_at = now() - INTERVAL '100 days' WHERE id = $1",
    )
    .bind(e)
    .execute(&app.pool)
    .await
    .unwrap();
    let _recent = create_test_entry_with_domain(&app.pool, user_id, "https://b.test").await;

    // since=7d should only show recent
    let params = ListParams {
        page: None,
        per_page: None,
        is_archived: None,
        is_starred: None,
        is_read: None,
        domain: None,
        tag: None,
        exclude_tag: None,
        untagged: None,
        since: Some(chrono::Utc::now() - chrono::Duration::days(7)),
        before: None,
        search: None,
        fields: None,
        cursor: None,
    };
    let res = entry::list_entries(&app.pool, user_id, &params)
        .await
        .unwrap();
    assert_eq!(res.len(), 1);

    // before=30d should only show old one
    let params = ListParams {
        page: None,
        per_page: None,
        is_archived: None,
        is_starred: None,
        is_read: None,
        domain: None,
        tag: None,
        exclude_tag: None,
        untagged: None,
        since: None,
        before: Some(chrono::Utc::now() - chrono::Duration::days(30)),
        search: None,
        fields: None,
        cursor: None,
    };
    let res = entry::list_entries(&app.pool, user_id, &params)
        .await
        .unwrap();
    assert_eq!(res.len(), 1);
    app.cleanup().await;
}

#[tokio::test]
async fn is_read_is_alias_for_is_archived() {
    let app = TestApp::new().await;
    let (user_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO users (username, email, password_hash) \
         VALUES ('u','u@e.com','x') RETURNING id",
    )
    .fetch_one(&app.pool)
    .await
    .unwrap();
    let e1 = create_test_entry_with_domain(&app.pool, user_id, "https://a.test").await;
    let _e2 = create_test_entry_with_domain(&app.pool, user_id, "https://b.test").await;
    sqlx::query("UPDATE entries SET is_archived = true WHERE id = $1")
        .bind(e1)
        .execute(&app.pool)
        .await
        .unwrap();

    let params = ListParams {
        page: None,
        per_page: None,
        is_archived: None,
        is_starred: None,
        is_read: Some(true), // should find archived entries
        domain: None,
        tag: None,
        exclude_tag: None,
        untagged: None,
        since: None,
        before: None,
        search: None,
        fields: None,
        cursor: None,
    };
    let res = entry::list_entries(&app.pool, user_id, &params)
        .await
        .unwrap();
    assert_eq!(res.len(), 1);
    assert_eq!(res[0].id, e1);
    app.cleanup().await;
}

#[tokio::test]
async fn list_rejects_excessive_page() {
    let app = TestApp::new().await;
    app.client.post(app.url("/api/v1/auth/register"))
        .json(&json!({"username":"tester","email":"t@e.com","password":"password123"}))
        .send().await.unwrap();
    let login: serde_json::Value = app.client.post(app.url("/api/v1/auth/login"))
        .json(&json!({"email":"t@e.com","password":"password123"}))
        .send().await.unwrap().json().await.unwrap();
    let token = login["access_token"].as_str().unwrap();

    // page=51 should return 400
    let res = app.client.get(app.url("/api/v1/entries?page=51&per_page=100"))
        .header("Authorization", format!("Bearer {}", token))
        .send().await.unwrap();
    assert_eq!(res.status(), 400, "page=51 should be rejected");

    // page=50 is the boundary, must succeed
    let res = app.client.get(app.url("/api/v1/entries?page=50&per_page=100"))
        .header("Authorization", format!("Bearer {}", token))
        .send().await.unwrap();
    assert_eq!(res.status(), 200, "page=50 is the boundary, must succeed");

    app.cleanup().await;
}

#[tokio::test]
async fn list_with_cursor_returns_next_page_via_header() {
    let app = TestApp::new().await;
    let token = {
        app.client.post(app.url("/api/v1/auth/register"))
            .json(&json!({"username":"cursor1","email":"c1@e.com","password":"password123"}))
            .send().await.unwrap();
        let login: serde_json::Value = app.client.post(app.url("/api/v1/auth/login"))
            .json(&json!({"email":"c1@e.com","password":"password123"}))
            .send().await.unwrap().json().await.unwrap();
        login["access_token"].as_str().unwrap().to_string()
    };

    // Save 5 entries
    for i in 0..5 {
        app.client.post(app.url("/api/v1/entries"))
            .header("Authorization", format!("Bearer {}", token))
            .json(&json!({"url": format!("https://example.com/{}", i)}))
            .send().await.unwrap();
    }

    // First request: per_page=2, no cursor
    let res = app.client.get(app.url("/api/v1/entries?per_page=2"))
        .header("Authorization", format!("Bearer {}", token))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let next_header_first = res.headers().get("x-next-cursor").map(|v| v.to_str().unwrap().to_string());
    let first: Vec<serde_json::Value> = res.json().await.unwrap();
    assert_eq!(first.len(), 2);
    // First page should have next_cursor since there are more entries
    assert!(next_header_first.is_some(), "X-Next-Cursor must be present on first page when there are more entries");

    // Now request with cursor=<last entry's encoded cursor>. Construct one
    // from the last item's created_at + id (manual %3A to avoid a dep).
    let last = &first[1];
    let last_id = last["id"].as_str().unwrap();
    let last_created = last["created_at"].as_str().unwrap();
    let ts = chrono::DateTime::parse_from_rfc3339(last_created).unwrap();
    let cursor = format!("{}:{}", ts.timestamp_micros(), last_id);
    let url_cursor = cursor.replace(':', "%3A");

    let res = app.client.get(app.url(&format!("/api/v1/entries?per_page=2&cursor={}", url_cursor)))
        .header("Authorization", format!("Bearer {}", token))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let next_header = res.headers().get("x-next-cursor").map(|v| v.to_str().unwrap().to_string());
    let second: Vec<serde_json::Value> = res.json().await.unwrap();
    // Expect 2 more items (we have 5 total, used 2 in page 1)
    assert_eq!(second.len(), 2);
    // The two pages should not overlap
    let ids_first: Vec<&str> = first.iter().map(|e| e["id"].as_str().unwrap()).collect();
    for item in &second {
        assert!(!ids_first.contains(&item["id"].as_str().unwrap()),
                "cursor result overlaps with first page");
    }
    // Header should be present (page is full)
    assert!(next_header.is_some(), "X-Next-Cursor expected on full page in cursor mode");

    // Final cursor request: use second page's last as cursor. Should return 1 item, no next cursor.
    let last2 = &second[1];
    let last2_id = last2["id"].as_str().unwrap();
    let last2_created = last2["created_at"].as_str().unwrap();
    let ts2 = chrono::DateTime::parse_from_rfc3339(last2_created).unwrap();
    let cursor2 = format!("{}:{}", ts2.timestamp_micros(), last2_id);
    let url_cursor2 = cursor2.replace(':', "%3A");

    let res = app.client.get(app.url(&format!("/api/v1/entries?per_page=2&cursor={}", url_cursor2)))
        .header("Authorization", format!("Bearer {}", token))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let final_header = res.headers().get("x-next-cursor").map(|v| v.to_str().unwrap().to_string());
    let third: Vec<serde_json::Value> = res.json().await.unwrap();
    assert_eq!(third.len(), 1, "5 items / per_page=2: third page has 1 item");
    assert!(final_header.is_none(), "X-Next-Cursor must NOT be set when page is short");

    app.cleanup().await;
}

#[tokio::test]
async fn cursor_bypasses_page_50_guard() {
    let app = TestApp::new().await;
    let token = {
        app.client.post(app.url("/api/v1/auth/register"))
            .json(&json!({"username":"cursor2","email":"c2@e.com","password":"password123"}))
            .send().await.unwrap();
        let login: serde_json::Value = app.client.post(app.url("/api/v1/auth/login"))
            .json(&json!({"email":"c2@e.com","password":"password123"}))
            .send().await.unwrap().json().await.unwrap();
        login["access_token"].as_str().unwrap().to_string()
    };

    // Use a far-future cursor so the result is empty but the request itself succeeds.
    let cursor = format!("{}:{}", 9_999_999_999_999_999i64, uuid::Uuid::nil());
    let url_cursor = cursor.replace(':', "%3A");
    let res = app.client.get(app.url(&format!("/api/v1/entries?page=999&cursor={}", url_cursor)))
        .header("Authorization", format!("Bearer {}", token))
        .send().await.unwrap();
    assert_eq!(res.status(), 200, "cursor mode must skip the page>50 guard");

    app.cleanup().await;
}

#[tokio::test]
async fn invalid_cursor_returns_400() {
    let app = TestApp::new().await;
    let token = {
        app.client.post(app.url("/api/v1/auth/register"))
            .json(&json!({"username":"cursor3","email":"c3@e.com","password":"password123"}))
            .send().await.unwrap();
        let login: serde_json::Value = app.client.post(app.url("/api/v1/auth/login"))
            .json(&json!({"email":"c3@e.com","password":"password123"}))
            .send().await.unwrap().json().await.unwrap();
        login["access_token"].as_str().unwrap().to_string()
    };

    let res = app.client.get(app.url("/api/v1/entries?cursor=garbage"))
        .header("Authorization", format!("Bearer {}", token))
        .send().await.unwrap();
    assert_eq!(res.status(), 400);

    app.cleanup().await;
}
