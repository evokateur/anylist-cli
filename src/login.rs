use anyhow::{Context, Result, bail};
use anylist_rs::{AnyListClient, SavedTokens};
use std::io::{self, Write};

pub fn restore(tokens: SavedTokens) -> Result<AnyListClient> {
    if tokens.access_token().is_empty()
        || reqwest::header::HeaderValue::from_str(tokens.access_token()).is_err()
    {
        bail!("Invalid AnyList access token");
    }
    Ok(AnyListClient::from_tokens(tokens)?)
}

pub async fn prompt() -> Result<AnyListClient> {
    eprint!("Email: ");
    io::stderr().flush()?;
    let mut email = String::new();
    io::stdin().read_line(&mut email)?;
    let email = email.trim();
    if email.is_empty() {
        bail!("Email is required");
    }
    let password =
        rpassword::prompt_password("Password: ").context("Cannot read password from terminal")?;
    authenticate("https://www.anylist.com/auth/token", email, password).await
}

// The upstream login helper prints response headers and bodies on failure.
// Keep login diagnostics free of account data and use the upstream client for the session.
async fn authenticate(endpoint: &str, email: &str, password: String) -> Result<AnyListClient> {
    let identifier = uuid::Uuid::new_v4().simple().to_string();
    let response = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()?
        .post(endpoint)
        .header("X-AnyLeaf-API-Version", "3")
        .header("X-AnyLeaf-Client-Identifier", &identifier)
        .multipart(
            reqwest::multipart::Form::new()
                .text("email", email.to_owned())
                .text("password", password),
        )
        .send()
        .await
        .context("Cannot reach AnyList login")?;
    if !response.status().is_success() {
        bail!("AnyList login failed ({})", response.status());
    }
    #[derive(serde::Deserialize)]
    struct LoginResponse {
        access_token: String,
        refresh_token: String,
        user_id: String,
        #[serde(default)]
        is_premium_user: bool,
    }
    let login: LoginResponse = response
        .json()
        .await
        .context("Cannot read AnyList login response")?;
    let tokens = SavedTokens::new(
        login.access_token,
        login.refresh_token,
        login.user_id,
        login.is_premium_user,
    );
    let mut client = restore(tokens)?;
    client.set_client_identifier(identifier);
    Ok(client)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Storage;
    use std::io::{BufRead, BufReader, Read};
    use std::net::TcpListener;

    #[tokio::test]
    async fn login_exchanges_credentials_and_keeps_tokens_if_first_sync_fails() {
        for (refresh_status, retry_status, expected_error) in [
            ("200 OK", "503 Unavailable", "503"),
            ("200 OK", "401 Unauthorized", crate::storage::LOGIN_REQUIRED),
            ("503 Unavailable", "", "503"),
            ("401 Unauthorized", "", crate::storage::LOGIN_REQUIRED),
        ] {
            failed_sync_fixture(refresh_status, retry_status, expected_error).await;
        }
    }

    async fn failed_sync_fixture(
        refresh_status: &'static str,
        retry_status: &'static str,
        expected_error: &str,
    ) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            for (path, status, response) in [
                (
                    "/auth/token",
                    "200 OK",
                    r#"{"access_token":"initial","refresh_token":"first","user_id":"test"}"#,
                ),
                ("/data/user-data/get", "401 Unauthorized", ""),
                (
                    "/auth/token/refresh",
                    refresh_status,
                    r#"{"access_token":"renewed","refresh_token":"second"}"#,
                ),
                ("/data/user-data/get", retry_status, ""),
            ] {
                if status.is_empty() {
                    break;
                }
                let (mut stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(&stream);
                let mut headers = String::new();
                while !headers.ends_with("\r\n\r\n") {
                    assert_ne!(reader.read_line(&mut headers).unwrap(), 0);
                }
                assert!(headers.starts_with(&format!("POST {path} ")));
                let length: usize = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .map(|value| value.parse().unwrap())
                    })
                    .unwrap();
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                if path == "/auth/token" {
                    let body = String::from_utf8(body).unwrap();
                    for value in [
                        "name=\"email\"",
                        "tester@example.invalid",
                        "name=\"password\"",
                        "fixture-password",
                    ] {
                        assert!(body.contains(value));
                    }
                }
                write!(stream, "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}", response.len()).unwrap();
            }
        });
        let client = authenticate(
            &format!("{base}/auth/token"),
            "tester@example.invalid",
            "fixture-password".into(),
        )
        .await
        .unwrap()
        .with_base_url(base);
        let home = tempfile::tempdir().unwrap();
        let storage = Storage {
            credentials: home.path().join("credentials.json"),
            cache: home.path().join("lists.json"),
        };
        std::fs::write(&storage.cache, "old account").unwrap();
        storage
            .begin_session(&client.export_tokens().unwrap())
            .unwrap();
        assert_eq!(storage.tokens().unwrap().access_token(), "initial");
        assert!(
            crate::session::sync(&client, &storage)
                .await
                .unwrap_err()
                .to_string()
                .contains(expected_error)
        );
        assert_eq!(
            storage.tokens().unwrap().refresh_token(),
            if refresh_status == "200 OK" {
                "second"
            } else {
                "first"
            }
        );
        assert!(!storage.cache.exists());
        server.join().unwrap();
    }
}
