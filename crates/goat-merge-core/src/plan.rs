use crate::config::{Enqueue, MergeMethod, Queue};
use crate::readiness::{Readiness, assess, conclusion_about, required_checks};
use crate::snapshot::{Conclusion, Sha, Snapshot};
use crate::state::{NotQueued, Status, WhyBlocked, WhyFailed, WhyWaiting};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aboard {
    pub number: u64,
    pub head: Sha,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HowItWent {
    StillGoing,
    Landed { at: Sha, head: Sha },
    GoneFromTheQueue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assumed {
    pub number: u64,
    pub head: Sha,
    pub went: HowItWent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verification {
    pub base: Sha,
    pub aboard: Vec<Aboard>,
    pub assumed: Vec<Assumed>,
    pub candidate: Sha,
    pub conclusion: Conclusion,
    pub ran_out_of_time: bool,
    pub failed_checks: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InQueue<'a> {
    pub ahead: usize,
    pub aboard: &'a [Aboard],
    pub assuming: &'a [Aboard],
    pub onto: &'a Sha,
    pub paused: bool,
    pub verification: Option<&'a Verification>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TheAssumption {
    Held,
    NotYet { waiting_on: Vec<u64> },
    Broken,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Next {
    Nothing,
    BuildCandidate { onto: Sha, aboard: Vec<Aboard> },
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

    if !entry.aboard.iter().any(|one| one.head == snapshot.head) {
        return settled(Status::Queued { ahead: entry.ahead });
    }
    let alongside = alongside(entry.aboard, snapshot.number);
    let assuming = numbers(entry.assuming);
    let preparing = Status::Preparing {
        alongside: alongside.clone(),
        assuming: assuming.clone(),
    };
    let alone = entry.aboard.len() <= 1 && entry.assuming.is_empty();

    let Some(verification) = entry.verification else {
        return if alone && already_verified(snapshot) {
            Decision {
                status: Status::Merging,
                next: Next::Merge { method },
            }
        } else {
            Decision {
                status: preparing,
                next: Next::BuildCandidate {
                    onto: entry.onto.clone(),
                    aboard: entry.aboard.to_vec(),
                },
            }
        };
    };

    if verification.base != *entry.onto
        || verification.aboard != entry.aboard
        || !the_same_ones(&verification.assumed, entry.assuming)
    {
        return Decision {
            status: preparing,
            next: Next::DiscardVerification,
        };
    }

    let at_fault_if_it_fails = verification.aboard.len() <= 1 && verification.assumed.is_empty();

    match verification.conclusion {
        Conclusion::Success => {
            match how_the_assumption_turned_out(&verification.assumed, &snapshot.base.sha) {
                TheAssumption::Held => Decision {
                    status: Status::Merging,
                    next: Next::Merge { method },
                },
                TheAssumption::NotYet { waiting_on } => {
                    settled(Status::Waiting(WhyWaiting::ThoseAheadHaveNotLanded {
                        numbers: waiting_on,
                    }))
                }
                TheAssumption::Broken => Decision {
                    status: preparing,
                    next: Next::DiscardVerification,
                },
            }
        }
        Conclusion::Failure if !at_fault_if_it_fails => Decision {
            status: Status::Preparing {
                alongside: Vec::new(),
                assuming: Vec::new(),
            },
            next: Next::DiscardVerification,
        },
        Conclusion::Failure if verification.ran_out_of_time => {
            settled(Status::Failed(WhyFailed::TimedOut))
        }
        Conclusion::Failure => settled(Status::Failed(WhyFailed::ChecksFailed {
            checks: verification.failed_checks.clone(),
        })),
        Conclusion::Pending => settled(Status::Validating {
            alongside,
            assuming,
        }),
    }
}

pub fn how_the_assumption_turned_out(assumed: &[Assumed], tip: &Sha) -> TheAssumption {
    let mut waiting_on = Vec::new();
    for one in assumed {
        match &one.went {
            HowItWent::GoneFromTheQueue => return TheAssumption::Broken,
            HowItWent::Landed { head, .. } if *head != one.head => return TheAssumption::Broken,
            HowItWent::Landed { .. } => {}
            HowItWent::StillGoing => waiting_on.push(one.number),
        }
    }
    if !waiting_on.is_empty() {
        return TheAssumption::NotYet { waiting_on };
    }
    match assumed.last() {
        None => TheAssumption::Held,
        Some(Assumed {
            went: HowItWent::Landed { at, .. },
            ..
        }) if at == tip => TheAssumption::Held,
        Some(_) => TheAssumption::Broken,
    }
}

fn the_same_ones(assumed: &[Assumed], assuming: &[Aboard]) -> bool {
    assumed.len() == assuming.len()
        && assumed
            .iter()
            .zip(assuming)
            .all(|(was, now)| was.number == now.number && was.head == now.head)
}

fn numbers(aboard: &[Aboard]) -> Vec<u64> {
    aboard.iter().map(|one| one.number).collect()
}

fn alongside(aboard: &[Aboard], mine: u64) -> Vec<u64> {
    aboard
        .iter()
        .map(|one| one.number)
        .filter(|number| *number != mine)
        .collect()
}

pub fn how_many_to_verify(rules: &Queue, now: usize, ready: usize) -> usize {
    now.clamp(1, rules.most_it_will_verify_at_once()).min(ready)
}

pub fn after_a_batch(now: usize, size: usize, rules: &Queue, passed: bool) -> usize {
    if passed {
        size.saturating_add(1)
            .max(now)
            .min(rules.most_it_will_verify_at_once())
    } else {
        (size / 2).max(1)
    }
}

pub fn how_deep_to_speculate(rules: &Queue, now: usize, verifying_at_once: usize) -> usize {
    if verifying_at_once <= 1 && rules.most_it_will_verify_at_once() > 1 {
        return 1;
    }
    now.clamp(1, rules.most_it_will_speculate())
}

pub fn after_a_chain(now: usize, rules: &Queue, wasted: bool) -> usize {
    if wasted {
        (now / 2).max(1)
    } else {
        now.saturating_add(1).min(rules.most_it_will_speculate())
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
