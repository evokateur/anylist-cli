use std::process::Command;
use tempfile::tempdir;

#[test]
fn sync_requires_login() {
    let home = tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_anylist"))
        .arg("sync")
        .env("XDG_DATA_HOME", home.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stderr, b"Not logged in. Run: anylist login\n");
    assert!(output.stdout.is_empty());
}

#[test]
fn logout_removes_local_session_and_preserves_config() {
    let home = tempdir().unwrap();
    let data = home
        .path()
        .join(".local/share/anylist-cli/credentials.json");
    let cache = home.path().join(".cache/anylist-cli/lists.json");
    let config = home.path().join(".config/anylist-cli/config.toml");
    for file in [&data, &cache, &config] {
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, "fixture").unwrap();
    }
    for _ in 0..2 {
        let output = Command::new(env!("CARGO_BIN_EXE_anylist"))
            .arg("logout")
            .env("HOME", home.path())
            .env_remove("XDG_DATA_HOME")
            .env_remove("XDG_CACHE_HOME")
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(!data.exists() && !cache.exists());
        assert_eq!(std::fs::read_to_string(&config).unwrap(), "fixture");
    }
}

#[test]
fn login_requires_an_email_before_authentication() {
    let output = Command::new(env!("CARGO_BIN_EXE_anylist"))
        .arg("login")
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stderr, b"Email: Email is required\n");
    assert!(output.stdout.is_empty());
}
