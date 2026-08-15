use std::net::SocketAddr;

use crate::store::{SealError, key_from_hex};

pub const GITHUB_API: &str = "https://api.github.com";
pub const GITHUB_WEB: &str = "https://github.com";

#[derive(Debug, Clone)]
pub struct Settings {
    pub public_url: String,
    pub listen: SocketAddr,
    pub database_url: String,
    pub master_key: [u8; 32],
    pub github_api: String,
    pub github_web: String,
}

impl Settings {
    pub fn from_the_environment() -> Result<Self, SettingsError> {
        let public_url = first_of(&["GOAT_MERGE_PUBLIC_URL", "PUBLIC_URL"])
            .ok_or(SettingsError::NoPublicUrl)?
            .trim_end_matches('/')
            .to_owned();
        if !public_url.starts_with("http") {
            return Err(SettingsError::PublicUrlIsNotAUrl { given: public_url });
        }
        let listen = first_of(&["LISTEN_ADDR"])
            .unwrap_or_else(|| "0.0.0.0:8080".to_owned())
            .parse()
            .map_err(|_| SettingsError::ListenAddressIsNotAnAddress)?;
        let database_url = first_of(&["DATABASE_URL"]).ok_or(SettingsError::NoDatabase)?;
        let master_key = key_from_hex(&first_of(&["GOAT_MERGE_MASTER_KEY"]).ok_or_else(|| {
            SettingsError::NoMasterKey {
                suggestion: crate::store::fresh_key(),
            }
        })?)?;
        Ok(Self {
            public_url,
            listen,
            database_url,
            master_key,
            github_api: first_of(&["GOAT_MERGE_GITHUB_API"])
                .unwrap_or_else(|| GITHUB_API.to_owned()),
            github_web: first_of(&["GOAT_MERGE_GITHUB_WEB"])
                .unwrap_or_else(|| GITHUB_WEB.to_owned()),
        })
    }

    pub fn webhook_url(&self) -> String {
        format!("{}/api/github/webhook", self.public_url)
    }

    pub fn callback_url(&self) -> String {
        format!("{}/auth/github/callback", self.public_url)
    }

    pub fn setup_url(&self) -> String {
        format!("{}/setup/github", self.public_url)
    }

    pub fn queue_url(&self, repository: &str, branch: &str, pull_request: i32) -> String {
        format!(
            "{}/queue/{repository}/{branch}?pull={pull_request}",
            self.public_url
        )
    }
}

fn first_of(names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|name| std::env::var(name).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error(
        "GOAT_MERGE_PUBLIC_URL is not set. It is the https address GitHub will reach this \
         server on, for example https://merge.example.com"
    )]
    NoPublicUrl,
    #[error("GOAT_MERGE_PUBLIC_URL is {given:?}, which is not an http address")]
    PublicUrlIsNotAUrl { given: String },
    #[error("LISTEN_ADDR is not an address to listen on, for example 0.0.0.0:8080")]
    ListenAddressIsNotAnAddress,
    #[error("DATABASE_URL is not set. goat-merge keeps its queues in PostgreSQL")]
    NoDatabase,
    #[error(
        "GOAT_MERGE_MASTER_KEY is not set. It encrypts the GitHub App private key before it \
         is written down, so it has to survive restarts. Here is a fresh one to keep:\n\n    \
         GOAT_MERGE_MASTER_KEY={suggestion}\n"
    )]
    NoMasterKey { suggestion: String },
    #[error(transparent)]
    Sealed(#[from] SealError),
}
