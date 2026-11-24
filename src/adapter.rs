use anyhow::{Context, Result};
use anylist_rs::protobuf::anylist::PbUserDataResponse;
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

impl Snapshot {
    pub fn current_list(&self) -> Result<&List> {
        self.lists
            .iter()
            .find(|list| list.id == self.current_list_id)
            .context("AnyList default list ID matches no shopping list")
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

impl crate::session::Session for anylist_rs::AnyListClient {
    async fn fetch(&self) -> Result<Snapshot> {
        let data = self.get_user_data().await.map_err(|error| match error {
            anylist_rs::AnyListError::AuthenticationFailed(_) => {
                anyhow::anyhow!(crate::storage::LOGIN_REQUIRED)
            }
            other => anyhow::Error::new(other),
        })?;
        Snapshot::try_from(data)
    }

    fn tokens(&self) -> Result<anylist_rs::SavedTokens> {
        Ok(self.export_tokens()?)
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
