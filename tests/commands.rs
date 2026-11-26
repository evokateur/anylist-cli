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

const CACHE: &str = r#"{"current_list_id":"market","lists":[
{"id":"market","name":"queste sono le cose","items":[
{"id":"a1c7e4b9d2f04e6b8c5a3f0e7d1b9c26","name":"Cream cheese","notes":"","ticked":true}]},
{"id":"hardware","name":"Hardware","items":[
{"id":"0b8e2f3c4d5a6b7c8d9e0f1a2b3c4d5e","name":"Nails","notes":"","ticked":true}]}]}"#;

/// Isolated XDG directories holding a logged-in session, the given cache and config.
fn untick_home(cache: Option<&str>, config: Option<&str>) -> tempfile::TempDir {
    let home = tempdir().unwrap();
    let write = |relative: &str, contents: &str| {
        let path = home.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    };
    write(
        "data/anylist-cli/credentials.json",
        r#"{"access_token":"token","refresh_token":"refresh","user_id":"u","is_premium_user":false}"#,
    );
    if let Some(cache) = cache {
        write("cache/anylist-cli/lists.json", cache);
    }
    if let Some(config) = config {
        write("config/anylist-cli/config.toml", config);
    }
    home
}

fn untick(home: &std::path::Path, argument: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_anylist"))
        .args(["untick", argument])
        .env("XDG_DATA_HOME", home.join("data"))
        .env("XDG_CACHE_HOME", home.join("cache"))
        .env("XDG_CONFIG_HOME", home.join("config"))
        .output()
        .unwrap()
}

#[test]
fn untick_reports_malformed_config_or_non_string_alias() {
    for config in ["[aliases\n", "[aliases]\nphilly = 1\n"] {
        let home = untick_home(Some(CACHE), Some(config));
        let output = untick(home.path(), "philly");
        assert_eq!(output.status.code(), Some(1), "{config:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.starts_with("Config error: "), "{stderr}");
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn untick_requires_cached_lists() {
    let home = untick_home(None, None);
    let output = untick(home.path(), "philly");
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stderr, b"No cached lists. Run: anylist sync\n");
    assert!(output.stdout.is_empty());
}

#[test]
fn untick_checks_login_before_cache() {
    let home = tempdir().unwrap();
    let output = untick(home.path(), "philly");
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stderr, b"Not logged in. Run: anylist login\n");
}

#[test]
fn untick_rejects_arguments_not_on_the_cached_current_list() {
    let config = "[aliases]\nphilly = \"a1c7e4b9d2f04e6b8c5a3f0e7d1b9c26\"\nnails = \"0b8e2f3c4d5a6b7c8d9e0f1a2b3c4d5e\"\n";
    for argument in ["phil", "nails", "0b8e2f3c4d5a6b7c8d9e0f1a2b3c4d5e"] {
        let home = untick_home(Some(CACHE), Some(config));
        let output = untick(home.path(), argument);
        assert_eq!(output.status.code(), Some(1), "{argument}");
        assert_eq!(
            String::from_utf8(output.stderr).unwrap(),
            format!("No item with alias or ID '{argument}' in 'queste sono le cose'\n")
        );
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn untick_without_an_argument_is_a_usage_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_anylist"))
        .arg("untick")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("Usage: anylist untick <ITEM>")
    );
    assert!(output.stdout.is_empty());
}
