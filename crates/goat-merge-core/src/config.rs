use std::fmt;
use std::time::Duration;

use serde::{Deserialize, Deserializer};

pub const FILE: &str = ".github/merge-queue.yml";
pub const LABEL: &str = "merge-queue";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    #[serde(default)]
    pub queues: Vec<Queue>,
    #[serde(default)]
    pub retry: Retry,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Queue {
    pub branch: String,
    #[serde(default)]
    pub enqueue: Enqueue,
    #[serde(default)]
    pub merge_method: Option<MergeMethod>,
    #[serde(default = "half_an_hour_and_a_quarter", deserialize_with = "duration")]
    pub check_timeout: Duration,
    #[serde(default = "five")]
    pub batch_size: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Enqueue {
    #[default]
    Manual,
    Automatic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum MergeMethod {
    Merge,
    Squash,
    Rebase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Retry {
    #[serde(default)]
    pub ci_failures: u32,
}

impl Config {
    pub fn parse(source: &str) -> Result<Self, ConfigError> {
        let config: Self = serde_yaml::from_str(source)?;
        if config.version != 1 {
            return Err(ConfigError::UnknownVersion {
                found: config.version,
            });
        }
        for (at, queue) in config.queues.iter().enumerate() {
            if config.queues[..at]
                .iter()
                .any(|earlier| earlier.branch == queue.branch)
            {
                return Err(ConfigError::BranchTwice {
                    branch: queue.branch.clone(),
                });
            }
        }
        Ok(config)
    }

    pub fn queue_for(&self, branch: &str) -> Option<&Queue> {
        self.queues.iter().find(|queue| queue.branch == branch)
    }
}

impl Queue {
    pub fn following_github(branch: impl Into<String>) -> Self {
        Self {
            branch: branch.into(),
            enqueue: Enqueue::Manual,
            merge_method: None,
            check_timeout: half_an_hour_and_a_quarter(),
            batch_size: five(),
        }
    }

    pub fn most_it_will_verify_at_once(&self) -> usize {
        self.batch_size.max(1)
    }
}

impl fmt::Display for MergeMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Merge => "merge",
            Self::Squash => "squash",
            Self::Rebase => "rebase",
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("is not valid YAML: {0}")]
    Malformed(#[from] serde_yaml::Error),
    #[error("says version {found}, and this server only understands version 1")]
    UnknownVersion { found: u32 },
    #[error("gives {branch} two queues. One branch, one queue")]
    BranchTwice { branch: String },
}

fn half_an_hour_and_a_quarter() -> Duration {
    Duration::from_secs(45 * 60)
}

fn five() -> usize {
    5
}

fn duration<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Duration, D::Error> {
    let written = String::deserialize(deserializer)?;
    read_duration(&written).ok_or_else(|| {
        serde::de::Error::custom(format!(
            "{written:?} is not a length of time. Write a number followed by s, m, or h, like 45m"
        ))
    })
}

fn read_duration(written: &str) -> Option<Duration> {
    let trimmed = written.trim();
    let (count, unit) = trimmed.split_at(trimmed.len().checked_sub(1)?);
    let count: u64 = count.trim().parse().ok()?;
    let seconds = match unit {
        "s" => count,
        "m" => count.checked_mul(60)?,
        "h" => count.checked_mul(60 * 60)?,
        _ => return None,
    };
    Some(Duration::from_secs(seconds))
}
