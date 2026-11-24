use anyhow::{Context, Result, anyhow};
use anylist_rs::SavedTokens;
use std::{env, fs, io, path::PathBuf};

pub const LOGIN_REQUIRED: &str = "Not logged in. Run: anylist login";

pub struct Storage {
    pub credentials: PathBuf,
    pub cache: PathBuf,
}

fn directory(variable: &str, fallback: &str) -> Result<PathBuf> {
    if let Some(path) = env::var_os(variable)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
    {
        return Ok(path.join("anylist-cli"));
    }
    let home = env::var_os("HOME").context("HOME is unset")?;
    Ok(PathBuf::from(home).join(fallback).join("anylist-cli"))
}

impl Storage {
    pub fn discover() -> Result<Self> {
        Ok(Self {
            credentials: directory("XDG_DATA_HOME", ".local/share")?.join("credentials.json"),
            cache: directory("XDG_CACHE_HOME", ".cache")?.join("lists.json"),
        })
    }

    pub fn tokens(&self) -> Result<SavedTokens> {
        let file = match fs::File::open(&self.credentials) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(anyhow!(LOGIN_REQUIRED));
            }
            result => result.context("Cannot open credentials")?,
        };
        serde_json::from_reader(file).context("Cannot read credentials")
    }
}

fn remove(path: &std::path::Path) -> Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result.with_context(|| format!("Cannot remove {}", path.display())),
    }
}

impl Storage {
    pub fn logout(&self) -> Result<()> {
        remove(&self.cache)?;
        remove(&self.credentials)
    }
}

fn write_json(path: &std::path::Path, value: &impl serde::Serialize) -> Result<()> {
    let directory = path.parent().context("Missing storage directory")?;
    fs::create_dir_all(directory)?;
    let mut file = tempfile::NamedTempFile::new_in(directory)?;
    serde_json::to_writer(file.as_file_mut(), value)?;
    file.persist(path)?;
    Ok(())
}

impl Storage {
    pub fn begin_session(&self, tokens: &SavedTokens) -> Result<()> {
        remove(&self.cache)?;
        self.save_tokens(tokens)
    }

    pub fn save_tokens(&self, tokens: &SavedTokens) -> Result<()> {
        write_json(&self.credentials, tokens).context("Cannot save credentials")
    }

    pub fn save_snapshot(&self, snapshot: &crate::adapter::Snapshot) -> Result<()> {
        write_json(&self.cache, snapshot).context("Cannot cache shopping lists")
    }
}
