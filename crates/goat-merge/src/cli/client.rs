use std::path::PathBuf;
use std::process::Command;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::api::COOKIE;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Remembered {
    pub url: String,
    pub token: String,
}

pub fn where_we_remember_things() -> PathBuf {
    std::env::var("GOAT_MERGE_HOME")
        .map_or_else(
            |_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_owned());
                PathBuf::from(home).join(".goat").join("merge")
            },
            PathBuf::from,
        )
        .join("credentials.json")
}

pub fn remember(what: &Remembered) -> Result<(), Trouble> {
    let path = where_we_remember_things();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|problem| Trouble::CannotWrite {
            path: parent.display().to_string(),
            problem: problem.to_string(),
        })?;
    }
    let written = serde_json::to_string_pretty(what).unwrap_or_default();
    std::fs::write(&path, written).map_err(|problem| Trouble::CannotWrite {
        path: path.display().to_string(),
        problem: problem.to_string(),
    })
}

pub fn recall() -> Result<Remembered, Trouble> {
    let path = where_we_remember_things();
    let written = std::fs::read_to_string(&path).map_err(|_| Trouble::NotSignedIn)?;
    serde_json::from_str(&written).map_err(|_| Trouble::NotSignedIn)
}

pub struct Server {
    remembered: Remembered,
    http: reqwest::Client,
}

impl Server {
    pub fn we_know() -> Result<Self, Trouble> {
        Ok(Self {
            remembered: recall()?,
            http: reqwest::Client::builder()
                .user_agent(crate::github::USER_AGENT)
                .build()
                .map_err(|problem| Trouble::Unreachable {
                    problem: problem.to_string(),
                })?,
        })
    }

    pub fn url(&self) -> &str {
        &self.remembered.url
    }

    pub async fn get(&self, path: &str) -> Result<Value, Trouble> {
        self.send(self.http.get(format!("{}{path}", self.remembered.url)))
            .await
    }

    pub async fn post(&self, path: &str, body: Value) -> Result<Value, Trouble> {
        self.send(
            self.http
                .post(format!("{}{path}", self.remembered.url))
                .json(&body),
        )
        .await
    }

    async fn send(&self, request: reqwest::RequestBuilder) -> Result<Value, Trouble> {
        let response = request
            .header("cookie", format!("{COOKIE}={}", self.remembered.token))
            .send()
            .await
            .map_err(|problem| Trouble::Unreachable {
                problem: problem.to_string(),
            })?;
        let status = response.status();
        let body: Value = response.json().await.unwrap_or(Value::Null);
        if body.get("kind").and_then(Value::as_str) == Some("not_signed_in") {
            return Err(Trouble::NotSignedIn);
        }
        if !status.is_success() {
            let Some(said) = body.get("error").and_then(Value::as_str) else {
                return Err(Trouble::Unreadable { status });
            };
            return Err(Trouble::Refused {
                what: said.to_owned(),
            });
        }
        Ok(body)
    }
}

pub fn repository_here(given: Option<&str>) -> Result<String, Trouble> {
    if let Some(given) = given {
        return Ok(given.to_owned());
    }
    let remote = run_git(&["remote", "get-url", "origin"]).ok_or(Trouble::NotInARepository)?;
    from_a_remote(&remote).ok_or(Trouble::NotAGithubRemote { remote })
}

pub fn branch_here() -> Option<String> {
    run_git(&["rev-parse", "--abbrev-ref", "HEAD"])
}

fn run_git(arguments: &[&str]) -> Option<String> {
    let output = Command::new("git").args(arguments).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let said = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    (!said.is_empty()).then_some(said)
}

pub fn from_a_remote(remote: &str) -> Option<String> {
    let trimmed = remote.trim().trim_end_matches(".git");
    let after_host = if let Some(rest) = trimmed.strip_prefix("git@") {
        rest.split_once(':').map(|(_, path)| path)?
    } else {
        let rest = trimmed.split_once("://").map(|(_, rest)| rest)?;
        rest.split_once('/').map(|(_, path)| path)?
    };
    let mut parts = after_host.split('/');
    let owner = parts.next()?;
    let name = parts.next()?;
    (!owner.is_empty() && !name.is_empty()).then(|| format!("{owner}/{name}"))
}

#[derive(Debug, thiserror::Error)]
pub enum Trouble {
    #[error("run `goat-merge login <url>` first")]
    NotSignedIn,
    #[error("this is not a git repository, so say which one with --repo owner/name")]
    NotInARepository,
    #[error("{remote} is not a GitHub remote, so say which repository with --repo owner/name")]
    NotAGithubRemote { remote: String },
    #[error("the server could not be reached: {problem}")]
    Unreachable { problem: String },
    #[error("{what}")]
    Refused { what: String },
    #[error(
        "the server answered {status}, and this version of goat-merge cannot read that answer. \
         The server is probably a different version"
    )]
    Unreadable { status: reqwest::StatusCode },
    #[error("{path} could not be written: {problem}")]
    CannotWrite { path: String, problem: String },
    #[error("{path} could not be read: {problem}")]
    CannotRead { path: String, problem: String },
    #[error("say which pull request, or run this from the branch it was opened from")]
    WhichPullRequest,
    #[error("this repository has no queue here yet, so say which branch with --branch")]
    WhichBranch,
    #[error("this repository has queues for {branches}, so say which one with --branch")]
    WhichOfThese { branches: String },
}
