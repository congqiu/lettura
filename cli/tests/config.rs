use lettura_cli::config::{Config, Override, Profile, resolve};
use tempfile::tempdir;

#[test]
fn resolves_default_profile() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(
        &path,
        r#"
default_profile = "default"
[profiles.default]
url = "https://x.test"
token = "lta_abc"
"#,
    )
    .unwrap();
    let cfg = Config::load_from(&path).unwrap();
    let r = resolve(&cfg, &Override::default()).unwrap();
    assert_eq!(r.url, "https://x.test");
    assert_eq!(r.token, "lta_abc");
}

#[test]
fn cli_override_wins_over_profile() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(
        &path,
        r#"
default_profile = "default"
[profiles.default]
url = "https://x.test"
token = "lta_abc"
"#,
    )
    .unwrap();
    let cfg = Config::load_from(&path).unwrap();
    let r = resolve(
        &cfg,
        &Override {
            url: Some("https://override.test".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.url, "https://override.test");
    assert_eq!(r.token, "lta_abc");
}

#[test]
fn missing_config_returns_empty() {
    let dir = tempdir().unwrap();
    let res = Config::load_from(&dir.path().join("nope.toml")).unwrap();
    assert!(res.profiles.is_empty());
    assert!(res.default_profile.is_none());
}

#[test]
fn resolve_fails_with_no_url_and_no_override() {
    let cfg = Config::default();
    let err = resolve(&cfg, &Override::default()).unwrap_err();
    assert!(err.to_string().contains("URL"));
}

#[test]
fn resolve_with_named_profile_override() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(
        &path,
        r#"
default_profile = "default"
[profiles.default]
url = "https://default.test"
token = "t_d"
[profiles.work]
url = "https://work.test"
token = "t_w"
"#,
    )
    .unwrap();
    let cfg = Config::load_from(&path).unwrap();
    let r = resolve(
        &cfg,
        &Override {
            profile: Some("work".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.url, "https://work.test");
}

#[cfg(unix)]
#[test]
fn save_sets_0600_permissions_on_unix() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let mut cfg = Config::default();
    cfg.profiles.insert(
        "default".into(),
        Profile {
            url: "https://x.test".into(),
            token: "lta_abc".into(),
        },
    );
    cfg.default_profile = Some("default".into());
    cfg.save_to(&path).unwrap();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn round_trip_preserves_data() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let mut cfg = Config::default();
    cfg.profiles.insert(
        "default".into(),
        Profile {
            url: "https://x.test".into(),
            token: "lta_abc".into(),
        },
    );
    cfg.default_profile = Some("default".into());
    cfg.save_to(&path).unwrap();
    let loaded = Config::load_from(&path).unwrap();
    assert_eq!(loaded.default_profile.as_deref(), Some("default"));
    assert_eq!(
        loaded.profiles.get("default").unwrap().url,
        "https://x.test"
    );
}
