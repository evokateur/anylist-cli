use anyhow::{Context, Result};
use anylist_rs::protobuf::anylist::PbUserDataResponse;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Snapshot {
    pub current_list_id: String,
    pub lists: Vec<List>,
}

#[derive(Serialize, Deserialize)]
pub struct List {
    pub id: String,
    pub name: String,
    pub(crate) items: Vec<Item>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Item {
    pub(crate) id: String,
    pub(crate) name: String,
    notes: String,
    ticked: bool,
}

impl Snapshot {
    pub fn current_list(&self) -> Result<&List> {
        self.lists
            .iter()
            .find(|list| list.id == self.current_list_id)
            .context("AnyList default list ID matches no shopping list")
    }
}

impl List {
    pub fn item(&self, id: &str) -> Option<&Item> {
        self.items.iter().find(|item| item.id == id)
    }

    pub fn unknown_item(&self, alias_or_id: &str) -> anyhow::Error {
        anyhow::anyhow!(
            "No item with alias or ID '{alias_or_id}' in '{}'",
            self.name
        )
    }
}

impl TryFrom<PbUserDataResponse> for Snapshot {
    type Error = anyhow::Error;
    fn try_from(data: PbUserDataResponse) -> Result<Self> {
        let current_list_id = data
            .mobile_app_settings_response
            .and_then(|settings| settings.default_list_id)
            .filter(|id| !id.is_empty())
            .context("AnyList did not supply a default list")?;
        let mut lists = Vec::new();
        for source in data
            .shopping_lists_response
            .context("Missing shopping lists")?
            .new_lists
        {
            let mut items = Vec::new();
            for item in source.items {
                items.push(Item {
                    id: item.identifier,
                    name: item.name.context("Missing item name")?,
                    notes: item.details.unwrap_or_default(),
                    ticked: item.checked.unwrap_or(false),
                });
            }
            lists.push(List {
                id: source.identifier,
                name: source.name.context("Missing list name")?,
                items,
            });
        }
        let snapshot = Self {
            current_list_id,
            lists,
        };
        snapshot.current_list()?;
        Ok(snapshot)
    }
}

// anylist_rs 0.5.1 embeds the refresh HTTP status and raw response body in
// its error string. Surface only the status; bodies may contain account data.
fn refresh_error(error: anylist_rs::AnyListError) -> anyhow::Error {
    use anylist_rs::AnyListError;
    match error {
        AnyListError::AuthenticationFailed(message) => {
            let status = message
                .strip_prefix("Token refresh failed with status: ")
                .and_then(|rest| rest.split_once(", body: "))
                .and_then(|(status, _)| status.split_whitespace().next())
                .and_then(|code| code.parse::<u16>().ok());
            match status {
                Some(400 | 401 | 403) => anyhow::anyhow!(crate::storage::LOGIN_REQUIRED),
                Some(code) => anyhow::anyhow!("AnyList token refresh failed (HTTP {code})"),
                None => anyhow::anyhow!("AnyList token refresh failed"),
            }
        }
        other => other.into(),
    }
}

/// Make an authenticated call; on authentication failure, refresh and validate
/// the tokens, then retry once. A second authentication failure means not logged in.
async fn authenticated<T, Call: Future<Output = anylist_rs::Result<T>>>(
    client: &anylist_rs::AnyListClient,
    call: impl Fn() -> Call,
) -> Result<T> {
    use anylist_rs::AnyListError::AuthenticationFailed;
    match call().await {
        Err(AuthenticationFailed(_)) => {
            client.refresh_tokens().await.map_err(refresh_error)?;
            crate::session::Session::tokens(client)?;
            call().await.map_err(|error| match error {
                AuthenticationFailed(_) => anyhow::anyhow!(crate::storage::LOGIN_REQUIRED),
                other => other.into(),
            })
        }
        result => Ok(result?),
    }
}

impl crate::session::Session for anylist_rs::AnyListClient {
    async fn fetch(&self) -> Result<Snapshot> {
        Snapshot::try_from(authenticated(self, || self.get_user_data()).await?)
    }

    async fn untick(&self, list_id: &str, item_id: &str) -> Result<()> {
        authenticated(self, || self.uncheck_item(list_id, item_id)).await
    }

    fn tokens(&self) -> Result<anylist_rs::SavedTokens> {
        let tokens = self.export_tokens()?;
        crate::login::validate(&tokens)?;
        Ok(tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anylist_rs::protobuf::anylist::{
        PbListItem, PbMobileAppSettings, PbShoppingList, PbShoppingListsResponse,
    };
    use serde_json::json;

    #[test]
    fn snapshot_uses_account_default_and_preserves_all_lists_and_item_details() {
        let data = PbUserDataResponse {
            mobile_app_settings_response: Some(PbMobileAppSettings {
                default_list_id: Some("a".into()),
                ..Default::default()
            }),
            shopping_lists_response: Some(PbShoppingListsResponse {
                new_lists: vec![
                    PbShoppingList {
                        identifier: "a".into(),
                        name: Some("First".into()),
                        ..Default::default()
                    },
                    PbShoppingList {
                        identifier: "b".into(),
                        name: Some("Second".into()),
                        items: vec![PbListItem {
                            identifier: "c".into(),
                            name: Some("Tea".into()),
                            details: Some("Green".into()),
                            checked: Some(true),
                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            ..Default::default()
        };
        use prost::Message;
        let mut wire = data.encode_to_vec();
        // Desktop selects list b while the account default remains list a.
        wire.extend_from_slice(&[0x5a, 4, 0xda, 2, 1, b'b']);
        let data = PbUserDataResponse::decode(wire.as_slice()).unwrap();
        assert_eq!(
            serde_json::to_value(Snapshot::try_from(data).unwrap()).unwrap(),
            json!({
                "current_list_id":"a", "lists":[{"id":"a","name":"First","items":[]},{"id":"b","name":"Second","items":[{"id":"c","name":"Tea","notes":"Green","ticked":true}]}]
            })
        );
    }

    /// Serves `exchanges` in order as (expected path, status, body), returning each request body.
    fn serve(
        exchanges: Vec<(&'static str, &'static str, &'static str)>,
    ) -> (String, std::thread::JoinHandle<Vec<Vec<u8>>>) {
        use std::io::{BufRead, BufReader, Read, Write};
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let mut bodies = Vec::new();
            for (path, status, response) in exchanges {
                let (mut stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(&stream);
                let mut headers = String::new();
                while !headers.ends_with("\r\n\r\n") {
                    assert_ne!(reader.read_line(&mut headers).unwrap(), 0);
                }
                assert!(headers.starts_with(&format!("POST {path} ")), "{headers}");
                let length: usize = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .map(|value| value.parse().unwrap())
                    })
                    .unwrap_or(0);
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                bodies.push(body);
                write!(stream, "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}", response.len()).unwrap();
            }
            bodies
        });
        (base, server)
    }

    #[tokio::test]
    async fn untick_refreshes_and_retries_once_after_authentication_failure() {
        use crate::session::Session;
        const UPDATE: &str = "/data/shopping-lists/update";
        const REFRESH: &str = "/auth/token/refresh";
        const RENEWED: &str = r#"{"access_token":"renewed","refresh_token":"second"}"#;
        for (exchanges, expected) in [
            (
                vec![
                    (UPDATE, "401 Unauthorized", ""),
                    (REFRESH, "200 OK", RENEWED),
                    (UPDATE, "200 OK", ""),
                ],
                Ok("renewed"),
            ),
            (
                vec![
                    (UPDATE, "401 Unauthorized", ""),
                    (REFRESH, "200 OK", RENEWED),
                    (UPDATE, "401 Unauthorized", ""),
                ],
                Err(crate::storage::LOGIN_REQUIRED),
            ),
            (
                vec![
                    (UPDATE, "401 Unauthorized", ""),
                    (REFRESH, "401 Unauthorized", "private"),
                ],
                Err(crate::storage::LOGIN_REQUIRED),
            ),
        ] {
            let (base, server) = serve(exchanges);
            let client = crate::login::restore(anylist_rs::SavedTokens::new(
                "initial", "first", "user", false,
            ))
            .unwrap()
            .with_base_url(base);
            let result = client
                .untick("market", "a1c7e4b9d2f04e6b8c5a3f0e7d1b9c26")
                .await;
            let bodies = server.join().unwrap();
            match expected {
                Ok(access_token) => {
                    result.unwrap();
                    assert_eq!(client.tokens().unwrap().access_token(), access_token);
                }
                Err(message) => assert_eq!(result.unwrap_err().to_string(), message),
            }
            let operation = String::from_utf8_lossy(&bodies[0]);
            for value in [
                "set-list-item-checked",
                "market",
                "a1c7e4b9d2f04e6b8c5a3f0e7d1b9c26",
            ] {
                assert!(operation.contains(value), "{value}");
            }
        }
    }

    #[test]
    fn missing_or_unmatched_selection_reports_the_problem() {
        for (id, message) in [
            (None, "AnyList did not supply a default list"),
            (
                Some("absent"),
                "AnyList default list ID matches no shopping list",
            ),
        ] {
            let data = PbUserDataResponse {
                mobile_app_settings_response: Some(PbMobileAppSettings {
                    default_list_id: id.map(String::from),
                    ..Default::default()
                }),
                shopping_lists_response: Some(Default::default()),
                ..Default::default()
            };
            assert_eq!(
                Snapshot::try_from(data)
                    .err()
                    .expect("must reject unresolved list")
                    .to_string(),
                message
            );
        }
    }
}
