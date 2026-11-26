use anyhow::{Context, Result, anyhow};
use anylist_rs::SavedTokens;
use std::{collections::HashMap, env, fmt, fs, io, path::PathBuf};

pub const LOGIN_REQUIRED: &str = "Not logged in. Run: anylist login";

#[derive(Debug)]
pub enum MissingState {
    Login,
    Cache,
}

impl fmt::Display for MissingState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Login => LOGIN_REQUIRED,
            Self::Cache => "No cached lists. Run: anylist sync",
        })
    }
}

impl std::error::Error for MissingState {}

pub struct Storage {
    pub credentials: PathBuf,
    pub cache: PathBuf,
    pub config: PathBuf,
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
            config: directory("XDG_CONFIG_HOME", ".config")?.join("config.toml"),
        })
    }

    pub fn tokens(&self) -> Result<SavedTokens> {
        let file = match fs::File::open(&self.credentials) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(MissingState::Login.into());
            }
            result => result.context("Cannot open credentials")?,
        };
        serde_json::from_reader(file).context("Cannot read credentials")
    }

    /// Aliases from `[aliases]` in the config file, mapping alias to item ID.
    /// A missing config file means no aliases.
    pub fn aliases(&self) -> Result<HashMap<String, String>> {
        let text = match fs::read_to_string(&self.config) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(HashMap::new()),
            result => result.context("Cannot read config")?,
        };
        #[derive(serde::Deserialize)]
        struct Config {
            #[serde(default)]
            aliases: HashMap<String, String>,
        }
        let config: Config = toml::from_str(&text).context("Config error")?;
        Ok(config.aliases)
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
        let credentials = remove(&self.credentials);
        let cache = remove(&self.cache);
        match (credentials, cache) {
            (Err(first), Err(second)) => Err(anyhow!("{first:#}; {second:#}")),
            (credentials, cache) => credentials.and(cache),
        }
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

    pub fn snapshot(&self) -> Result<crate::adapter::Snapshot> {
        let file = match fs::File::open(&self.cache) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(MissingState::Cache.into());
            }
            result => result.context("Cannot open cached lists")?,
        };
        serde_json::from_reader(file).context("Cannot read cached lists")
    }

    pub fn save_snapshot(&self, snapshot: &crate::adapter::Snapshot) -> Result<()> {
        write_json(&self.cache, snapshot).context("Cannot cache shopping lists")
    }
}
