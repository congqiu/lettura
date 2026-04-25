mod common;

use lettura::models::{entry, tag};
use uuid::Uuid;

async fn make_user(pool: &sqlx::PgPool) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, username, email, password_hash) VALUES ($1, $2, $3, 'x')")
        .bind(id).bind(format!("u{}", id.simple())).bind(format!("{}@e.com", id.simple()))
        .execute(pool).await.unwrap();
    id
}

#[tokio::test]
async fn ensure_and_link_creates_missing_tags_and_links_all_pairs() {
    let app = common::TestApp::new().await;
    let user_id = make_user(&app.pool).await;
    let e1 = entry::create_entry(&app.pool, user_id, "https://a.test").await.unwrap();
    let e2 = entry::create_entry(&app.pool, user_id, "https://b.test").await.unwrap();

    let labels = vec!["rust".to_string(), "tokio".to_string()];
    let entry_ids = vec![e1.id, e2.id];
    tag::ensure_and_link(&app.pool, user_id, &entry_ids, &labels).await.unwrap();

    let t1 = tag::list_tags_for_entry(&app.pool, e1.id).await.unwrap();
    let t2 = tag::list_tags_for_entry(&app.pool, e2.id).await.unwrap();
    let l1: Vec<&str> = t1.iter().map(|t| t.label.as_str()).collect();
    let l2: Vec<&str> = t2.iter().map(|t| t.label.as_str()).collect();
    assert!(l1.contains(&"rust") && l1.contains(&"tokio"));
    assert!(l2.contains(&"rust") && l2.contains(&"tokio"));

    let all = tag::list_tags(&app.pool, user_id).await.unwrap();
    assert_eq!(all.len(), 2, "two unique tags shared between entries");

    app.cleanup().await;
}

#[tokio::test]
async fn ensure_and_link_is_idempotent() {
    let app = common::TestApp::new().await;
    let user_id = make_user(&app.pool).await;
    let e1 = entry::create_entry(&app.pool, user_id, "https://c.test").await.unwrap();

    tag::ensure_and_link(&app.pool, user_id, &[e1.id], &["rust".into()]).await.unwrap();
    tag::ensure_and_link(&app.pool, user_id, &[e1.id], &["rust".into()]).await.unwrap();
    let t = tag::list_tags_for_entry(&app.pool, e1.id).await.unwrap();
    assert_eq!(t.len(), 1, "duplicate ensure_and_link must not duplicate links");

    app.cleanup().await;
}

#[tokio::test]
async fn ensure_and_link_empty_inputs_noop() {
    let app = common::TestApp::new().await;
    let user_id = make_user(&app.pool).await;
    let e1 = entry::create_entry(&app.pool, user_id, "https://d.test").await.unwrap();

    tag::ensure_and_link(&app.pool, user_id, &[], &["rust".into()]).await.unwrap();
    tag::ensure_and_link(&app.pool, user_id, &[e1.id], &[]).await.unwrap();

    assert_eq!(tag::list_tags_for_entry(&app.pool, e1.id).await.unwrap().len(), 0);
    assert_eq!(tag::list_tags(&app.pool, user_id).await.unwrap().len(), 0);

    app.cleanup().await;
}
