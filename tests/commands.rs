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

#[test]
fn malformed_saved_access_token_is_rejected_before_sync() {
    let home = tempdir().unwrap();
    let data = home.path().join("anylist-cli");
    std::fs::create_dir(&data).unwrap();
    std::fs::write(data.join("credentials.json"), r#"{"access_token":"invalid\nsecret","refresh_token":"refresh","user_id":"u","is_premium_user":false}"#).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_anylist"))
        .arg("sync")
        .env("XDG_DATA_HOME", home.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stderr, b"Invalid AnyList access token\n");
}

#[test]
fn logout_removes_credentials_even_if_cache_cannot_be_removed() {
    let home = tempdir().unwrap();
    let dir = home.path().join("anylist-cli");
    std::fs::create_dir_all(dir.join("lists.json")).unwrap();
    let credentials = dir.join("credentials.json");
    std::fs::write(&credentials, "fixture").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_anylist"))
        .arg("logout")
        .env("XDG_CACHE_HOME", home.path())
        .env("XDG_DATA_HOME", home.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(!credentials.exists());
}
