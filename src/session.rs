use crate::{adapter::Snapshot, storage::Storage};
use anyhow::Result;
use anylist_rs::SavedTokens;

pub trait Session {
    async fn fetch(&self) -> Result<Snapshot>;
    async fn untick(&self, list_id: &str, item_id: &str) -> Result<()>;
    fn tokens(&self) -> Result<SavedTokens>;
}

/// Save the session's tokens whatever the outcome, then cache a fetched snapshot.
fn persist(
    session: &impl Session,
    storage: &Storage,
    fetched: Result<Snapshot>,
) -> Result<Snapshot> {
    storage.save_tokens(&session.tokens()?)?;
    let snapshot = fetched?;
    storage.save_snapshot(&snapshot)?;
    Ok(snapshot)
}

pub async fn sync(session: &impl Session, storage: &Storage) -> Result<String> {
    let snapshot = persist(session, storage, session.fetch().await)?;
    Ok(snapshot.current_list()?.name.clone())
}

/// Untick the item named by `argument` on the cached current list.
pub async fn untick(session: &impl Session, storage: &Storage, argument: &str) -> Result<String> {
    let cached = storage.snapshot()?;
    let aliases = storage.aliases()?;
    let list = cached.current_list()?;
    let item = list
        .item(argument, &aliases)
        .ok_or_else(|| list.unknown_item(argument))?;
    let fetched = async {
        session.untick(&list.id, &item.id).await?;
        session.fetch().await
    }
    .await;
    let fetched = persist(session, storage, fetched)?;
    let list = fetched.current_list()?;
    let unticked = list
        .items
        .iter()
        .find(|candidate| candidate.id == item.id)
        .ok_or_else(|| list.unknown_item(argument))?;
    Ok(format!("Unticked: {} on '{}'", unticked.name, list.name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::List;
    use std::{cell::Cell, fs, os::unix::fs::PermissionsExt};

    struct FakeSession {
        fetched: Cell<bool>,
        fails: bool,
    }
    impl Session for FakeSession {
        async fn fetch(&self) -> Result<Snapshot> {
            self.fetched.set(true);
            if self.fails {
                anyhow::bail!("fetch failed");
            }
            Ok(Snapshot {
                current_list_id: "a".into(),
                lists: vec![List {
                    id: "a".into(),
                    name: "Market".into(),
                    items: vec![],
                }],
            })
        }
        async fn untick(&self, _list_id: &str, _item_id: &str) -> Result<()> {
            unreachable!("sync does not untick")
        }
        fn tokens(&self) -> Result<SavedTokens> {
            Ok(SavedTokens::new(
                if self.fetched.get() {
                    "renewed"
                } else {
                    "initial"
                },
                "refresh",
                "user",
                false,
            ))
        }
    }

    #[tokio::test]
    async fn sync_caches_selection_and_saves_private_rotated_credentials() {
        let root = tempfile::tempdir().unwrap();
        let storage = Storage {
            credentials: root.path().join("data/credentials.json"),
            cache: root.path().join("cache/lists.json"),
            config: root.path().join("config.toml"),
        };
        let current = sync(
            &FakeSession {
                fetched: Cell::new(false),
                fails: false,
            },
            &storage,
        )
        .await
        .unwrap();
        assert_eq!(current, "Market");
        let cache: serde_json::Value =
            serde_json::from_reader(fs::File::open(&storage.cache).unwrap()).unwrap();
        assert_eq!(cache["current_list_id"], "a");
        assert_eq!(cache["lists"][0]["name"], "Market");
        assert_eq!(storage.tokens().unwrap().access_token(), "renewed");
        assert_eq!(
            fs::metadata(&storage.credentials)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    #[tokio::test]
    async fn failed_sync_keeps_previous_cache_and_saves_rotated_tokens() {
        let root = tempfile::tempdir().unwrap();
        let storage = Storage {
            credentials: root.path().join("credentials.json"),
            cache: root.path().join("lists.json"),
            config: root.path().join("config.toml"),
        };
        fs::write(&storage.cache, "previous").unwrap();
        let error = sync(
            &FakeSession {
                fetched: Cell::new(false),
                fails: true,
            },
            &storage,
        )
        .await
        .unwrap_err();
        assert_eq!(error.to_string(), "fetch failed");
        assert_eq!(storage.tokens().unwrap().access_token(), "renewed");
        assert_eq!(fs::read_to_string(&storage.cache).unwrap(), "previous");
    }

    const CREAM_CHEESE: &str = "a1c7e4b9d2f04e6b8c5a3f0e7d1b9c26";

    fn snapshot(list_name: &str, ticked: bool) -> String {
        format!(
            r#"{{"current_list_id":"market","lists":[{{"id":"market","name":"{list_name}","items":[{{"id":"{CREAM_CHEESE}","name":"Cream cheese","notes":"","ticked":{ticked}}}]}}]}}"#
        )
    }

    /// Records untick ops; fetch returns `fetched` (JSON) or fails when absent.
    struct FakeUntick {
        calls: Cell<u32>,
        unticked: std::cell::RefCell<Vec<(String, String)>>,
        untick_error: Option<&'static str>,
        fetched: Option<String>,
    }
    impl FakeUntick {
        fn new(untick_error: Option<&'static str>, fetched: Option<String>) -> Self {
            Self {
                calls: Cell::new(0),
                unticked: Default::default(),
                untick_error,
                fetched,
            }
        }
    }
    impl Session for FakeUntick {
        async fn fetch(&self) -> Result<Snapshot> {
            self.calls.set(self.calls.get() + 1);
            match &self.fetched {
                Some(json) => Ok(serde_json::from_str(json)?),
                None => anyhow::bail!("fetch failed"),
            }
        }
        async fn untick(&self, list_id: &str, item_id: &str) -> Result<()> {
            self.calls.set(self.calls.get() + 1);
            self.unticked
                .borrow_mut()
                .push((list_id.into(), item_id.into()));
            match self.untick_error {
                Some(message) => anyhow::bail!(message),
                None => Ok(()),
            }
        }
        fn tokens(&self) -> Result<SavedTokens> {
            let access = format!("after {} calls", self.calls.get());
            Ok(SavedTokens::new(access, "refresh", "user", false))
        }
    }

    fn untick_storage(root: &std::path::Path) -> Storage {
        let storage = Storage {
            credentials: root.join("credentials.json"),
            cache: root.join("lists.json"),
            config: root.join("config.toml"),
        };
        fs::write(&storage.cache, snapshot("queste sono le cose", true)).unwrap();
        fs::write(
            &storage.config,
            format!("[aliases]\nphilly = \"{CREAM_CHEESE}\"\n"),
        )
        .unwrap();
        storage
    }

    #[tokio::test]
    async fn untick_sends_op_then_caches_fetched_lists_and_reports_from_them() {
        let root = tempfile::tempdir().unwrap();
        let storage = untick_storage(root.path());
        let session = FakeUntick::new(None, Some(snapshot("renamed", false)));
        let report = untick(&session, &storage, "philly").await.unwrap();
        assert_eq!(report, "Unticked: Cream cheese on 'renamed'");
        assert_eq!(
            *session.unticked.borrow(),
            [("market".to_string(), CREAM_CHEESE.to_string())]
        );
        assert_eq!(
            fs::read_to_string(&storage.cache).unwrap(),
            snapshot("renamed", false)
        );
        assert_eq!(storage.tokens().unwrap().access_token(), "after 2 calls");
    }

    #[tokio::test]
    async fn untick_reports_unknown_item_when_fetched_list_lacks_it() {
        let root = tempfile::tempdir().unwrap();
        let storage = untick_storage(root.path());
        let emptied =
            r#"{"current_list_id":"market","lists":[{"id":"market","name":"renamed","items":[]}]}"#;
        let session = FakeUntick::new(None, Some(emptied.into()));
        let error = untick(&session, &storage, "philly").await.unwrap_err();
        assert_eq!(
            error.to_string(),
            "No item with alias or ID 'philly' in 'renamed'"
        );
        assert_eq!(fs::read_to_string(&storage.cache).unwrap(), emptied);
        assert_eq!(storage.tokens().unwrap().access_token(), "after 2 calls");
    }

    #[tokio::test]
    async fn failed_op_or_follow_up_fetch_keeps_cache_and_saves_tokens() {
        for (untick_error, expected, calls) in [
            (Some("op failed"), "op failed", "after 1 calls"),
            (None, "fetch failed", "after 2 calls"),
        ] {
            let root = tempfile::tempdir().unwrap();
            let storage = untick_storage(root.path());
            let session = FakeUntick::new(untick_error, None);
            let error = untick(&session, &storage, CREAM_CHEESE).await.unwrap_err();
            assert_eq!(error.to_string(), expected);
            assert_eq!(session.unticked.borrow().len(), 1);
            assert_eq!(
                fs::read_to_string(&storage.cache).unwrap(),
                snapshot("queste sono le cose", true)
            );
            assert_eq!(storage.tokens().unwrap().access_token(), calls);
        }
    }
}
