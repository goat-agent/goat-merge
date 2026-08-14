use crate::config::{Enqueue, MergeMethod, Queue};
use crate::readiness::{Readiness, assess, conclusion_about, required_checks};
use crate::snapshot::{Conclusion, Sha, Snapshot};
use crate::state::{NotQueued, Status, WhyBlocked, WhyFailed, WhyWaiting};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verification {
    pub base: Sha,
    pub head: Sha,
    pub candidate: Sha,
    pub conclusion: Conclusion,
    pub ran_out_of_time: bool,
    pub failed_checks: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InQueue<'a> {
    pub ahead: usize,
    pub paused: bool,
    pub verification: Option<&'a Verification>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Next {
    Nothing,
    BuildCandidate { onto: Sha, head: Sha },
    DiscardVerification,
    Merge { method: MergeMethod },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub status: Status,
    pub next: Next,
}

pub fn decide(snapshot: &Snapshot, queue: &Queue, entry: &InQueue<'_>) -> Decision {
    if queue.enqueue == Enqueue::Manual && !snapshot.labelled {
        return settled(Status::NotQueued(NotQueued::AwaitingLabel {
            branch: snapshot.base.branch.clone(),
        }));
    }

    match assess(snapshot) {
        Readiness::Blocked(why) => return settled(Status::Blocked(why)),
        Readiness::Waiting(why) => return settled(Status::Waiting(why)),
        Readiness::Ready => {}
    }

    if entry.paused {
        return settled(Status::Waiting(WhyWaiting::QueuePaused));
    }

    let method = match merge_method(queue, snapshot) {
        Ok(method) => method,
        Err(why) => return settled(Status::Blocked(why)),
    };

    if entry.ahead > 0 {
        return settled(Status::Queued { ahead: entry.ahead });
    }

    let Some(verification) = entry.verification else {
        return if already_verified(snapshot) {
            Decision {
                status: Status::Merging,
                next: Next::Merge { method },
            }
        } else {
            Decision {
                status: Status::Preparing,
                next: Next::BuildCandidate {
                    onto: snapshot.base.sha.clone(),
                    head: snapshot.head.clone(),
                },
            }
        };
    };

    if verification.base != snapshot.base.sha || verification.head != snapshot.head {
        return Decision {
            status: Status::Preparing,
            next: Next::DiscardVerification,
        };
    }

    match verification.conclusion {
        Conclusion::Success => Decision {
            status: Status::Merging,
            next: Next::Merge { method },
        },
        Conclusion::Failure if verification.ran_out_of_time => {
            settled(Status::Failed(WhyFailed::TimedOut))
        }
        Conclusion::Failure => settled(Status::Failed(WhyFailed::ChecksFailed {
            checks: verification.failed_checks.clone(),
        })),
        Conclusion::Pending => settled(Status::Validating),
    }
}

pub fn already_verified(snapshot: &Snapshot) -> bool {
    required_checks(snapshot).all(|check| {
        conclusion_about(check, &snapshot.head) == Conclusion::Success
            && check
                .started_at
                .is_some_and(|started| started >= snapshot.base.last_moved_at)
    })
}

pub fn merge_method(queue: &Queue, snapshot: &Snapshot) -> Result<MergeMethod, WhyBlocked> {
    let allowed = &snapshot.allowed_merge_methods;
    match queue.merge_method {
        Some(chosen) if allowed.contains(&chosen) => Ok(chosen),
        Some(chosen) => Err(WhyBlocked::MergeMethodNotAllowed {
            chosen,
            allowed: allowed.clone(),
        }),
        None => match allowed.as_slice() {
            [only] => Ok(*only),
            _ => Err(WhyBlocked::MergeMethodUnclear {
                allowed: allowed.clone(),
            }),
        },
    }
}

fn settled(status: Status) -> Decision {
    Decision {
        status,
        next: Next::Nothing,
    }
}
