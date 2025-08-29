use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::auth::{Access, Account, AuthenticationMode, Username};
use crate::filesystem;
use crate::filesystem::{FileWriteError, UnhandlableWriteSnafu, WritableFile};

#[derive(Snafu, Debug)]
pub enum ConfigReadError {
    #[snafu(display("Failed to read the configuration file: {}", source))]
    Io {
        source: tokio::io::Error,
    },
    #[snafu(display("Failed to deserialize the configuration file: {}", source))]
    Serde {
        source: toml::de::Error,
    },
}

#[derive(Deserialize, Clone)]
pub struct Config {
    pub address: String,
    #[serde(with="base64serde")]
    pub secret_key: Vec<u8>,
    pub auth_mode: AuthenticationMode,
    pub create_access: Access,
    pub administrator_access: Access,
    pub discovery_access: Access,
    pub articles: PathBuf,
    pub assets: PathBuf,
    pub templates: String,
}

impl Config {
    pub fn generate_secret_key(&mut self) -> String {
        use base64::prelude::*;
        use rand_core::RngCore;
        let mut random_key = vec![0u8; 64];
        rand_core::OsRng::default().fill_bytes(&mut random_key);
        let key_string = BASE64_STANDARD.encode(&random_key);

        self.secret_key = random_key;
        key_string
    }

    pub async fn from_file<P>(path: P) -> Result<Config, ConfigReadError>
    where P : AsRef<Path> {
        let mut file = filesystem::ReadableFile::open(path.as_ref()).await.context(IoSnafu)?;
        let mut str = String::new();
        file.reader.read_to_string(&mut str).await.context(IoSnafu)?;
        Ok(toml::from_str(&str).context(SerdeSnafu)?)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AccountConfig {
    pub single_password: Option<String>,
    pub accounts: Vec<Account>,
}

impl AccountConfig {
    /// Validates that the specified single-user password is set and is a valid password hash.
    pub fn validate_single_user_password(&self) -> bool {
        use argon2::PasswordHash;
        match &self.single_password {
            None => false,
            Some(hash) => PasswordHash::new(hash).is_ok()
        }
    }

    pub fn generate_single_user_password(&mut self) -> String {
        use base64::prelude::*;
        use rand_core::RngCore;

        // Generate a new 120-bit password and encode with base64
        let mut bits = vec![0u8; 15];
        rand_core::OsRng::default().fill_bytes(&mut bits);
        let password = BASE64_STANDARD.encode(&bits);

        self.single_password = Some(crate::auth::hash_password(&password));
        password
    }

    pub fn find_by_username_mut(&mut self, username: &Username) -> Option<&mut Account> {
        self.accounts.iter_mut().find(|acc| &acc.username == username)
    }

    pub fn find_by_username(&self, username: &Username) -> Option<&Account> {
        self.accounts.iter().find(|acc| &acc.username == username)
    }

    pub async fn from_file<P>(path: P) -> Result<AccountConfig, ConfigReadError>
    where P : AsRef<Path> {
        let mut file = filesystem::ReadableFile::open(path.as_ref()).await.context(IoSnafu)?;
        let mut str = String::new();
        file.reader.read_to_string(&mut str).await.context(IoSnafu)?;
        Ok(toml::from_str(&str).context(SerdeSnafu)?)
    }

    pub async fn write_to_file<P>(&self, path: P) -> Result<(), FileWriteError>
    where P : AsRef<Path> {
        let filepath = path.as_ref();
        let mut file = WritableFile::open(filepath).await?;
        let toml = toml::to_string_pretty(&self).expect("TOML Serialization should always succeed.");
        (&mut file.writer).write_all(toml.as_bytes()).await.with_context(|_| UnhandlableWriteSnafu { filepath })?;
        file.close().await?;
        Ok(())
    }
}

mod base64serde {
    use serde::{Deserialize, Deserializer};
    use base64::Engine;

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        use base64::prelude::BASE64_STANDARD;

        let encoded = String::deserialize(d)?;
        BASE64_STANDARD.decode(encoded).map_err(serde::de::Error::custom)
    }
}
