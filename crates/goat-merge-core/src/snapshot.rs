use std::fmt;

use time::OffsetDateTime;

use crate::config::MergeMethod;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Sha(String);

impl Sha {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<T: Into<String>> From<T> for Sha {
    fn from(value: T) -> Self {
        Self(value.into())
    }
}

impl fmt::Display for Sha {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conclusion {
    Success,
    Failure,
    Pending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mergeable {
    Clean,
    Conflict,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub name: String,
    pub conclusion: Conclusion,
    pub head: Sha,
    pub started_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Base {
    pub branch: String,
    pub sha: Sha,
    pub last_moved_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub number: u64,
    pub head: Sha,
    pub base: Base,
    pub draft: bool,
    pub mergeable: Mergeable,
    pub approvals: u32,
    pub approvals_required: u32,
    pub labelled: bool,
    pub from_fork: bool,
    pub fork_workflow_declared_safe: bool,
    pub required_checks: Vec<Check>,
    pub allowed_merge_methods: Vec<MergeMethod>,
}
