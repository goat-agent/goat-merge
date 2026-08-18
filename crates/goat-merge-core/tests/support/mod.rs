#![allow(dead_code)]

use goat_merge_core::{Base, Check, Conclusion, MergeMethod, Mergeable, Sha, Snapshot};
use time::macros::datetime;
use time::{Duration, OffsetDateTime};

pub fn oclock(hour: i64, minute: i64) -> OffsetDateTime {
    datetime!(2026-08-13 00:00 UTC) + Duration::hours(hour) + Duration::minutes(minute)
}

pub fn passing(name: &str, head: &str, started: OffsetDateTime) -> Check {
    Check {
        name: name.to_owned(),
        conclusion: Conclusion::Success,
        head: Sha::from(head),
        started_at: Some(started),
    }
}

pub fn pending(name: &str, head: &str) -> Check {
    Check {
        name: name.to_owned(),
        conclusion: Conclusion::Pending,
        head: Sha::from(head),
        started_at: None,
    }
}

pub fn failing(name: &str, head: &str) -> Check {
    Check {
        name: name.to_owned(),
        conclusion: Conclusion::Failure,
        head: Sha::from(head),
        started_at: Some(oclock(9, 30)),
    }
}

pub struct Building {
    snapshot: Snapshot,
}

pub fn a_pull_request() -> Building {
    Building {
        snapshot: Snapshot {
            number: 4835,
            head: Sha::from("head-one"),
            base: Base {
                branch: "main".to_owned(),
                sha: Sha::from("base-one"),
                last_moved_at: oclock(9, 0),
            },
            draft: false,
            mergeable: Mergeable::Clean,
            approvals: 2,
            approvals_required: 2,
            labelled: true,
            from_fork: false,
            fork_workflow_declared_safe: false,
            required_checks: vec![passing("test", "head-one", oclock(9, 30))],
            allowed_merge_methods: vec![MergeMethod::Squash],
        },
    }
}

impl Building {
    pub fn done(self) -> Snapshot {
        self.snapshot
    }

    pub fn numbered(mut self, number: u64) -> Self {
        self.snapshot.number = number;
        self
    }

    pub fn that_is_a_draft(mut self) -> Self {
        self.snapshot.draft = true;
        self
    }

    pub fn that_conflicts(mut self) -> Self {
        self.snapshot.mergeable = Mergeable::Conflict;
        self
    }

    pub fn whose_mergeability_is_unknown(mut self) -> Self {
        self.snapshot.mergeable = Mergeable::Unknown;
        self
    }

    pub fn without_the_label(mut self) -> Self {
        self.snapshot.labelled = false;
        self
    }

    pub fn approved_by(mut self, have: u32, of: u32) -> Self {
        self.snapshot.approvals = have;
        self.snapshot.approvals_required = of;
        self
    }

    pub fn sent_from_a_fork(mut self) -> Self {
        self.snapshot.from_fork = true;
        self
    }

    pub fn whose_fork_workflow_is_declared_safe(mut self) -> Self {
        self.snapshot.fork_workflow_declared_safe = true;
        self
    }

    pub fn at_head(mut self, head: &str) -> Self {
        self.snapshot.head = Sha::from(head);
        self
    }

    pub fn whose_base_last_moved_at(mut self, moment: OffsetDateTime) -> Self {
        self.snapshot.base.last_moved_at = moment;
        self
    }

    pub fn with_checks(mut self, checks: Vec<Check>) -> Self {
        self.snapshot.required_checks = checks;
        self
    }

    pub fn with_no_required_checks(mut self) -> Self {
        self.snapshot.required_checks = Vec::new();
        self
    }

    pub fn allowing(mut self, methods: Vec<MergeMethod>) -> Self {
        self.snapshot.allowed_merge_methods = methods;
        self
    }
}
