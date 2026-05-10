use lettura_cli::commands;
use tempfile::tempdir;

#[test]
fn skill_print_returns_ok() {
    let code = commands::skill::run_print().unwrap();
    assert_eq!(code, 0);
}

#[test]
fn skill_install_writes_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("installed-skill.md");
    let code = commands::skill::run_install(Some(path.to_str().unwrap())).unwrap();
    assert_eq!(code, 0);
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(
        content.contains("# Lettura CLI"),
        "expected skill body in installed file"
    );
    assert!(
        content.contains("{{BASE_URL}}"),
        "placeholders should be preserved verbatim"
    );
}

#[test]
fn skill_install_creates_parent_dirs() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("a").join("b").join("c").join("skill.md");
    let code = commands::skill::run_install(Some(path.to_str().unwrap())).unwrap();
    assert_eq!(code, 0);
    assert!(path.exists());
}
