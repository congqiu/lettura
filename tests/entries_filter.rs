mod common;
use common::TestApp;
use lettura::models::entry::{self, ListParams};

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
    };
    let res = entry::list_entries(&app.pool, user_id, &params)
        .await
        .unwrap();
    assert_eq!(res.len(), 1);
    assert_eq!(res[0].id, e1);
    app.cleanup().await;
}
