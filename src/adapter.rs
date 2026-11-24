use anyhow::{Context, Result};
use anylist_rs::{AnyListClient, SavedTokens, protobuf::anylist::PbShoppingListsResponse};
use prost::Message;
use serde::Serialize;

#[derive(Serialize)]
pub struct Snapshot {
    pub current_list_id: String,
    pub lists: Vec<List>,
}

#[derive(Serialize)]
pub struct List {
    pub id: String,
    pub name: String,
    pub(crate) items: Vec<Item>,
}

#[derive(Serialize)]
pub(crate) struct Item {
    id: String,
    name: String,
    notes: String,
    ticked: bool,
}

// anylist_rs 0.5.1 drops settings field 43 during decoding. Project only the
// two sections needed for a snapshot, reusing upstream shopping-list types.
#[derive(Message)]
struct UserData {
    #[prost(message, optional, tag = "1")]
    shopping_lists_response: Option<PbShoppingListsResponse>,
    #[prost(message, optional, tag = "11")]
    mobile_app_settings_response: Option<Selection>,
}

#[derive(Message)]
struct Selection {
    #[prost(string, optional, tag = "43")]
    selected_list_id: Option<String>,
}

impl Snapshot {
    pub fn current_list(&self) -> Result<&List> {
        self.lists
            .iter()
            .find(|list| list.id == self.current_list_id)
            .context("AnyList current list ID matches no shopping list")
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        let data = UserData::decode(bytes)?;
        let current_list_id = data
            .mobile_app_settings_response
            .and_then(|settings| settings.selected_list_id)
            .filter(|id| !id.is_empty())
            .context("AnyList did not supply a current list")?;
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

pub struct RemoteSession {
    client: AnyListClient,
    http: reqwest::Client,
    user_data_url: String,
}

impl RemoteSession {
    pub fn new(client: AnyListClient, base_url: &str) -> Result<Self> {
        Ok(Self {
            client: client.with_base_url(base_url),
            http: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
            user_data_url: format!("{base_url}/data/user-data/get"),
        })
    }

    async fn request_data(&self) -> Result<reqwest::Response> {
        Ok(self
            .http
            .post(&self.user_data_url)
            .bearer_auth(self.client.export_tokens()?.access_token())
            .header("X-AnyLeaf-API-Version", "3")
            .header(
                "X-AnyLeaf-Client-Identifier",
                self.client.client_identifier(),
            )
            .multipart(
                reqwest::multipart::Form::new()
                    .part("operations", reqwest::multipart::Part::bytes(Vec::new())),
            )
            .send()
            .await?)
    }
}

impl crate::session::Session for RemoteSession {
    async fn fetch(&self) -> Result<Snapshot> {
        let mut response = self.request_data().await?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            self.client
                .refresh_tokens()
                .await
                .map_err(|error| match error {
                    anylist_rs::AnyListError::AuthenticationFailed(_) => {
                        anyhow::anyhow!(crate::storage::LOGIN_REQUIRED)
                    }
                    other => anyhow::Error::new(other),
                })?;
            response = self.request_data().await?;
        }
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            anyhow::bail!(crate::storage::LOGIN_REQUIRED);
        }
        if !response.status().is_success() {
            anyhow::bail!("Cannot fetch AnyList data ({})", response.status());
        }
        Snapshot::decode(&response.bytes().await?)
    }
    fn tokens(&self) -> Result<SavedTokens> {
        Ok(self.client.export_tokens()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anylist_rs::protobuf::anylist::{
        PbListItem, PbMobileAppSettings, PbShoppingList, PbUserDataResponse,
    };
    use serde_json::json;

    #[test]
    fn snapshot_uses_selected_list_and_preserves_all_lists_and_item_details() {
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
        let mut wire = data.encode_to_vec();
        // A second settings fragment merges into field 11. String field 43 selects list b.
        wire.extend_from_slice(&[0x5a, 4, 0xda, 2, 1, b'b']);
        assert_eq!(
            serde_json::to_value(Snapshot::decode(&wire).unwrap()).unwrap(),
            json!({
                "current_list_id":"b", "lists":[{"id":"a","name":"First","items":[]},{"id":"b","name":"Second","items":[{"id":"c","name":"Tea","notes":"Green","ticked":true}]}]
            })
        );
    }

    #[test]
    fn missing_or_unmatched_selection_reports_the_problem() {
        for (id, message) in [
            (None, "AnyList did not supply a current list"),
            (
                Some("absent"),
                "AnyList current list ID matches no shopping list",
            ),
        ] {
            let data = UserData {
                mobile_app_settings_response: Some(Selection {
                    selected_list_id: id.map(String::from),
                }),
                shopping_lists_response: Some(Default::default()),
            };
            assert_eq!(
                Snapshot::decode(&data.encode_to_vec())
                    .err()
                    .expect("must reject unresolved list")
                    .to_string(),
                message
            );
        }
    }
}
