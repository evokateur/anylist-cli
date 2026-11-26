use crate::{adapter::Snapshot, storage::Storage};
use anyhow::Result;
use anylist_rs::SavedTokens;

pub trait Session {
    async fn fetch(&self) -> Result<Snapshot>;
    fn tokens(&self) -> Result<SavedTokens>;
}

pub async fn sync(session: &impl Session, storage: &Storage) -> Result<String> {
    let snapshot = session.fetch().await;
    storage.save_tokens(&session.tokens()?)?;
    let snapshot = snapshot?;
    let name = snapshot.current_list()?.name.clone();
    storage.save_snapshot(&snapshot)?;
    Ok(name)
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
}
