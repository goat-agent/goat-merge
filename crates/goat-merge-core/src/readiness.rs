use crate::snapshot::{Check, Conclusion, Mergeable, Sha, Snapshot};
use crate::state::{WhyBlocked, WhyWaiting};

pub const CHECK_NAME: &str = "Merge Queue";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Readiness {
    Ready,
    Waiting(WhyWaiting),
    Blocked(WhyBlocked),
}

pub fn required_checks(snapshot: &Snapshot) -> impl Iterator<Item = &Check> {
    snapshot
        .required_checks
        .iter()
        .filter(|check| check.name != CHECK_NAME)
}

pub fn conclusion_about(check: &Check, head: &Sha) -> Conclusion {
    if check.head == *head {
        check.conclusion
    } else {
        Conclusion::Pending
    }
}

pub fn assess(snapshot: &Snapshot) -> Readiness {
    if snapshot.from_fork && !snapshot.fork_workflow_declared_safe {
        return Readiness::Blocked(WhyBlocked::ForkWithoutSafeWorkflow);
    }
    match snapshot.mergeable {
        Mergeable::Conflict => return Readiness::Blocked(WhyBlocked::Conflict),
        Mergeable::Unknown => return Readiness::Waiting(WhyWaiting::MergeabilityUnknown),
        Mergeable::Clean => {}
    }
    if snapshot.draft {
        return Readiness::Waiting(WhyWaiting::Draft);
    }
    if snapshot.approvals < snapshot.approvals_required {
        return Readiness::Waiting(WhyWaiting::NeedsApproval {
            have: snapshot.approvals,
            want: snapshot.approvals_required,
        });
    }

    let checks: Vec<&Check> = required_checks(snapshot).collect();
    let failed: Vec<String> = checks
        .iter()
        .filter(|check| conclusion_about(check, &snapshot.head) == Conclusion::Failure)
        .map(|check| check.name.clone())
        .collect();
    if !failed.is_empty() {
        return Readiness::Blocked(WhyBlocked::RequiredCheckFailed { checks: failed });
    }
    let passed = checks
        .iter()
        .filter(|check| conclusion_about(check, &snapshot.head) == Conclusion::Success)
        .count();
    if passed < checks.len() {
        return Readiness::Waiting(WhyWaiting::RequiredChecksPending {
            done: passed,
            total: checks.len(),
        });
    }
    Readiness::Ready
}
