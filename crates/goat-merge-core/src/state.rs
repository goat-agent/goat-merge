use std::fmt;

use crate::config::MergeMethod;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    NotQueued(NotQueued),
    Waiting(WhyWaiting),
    Blocked(WhyBlocked),
    Queued {
        ahead: usize,
    },
    Preparing {
        alongside: Vec<u64>,
        assuming: Vec<u64>,
    },
    Validating {
        alongside: Vec<u64>,
        assuming: Vec<u64>,
    },
    Merging,
    Merged,
    Failed(WhyFailed),
    Cancelled {
        by: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotQueued {
    AwaitingLabel { branch: String },
    NoQueueForBranch { branch: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WhyWaiting {
    QueuePaused,
    Draft,
    NeedsApproval { have: u32, want: u32 },
    MergeabilityUnknown,
    RequiredChecksPending { done: usize, total: usize },
    ThoseAheadHaveNotLanded { numbers: Vec<u64> },
}

impl WhyWaiting {
    pub fn github_will_not_tell_us_when_this_changes(&self) -> bool {
        matches!(self, Self::MergeabilityUnknown)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WhyBlocked {
    Conflict,
    ForkWithoutSafeWorkflow,
    RequiredCheckFailed {
        checks: Vec<String>,
    },
    MergeMethodUnclear {
        allowed: Vec<MergeMethod>,
    },
    MergeMethodNotAllowed {
        chosen: MergeMethod,
        allowed: Vec<MergeMethod>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WhyFailed {
    ChecksFailed { checks: Vec<String> },
    ConflictWhilePreparing,
    TimedOut,
    MergeRejected { message: String },
}

impl Status {
    pub fn is_settled(&self) -> bool {
        matches!(
            self,
            Self::Merged | Self::Failed(_) | Self::Cancelled { .. }
        )
    }

    pub const MERGED: &'static str = "Merged";
    pub const FAILED: &'static str = "Failed";
    pub const CANCELLED: &'static str = "Cancelled";

    pub fn headline(&self) -> &'static str {
        match self {
            Self::NotQueued(_) => "Not queued",
            Self::Waiting(_) => "Waiting",
            Self::Blocked(_) => "Blocked",
            Self::Queued { .. } => "Queued",
            Self::Preparing { .. } => "Preparing",
            Self::Validating { .. } => "Checking",
            Self::Merging => "Merging",
            Self::Merged => Self::MERGED,
            Self::Failed(_) => Self::FAILED,
            Self::Cancelled { .. } => Self::CANCELLED,
        }
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotQueued(reason) => write!(f, "{reason}"),
            Self::Waiting(why) => write!(f, "{why}"),
            Self::Blocked(why) => write!(f, "{why}"),
            Self::Queued { ahead: 0 } => write!(f, "next in line"),
            Self::Queued { ahead: 1 } => write!(f, "1 pull request ahead"),
            Self::Queued { ahead } => write!(f, "{ahead} pull requests ahead"),
            Self::Preparing {
                alongside,
                assuming,
            } => write!(
                f,
                "building a candidate on {}{}",
                what_it_is_built_on(assuming),
                also_carrying(alongside)
            ),
            Self::Validating {
                alongside,
                assuming,
            } => write!(
                f,
                "checking the candidate{}{}",
                also_carrying(alongside),
                which_assumes(assuming)
            ),
            Self::Merging => write!(f, "checks passed, merging"),
            Self::Merged => write!(f, "merged"),
            Self::Failed(why) => write!(f, "{why}"),
            Self::Cancelled { by } => write!(f, "removed from the queue by {by}"),
        }
    }
}

impl fmt::Display for NotQueued {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AwaitingLabel { branch } => {
                write!(f, "add the merge-queue label to queue this for {branch}")
            }
            Self::NoQueueForBranch { branch } => write!(
                f,
                "no queue is configured for {branch}. Add it to .github/merge-queue.yml"
            ),
        }
    }
}

impl fmt::Display for WhyWaiting {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::QueuePaused => write!(f, "the queue is paused"),
            Self::Draft => write!(f, "the pull request is still a draft"),
            Self::NeedsApproval { have, want } => write!(f, "approvals: {have} of {want}"),
            Self::MergeabilityUnknown => {
                write!(
                    f,
                    "GitHub has not finished working out whether this merges cleanly"
                )
            }
            Self::RequiredChecksPending { done, total } => {
                write!(f, "{done}/{total} checks passed")
            }
            Self::ThoseAheadHaveNotLanded { numbers } => write!(
                f,
                "the checks passed, but this was verified on top of {}, so it waits for them \
                 to land",
                numbers_phrase(numbers)
            ),
        }
    }
}

impl fmt::Display for WhyBlocked {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict => write!(
                f,
                "this branch conflicts with its base. Resolve the conflict and it will queue again"
            ),
            Self::ForkWithoutSafeWorkflow => write!(
                f,
                "this pull request comes from a fork, and the repository has no queue workflow \
                 that runs fork code without secrets. Until it does, fork pull requests are not \
                 merged"
            ),
            Self::RequiredCheckFailed { checks } => {
                write!(f, "required {} failed", checks_phrase(checks))
            }
            Self::MergeMethodUnclear { allowed } => write!(
                f,
                "this repository allows {}. Say which one in .github/merge-queue.yml",
                methods_phrase(allowed)
            ),
            Self::MergeMethodNotAllowed { chosen, allowed } => write!(
                f,
                ".github/merge-queue.yml asks for {chosen}, which this repository does not allow. \
                 It allows {}",
                methods_phrase(allowed)
            ),
        }
    }
}

impl fmt::Display for WhyFailed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ChecksFailed { checks } => write!(f, "{} failed", checks_phrase(checks)),
            Self::ConflictWhilePreparing => write!(
                f,
                "this branch stopped merging cleanly while it was in the queue"
            ),
            Self::TimedOut => write!(f, "the checks did not finish in time"),
            Self::MergeRejected { message } => write!(f, "GitHub refused the merge: {message}"),
        }
    }
}

fn what_it_is_built_on(assuming: &[u64]) -> String {
    match assuming {
        [] => "the latest base".to_owned(),
        [one] => format!("top of #{one}, which has not landed yet"),
        many => format!("top of {}, which have not landed yet", numbers_phrase(many)),
    }
}

fn which_assumes(assuming: &[u64]) -> String {
    match assuming {
        [] => String::new(),
        [one] => format!(", which assumes #{one} lands first"),
        many => format!(", which assumes {} land first", numbers_phrase(many)),
    }
}

fn also_carrying(alongside: &[u64]) -> String {
    match alongside {
        [] => String::new(),
        many => format!(", which also carries {}", numbers_phrase(many)),
    }
}

fn numbers_phrase(numbers: &[u64]) -> String {
    match numbers {
        [] => String::new(),
        [one] => format!("#{one}"),
        [rest @ .., last] => format!(
            "{} and #{last}",
            rest.iter()
                .map(|number| format!("#{number}"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn checks_phrase(checks: &[String]) -> String {
    match checks {
        [] => "a check".to_owned(),
        [one] => format!("check {one:?}"),
        many => format!(
            "checks {}",
            many.iter()
                .map(|c| format!("{c:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn methods_phrase(methods: &[MergeMethod]) -> String {
    match methods {
        [] => "no merge method at all".to_owned(),
        [one] => one.to_string(),
        [rest @ .., last] => format!(
            "{} and {last}",
            rest.iter()
                .map(MergeMethod::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}
