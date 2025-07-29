use std::path::{Path, PathBuf};
use serde::Deserialize;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use crate::auth::{Access, Account, AuthenticationMode};

#[derive(thiserror::Error, Debug)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] tokio::io::Error),
    #[error("Deserialization error: {0}")]
    Serde(#[from] toml::de::Error),
}

#[derive(Deserialize, Clone)]
pub struct Config {
    pub address: String,
    #[serde(with="base64")]
    pub secret_key: Vec<u8>,
    pub auth_mode: AuthenticationMode,
    pub create_access: Access,
    pub articles: PathBuf,
    pub assets: PathBuf,
    pub templates: String,
}

impl Config {
    pub async fn from_file<P>(path: P) -> Result<Config, ConfigError>
    where P : AsRef<Path> {
        let mut file = File::open(path).await?;
        let mut str = String::new();
        file.read_to_string(&mut str).await?;
        Ok(toml::from_str(&str)?)
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct AccountConfig {
    pub single_password: Option<String>,
    pub accounts: Vec<Account>,
}

impl AccountConfig {
    pub async fn from_file<P>(path: P) -> Result<AccountConfig, ConfigError>
    where P : AsRef<Path> {
        let mut file = File::open(path).await?;
        let mut str = String::new();
        file.read_to_string(&mut str).await?;
        Ok(toml::from_str(&str)?)
    }
}

mod base64 {
    use serde::{Deserialize, Deserializer};
    use base64::Engine;

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        use base64::prelude::BASE64_STANDARD;

        let encoded = String::deserialize(d)?;
        BASE64_STANDARD.decode(encoded).map_err(serde::de::Error::custom)
    }
}
