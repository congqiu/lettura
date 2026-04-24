use lettura_cli::filter::parse;

#[test]
fn parses_single_tag() {
    let f = parse("tag:golang").unwrap();
    assert_eq!(f.tags_include, vec!["golang".to_string()]);
}

#[test]
fn parses_negated_tag() {
    let f = parse("!tag:archive").unwrap();
    assert_eq!(f.tags_exclude, vec!["archive".to_string()]);
}

#[test]
fn parses_untagged() {
    let f = parse("untagged").unwrap();
    assert!(f.untagged);
}

#[test]
fn parses_multi_and() {
    let f = parse("domain:medium.com,untagged,since:7d").unwrap();
    assert_eq!(f.domain, Some("medium.com".into()));
    assert!(f.untagged);
    assert!(f.since.is_some());
}

#[test]
fn parses_absolute_since() {
    let f = parse("since:2026-01-01").unwrap();
    let expect = chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()
        .and_hms_opt(0, 0, 0).unwrap().and_utc();
    assert_eq!(f.since.unwrap(), expect);
}

#[test]
fn parses_relative_older_than() {
    let f = parse("older-than:90d").unwrap();
    assert!(f.older_than.is_some());
    // older-than should be roughly 90 days ago (within a minute of now - 90d)
    let expected = chrono::Utc::now() - chrono::Duration::days(90);
    let delta = (f.older_than.unwrap() - expected).num_seconds().abs();
    assert!(delta < 60, "older-than drift: {delta}s");
}

#[test]
fn parses_relative_hours() {
    let f = parse("since:24h").unwrap();
    assert!(f.since.is_some());
}

#[test]
fn parses_relative_weeks() {
    let f = parse("older-than:2w").unwrap();
    assert!(f.older_than.is_some());
}

#[test]
fn parses_boolean_flags() {
    let f = parse("starred,!archived,unread").unwrap();
    assert_eq!(f.starred, Some(true));
    assert_eq!(f.archived, Some(false));
    assert_eq!(f.read, Some(false));
}

#[test]
fn parses_read_flag() {
    let f = parse("read").unwrap();
    assert_eq!(f.read, Some(true));
}

#[test]
fn parses_search() {
    let f = parse("search:rust async").unwrap();
    assert_eq!(f.search, Some("rust async".into()));
}

#[test]
fn parses_negated_starred() {
    let f = parse("!starred").unwrap();
    assert_eq!(f.starred, Some(false));
}

#[test]
fn rejects_unknown_key() {
    assert!(parse("banana:1").is_err());
}

#[test]
fn rejects_unknown_single_word() {
    assert!(parse("weirdo").is_err());
}

#[test]
fn ignores_empty_pieces() {
    // trailing comma / double comma should not error
    let f = parse("tag:x,,").unwrap();
    assert_eq!(f.tags_include, vec!["x".to_string()]);
}

#[test]
fn to_query_emits_http_params() {
    let f = parse("tag:golang,untagged,domain:medium.com,starred,unread,search:async").unwrap();
    let q = f.to_query();
    // Check presence of each expected pair (order independent)
    let keys: Vec<_> = q.iter().map(|(k, _)| *k).collect();
    assert!(keys.contains(&"tag"));
    assert!(keys.contains(&"untagged"));
    assert!(keys.contains(&"domain"));
    assert!(keys.contains(&"is_starred"));
    assert!(keys.contains(&"is_read"));
    assert!(keys.contains(&"search"));
}
