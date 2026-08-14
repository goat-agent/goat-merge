mod support;

use goat_merge_core::{
    Conclusion, Enqueue, InQueue, MergeMethod, Next, NotQueued, Queue, Sha, Status, Verification,
    WhyBlocked, WhyFailed, WhyWaiting, decide,
};
use support::{a_pull_request, oclock, passing};

fn main_queue() -> Queue {
    Queue::following_github("main")
}

fn at_the_front<'a>() -> InQueue<'a> {
    InQueue {
        ahead: 0,
        paused: false,
        verification: None,
    }
}

fn verified(conclusion: Conclusion) -> Verification {
    Verification {
        base: Sha::from("base-one"),
        head: Sha::from("head-one"),
        candidate: Sha::from("candidate-one"),
        conclusion,
        ran_out_of_time: false,
        failed_checks: Vec::new(),
    }
}

fn ran_out_of_time() -> Verification {
    Verification {
        ran_out_of_time: true,
        ..verified(Conclusion::Failure)
    }
}

#[test]
fn an_unlabelled_pull_request_is_not_queued() {
    let snapshot = a_pull_request().without_the_label().done();

    let decision = decide(&snapshot, &main_queue(), &at_the_front());

    assert_eq!(
        decision.status,
        Status::NotQueued(NotQueued::AwaitingLabel {
            branch: "main".to_owned()
        })
    );
    assert_eq!(decision.next, Next::Nothing);
}

#[test]
fn automatic_enqueue_needs_no_label() {
    let snapshot = a_pull_request().without_the_label().done();
    let queue = Queue {
        enqueue: Enqueue::Automatic,
        ..main_queue()
    };

    assert_eq!(
        decide(&snapshot, &queue, &at_the_front()).status,
        Status::Merging
    );
}

#[test]
fn a_paused_queue_holds_everything_that_is_ready() {
    let snapshot = a_pull_request().done();
    let entry = InQueue {
        paused: true,
        ..at_the_front()
    };

    let decision = decide(&snapshot, &main_queue(), &entry);

    assert_eq!(decision.status, Status::Waiting(WhyWaiting::QueuePaused));
    assert_eq!(decision.next, Next::Nothing);
}

#[test]
fn a_pause_does_not_hide_why_a_pull_request_was_not_ready_anyway() {
    let snapshot = a_pull_request().that_is_a_draft().done();
    let entry = InQueue {
        paused: true,
        ..at_the_front()
    };

    assert_eq!(
        decide(&snapshot, &main_queue(), &entry).status,
        Status::Waiting(WhyWaiting::Draft)
    );
}

#[test]
fn only_the_front_of_the_queue_does_any_work() {
    let snapshot = a_pull_request()
        .whose_base_last_moved_at(oclock(10, 0))
        .with_checks(vec![passing("test", "head-one", oclock(10, 30))])
        .done();
    let behind = InQueue {
        ahead: 3,
        ..at_the_front()
    };

    let decision = decide(&snapshot, &main_queue(), &behind);

    assert_eq!(decision.status, Status::Queued { ahead: 3 });
    assert_eq!(
        decision.next,
        Next::Nothing,
        "a pull request that could merge on its own still waits its turn"
    );
}

#[test]
fn the_front_of_a_busy_queue_builds_a_candidate_on_the_current_base() {
    let snapshot = a_pull_request()
        .whose_base_last_moved_at(oclock(10, 0))
        .with_checks(vec![passing("test", "head-one", oclock(9, 30))])
        .done();

    let decision = decide(&snapshot, &main_queue(), &at_the_front());

    assert_eq!(decision.status, Status::Preparing);
    assert_eq!(
        decision.next,
        Next::BuildCandidate {
            onto: Sha::from("base-one"),
            head: Sha::from("head-one")
        }
    );
}

#[test]
fn a_quiet_queue_merges_without_building_anything() {
    let snapshot = a_pull_request().done();

    let decision = decide(&snapshot, &main_queue(), &at_the_front());

    assert_eq!(decision.status, Status::Merging);
    assert_eq!(
        decision.next,
        Next::Merge {
            method: MergeMethod::Squash
        }
    );
}

#[test]
fn a_pull_request_whose_base_moved_is_revalidated_not_merged() {
    let snapshot = a_pull_request()
        .whose_base_last_moved_at(oclock(11, 0))
        .with_checks(vec![passing("test", "head-one", oclock(9, 30))])
        .done();
    let stale = Verification {
        base: Sha::from("base-zero"),
        ..verified(Conclusion::Success)
    };
    let entry = InQueue {
        verification: Some(&stale),
        ..at_the_front()
    };

    let decision = decide(&snapshot, &main_queue(), &entry);

    assert_eq!(decision.next, Next::DiscardVerification);
    assert_eq!(decision.status, Status::Preparing);
}

#[test]
fn a_pull_request_whose_head_moved_is_revalidated_not_merged() {
    let snapshot = a_pull_request()
        .at_head("head-two")
        .whose_base_last_moved_at(oclock(11, 0))
        .with_checks(vec![passing("test", "head-two", oclock(11, 30))])
        .done();
    let stale = verified(Conclusion::Success);
    let entry = InQueue {
        verification: Some(&stale),
        ..at_the_front()
    };

    assert_eq!(
        decide(&snapshot, &main_queue(), &entry).next,
        Next::DiscardVerification
    );
}

#[test]
fn a_passing_verification_merges_by_the_configured_method() {
    let snapshot = a_pull_request()
        .whose_base_last_moved_at(oclock(11, 0))
        .allowing(vec![MergeMethod::Merge, MergeMethod::Squash])
        .with_checks(vec![passing("test", "head-one", oclock(9, 30))])
        .done();
    let queue = Queue {
        merge_method: Some(MergeMethod::Merge),
        ..main_queue()
    };
    let passed = verified(Conclusion::Success);
    let entry = InQueue {
        verification: Some(&passed),
        ..at_the_front()
    };

    let decision = decide(&snapshot, &queue, &entry);

    assert_eq!(decision.status, Status::Merging);
    assert_eq!(
        decision.next,
        Next::Merge {
            method: MergeMethod::Merge
        }
    );
}

#[test]
fn a_failing_verification_names_the_checks_that_failed() {
    let snapshot = a_pull_request()
        .whose_base_last_moved_at(oclock(11, 0))
        .with_checks(vec![passing("test", "head-one", oclock(9, 30))])
        .done();
    let failed = Verification {
        failed_checks: vec!["integration tests".to_owned()],
        ..verified(Conclusion::Failure)
    };
    let entry = InQueue {
        verification: Some(&failed),
        ..at_the_front()
    };

    let decision = decide(&snapshot, &main_queue(), &entry);

    assert_eq!(
        decision.status,
        Status::Failed(WhyFailed::ChecksFailed {
            checks: vec!["integration tests".to_owned()]
        })
    );
    assert_eq!(decision.next, Next::Nothing);
}

#[test]
fn a_candidate_still_running_is_left_alone() {
    let snapshot = a_pull_request()
        .whose_base_last_moved_at(oclock(11, 0))
        .with_checks(vec![passing("test", "head-one", oclock(9, 30))])
        .done();
    let running = verified(Conclusion::Pending);
    let entry = InQueue {
        verification: Some(&running),
        ..at_the_front()
    };

    let decision = decide(&snapshot, &main_queue(), &entry);

    assert_eq!(decision.status, Status::Validating);
    assert_eq!(decision.next, Next::Nothing);
}

#[test]
fn a_repository_allowing_one_merge_method_needs_no_configuration() {
    let snapshot = a_pull_request().allowing(vec![MergeMethod::Rebase]).done();

    assert_eq!(
        decide(&snapshot, &main_queue(), &at_the_front()).next,
        Next::Merge {
            method: MergeMethod::Rebase
        }
    );
}

#[test]
fn a_repository_allowing_several_asks_which_one() {
    let allowed = vec![MergeMethod::Merge, MergeMethod::Squash];
    let snapshot = a_pull_request().allowing(allowed.clone()).done();

    let decision = decide(&snapshot, &main_queue(), &at_the_front());

    assert_eq!(
        decision.status,
        Status::Blocked(WhyBlocked::MergeMethodUnclear { allowed })
    );
    assert_eq!(
        decision.next,
        Next::Nothing,
        "guessing squash would silently rewrite the project's history"
    );
}

#[test]
fn a_configured_method_the_repository_forbids_is_blocked() {
    let snapshot = a_pull_request().allowing(vec![MergeMethod::Merge]).done();
    let queue = Queue {
        merge_method: Some(MergeMethod::Squash),
        ..main_queue()
    };

    assert_eq!(
        decide(&snapshot, &queue, &at_the_front()).status,
        Status::Blocked(WhyBlocked::MergeMethodNotAllowed {
            chosen: MergeMethod::Squash,
            allowed: vec![MergeMethod::Merge],
        })
    );
}

#[test]
fn a_merge_method_nobody_can_satisfy_blocks_the_whole_queue_at_once() {
    let allowed = vec![MergeMethod::Merge, MergeMethod::Squash];
    let snapshot = a_pull_request().allowing(allowed.clone()).done();
    let behind = InQueue {
        ahead: 4,
        ..at_the_front()
    };

    assert_eq!(
        decide(&snapshot, &main_queue(), &behind).status,
        Status::Blocked(WhyBlocked::MergeMethodUnclear { allowed }),
        "a configuration problem should surface before a pull request reaches the front"
    );
}

#[test]
fn a_verification_that_ran_out_of_time_says_so_rather_than_naming_a_check() {
    let snapshot = a_pull_request().done();
    let waiting = ran_out_of_time();

    let decision = decide(
        &snapshot,
        &main_queue(),
        &InQueue {
            ahead: 0,
            paused: false,
            verification: Some(&waiting),
        },
    );

    assert_eq!(
        decision.status.to_string(),
        "the checks did not finish in time",
        "the sentence used to be smuggled through the failed-check list, which read as \
         check \"the checks did not finish in time\" failed"
    );
}
