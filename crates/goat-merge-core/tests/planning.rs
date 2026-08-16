mod support;

use goat_merge_core::{
    Aboard, Conclusion, Enqueue, InQueue, MergeMethod, Next, NotQueued, Queue, Sha, Status,
    Verification, WhyBlocked, WhyFailed, WhyWaiting, after_a_batch, decide, how_many_to_verify,
};
use std::sync::LazyLock;

use support::{a_pull_request, oclock, passing};

fn main_queue() -> Queue {
    Queue::following_github("main")
}

static ALONE: LazyLock<Vec<Aboard>> = LazyLock::new(|| carrying(&[(4835, "head-one")]));
static SOMEBODY_ELSE: LazyLock<Vec<Aboard>> =
    LazyLock::new(|| carrying(&[(11, "head-of-another")]));

fn carrying(who: &[(u64, &str)]) -> Vec<Aboard> {
    who.iter()
        .map(|(number, head)| Aboard {
            number: *number,
            head: Sha::from(*head),
        })
        .collect()
}

fn alone() -> &'static [Aboard] {
    &ALONE
}

fn at_the_front(aboard: &[Aboard]) -> InQueue<'_> {
    InQueue {
        ahead: 0,
        aboard,
        paused: false,
        verification: None,
    }
}

fn verified(conclusion: Conclusion) -> Verification {
    Verification {
        base: Sha::from("base-one"),
        aboard: ALONE.clone(),
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

    let decision = decide(&snapshot, &main_queue(), &at_the_front(alone()));

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
        decide(&snapshot, &queue, &at_the_front(alone())).status,
        Status::Merging
    );
}

#[test]
fn a_paused_queue_holds_everything_that_is_ready() {
    let snapshot = a_pull_request().done();
    let entry = InQueue {
        paused: true,
        ..at_the_front(alone())
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
        ..at_the_front(alone())
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
        ..at_the_front(&SOMEBODY_ELSE)
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

    let decision = decide(&snapshot, &main_queue(), &at_the_front(alone()));

    assert_eq!(
        decision.status,
        Status::Preparing {
            alongside: Vec::new()
        }
    );
    assert_eq!(
        decision.next,
        Next::BuildCandidate {
            onto: Sha::from("base-one"),
            aboard: ALONE.clone(),
        }
    );
}

#[test]
fn a_quiet_queue_merges_without_building_anything() {
    let snapshot = a_pull_request().done();

    let decision = decide(&snapshot, &main_queue(), &at_the_front(alone()));

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
        ..at_the_front(alone())
    };

    let decision = decide(&snapshot, &main_queue(), &entry);

    assert_eq!(decision.next, Next::DiscardVerification);
    assert_eq!(
        decision.status,
        Status::Preparing {
            alongside: Vec::new()
        }
    );
}

#[test]
fn a_pull_request_whose_head_moved_is_revalidated_not_merged() {
    let snapshot = a_pull_request()
        .at_head("head-two")
        .whose_base_last_moved_at(oclock(11, 0))
        .with_checks(vec![passing("test", "head-two", oclock(11, 30))])
        .done();
    let now_riding = carrying(&[(4835, "head-two")]);
    let stale = verified(Conclusion::Success);
    let entry = InQueue {
        verification: Some(&stale),
        ..at_the_front(&now_riding)
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
        ..at_the_front(alone())
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
        ..at_the_front(alone())
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
        ..at_the_front(alone())
    };

    let decision = decide(&snapshot, &main_queue(), &entry);

    assert_eq!(
        decision.status,
        Status::Validating {
            alongside: Vec::new()
        }
    );
    assert_eq!(decision.next, Next::Nothing);
}

#[test]
fn a_repository_allowing_one_merge_method_needs_no_configuration() {
    let snapshot = a_pull_request().allowing(vec![MergeMethod::Rebase]).done();

    assert_eq!(
        decide(&snapshot, &main_queue(), &at_the_front(alone())).next,
        Next::Merge {
            method: MergeMethod::Rebase
        }
    );
}

#[test]
fn a_repository_allowing_several_asks_which_one() {
    let allowed = vec![MergeMethod::Merge, MergeMethod::Squash];
    let snapshot = a_pull_request().allowing(allowed.clone()).done();

    let decision = decide(&snapshot, &main_queue(), &at_the_front(alone()));

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
        decide(&snapshot, &queue, &at_the_front(alone())).status,
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
        ..at_the_front(alone())
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
            verification: Some(&waiting),
            ..at_the_front(alone())
        },
    );

    assert_eq!(
        decision.status.to_string(),
        "the checks did not finish in time",
        "the sentence used to be smuggled through the failed-check list, which read as \
         check \"the checks did not finish in time\" failed"
    );
}

#[test]
fn everyone_on_a_passing_candidate_is_told_to_merge() {
    let together = carrying(&[(20, "head-one"), (21, "head-two"), (22, "head-three")]);
    let passed = Verification {
        aboard: together.clone(),
        ..verified(Conclusion::Success)
    };

    for (place, one) in together.iter().enumerate() {
        let snapshot = a_pull_request()
            .numbered(one.number)
            .at_head(one.head.as_str())
            .whose_base_last_moved_at(oclock(11, 0))
            .with_checks(vec![passing("test", one.head.as_str(), oclock(9, 30))])
            .done();
        let entry = InQueue {
            ahead: place,
            verification: Some(&passed),
            ..at_the_front(&together)
        };

        let decision = decide(&snapshot, &main_queue(), &entry);

        assert_eq!(decision.status, Status::Merging, "#{}", one.number);
        assert_eq!(
            decision.next,
            Next::Merge {
                method: MergeMethod::Squash
            },
            "#{} rode the candidate that passed, so it merges with the rest",
            one.number
        );
    }
}

#[test]
fn a_batch_that_fails_blames_nobody_and_is_tried_again_with_fewer() {
    let together = carrying(&[(20, "head-one"), (21, "head-two")]);
    let snapshot = a_pull_request()
        .numbered(20)
        .whose_base_last_moved_at(oclock(11, 0))
        .with_checks(vec![passing("test", "head-one", oclock(9, 30))])
        .done();
    let failed = Verification {
        aboard: together.clone(),
        failed_checks: vec!["integration tests".to_owned()],
        ..verified(Conclusion::Failure)
    };
    let entry = InQueue {
        verification: Some(&failed),
        ..at_the_front(&together)
    };

    let decision = decide(&snapshot, &main_queue(), &entry);

    assert_eq!(
        decision.next,
        Next::DiscardVerification,
        "two pull requests failed together, so neither of them is proven to be the one at fault"
    );
    assert!(
        !decision.status.is_settled(),
        "throwing somebody out of the queue on a shared failure punishes the innocent"
    );
}

#[test]
fn the_last_one_left_on_a_failing_candidate_is_the_one_at_fault() {
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
        ..at_the_front(alone())
    };

    assert_eq!(
        decide(&snapshot, &main_queue(), &entry).status,
        Status::Failed(WhyFailed::ChecksFailed {
            checks: vec!["integration tests".to_owned()]
        })
    );
}

#[test]
fn a_candidate_that_gained_or_lost_a_rider_is_no_evidence_about_the_ones_left() {
    let verified_two = carrying(&[(20, "head-one"), (21, "head-two")]);
    let snapshot = a_pull_request()
        .numbered(20)
        .whose_base_last_moved_at(oclock(11, 0))
        .with_checks(vec![passing("test", "head-one", oclock(9, 30))])
        .done();
    let passed = Verification {
        aboard: verified_two,
        ..verified(Conclusion::Success)
    };
    let entry = InQueue {
        verification: Some(&passed),
        ..at_the_front(alone())
    };

    assert_eq!(
        decide(&snapshot, &main_queue(), &entry).next,
        Next::DiscardVerification,
        "the tree that passed contained #21's commits, so it says nothing about #20 on its own"
    );
}

#[test]
fn a_batch_never_takes_the_fast_path() {
    let together = carrying(&[(20, "head-one"), (21, "head-two")]);
    let snapshot = a_pull_request().numbered(20).done();

    let decision = decide(&snapshot, &main_queue(), &at_the_front(&together));

    assert_eq!(
        decision.next,
        Next::BuildCandidate {
            onto: Sha::from("base-one"),
            aboard: together,
        },
        "each pull request's own checks say nothing about the two of them merged together"
    );
}

#[test]
fn somebody_riding_with_others_is_told_who_they_are_riding_with() {
    let together = carrying(&[(20, "head-one"), (21, "head-two"), (22, "head-three")]);
    let snapshot = a_pull_request()
        .numbered(21)
        .at_head("head-two")
        .with_checks(vec![passing("test", "head-two", oclock(9, 30))])
        .done();
    let running = Verification {
        aboard: together.clone(),
        ..verified(Conclusion::Pending)
    };
    let entry = InQueue {
        verification: Some(&running),
        ..at_the_front(&together)
    };

    assert_eq!(
        decide(&snapshot, &main_queue(), &entry).status.to_string(),
        "checking the candidate, which also carries #20 and #22",
        "somebody whose own checks are green needs to know whose work they are waiting on"
    );
}

#[test]
fn a_queue_verifying_one_at_a_time_says_nothing_about_company() {
    let snapshot = a_pull_request().done();
    let running = verified(Conclusion::Pending);
    let entry = InQueue {
        verification: Some(&running),
        ..at_the_front(alone())
    };

    assert_eq!(
        decide(&snapshot, &main_queue(), &entry).status.to_string(),
        "checking the candidate"
    );
}

#[test]
fn a_batch_grows_by_one_when_it_passes_and_halves_when_it_fails() {
    let rules = Queue {
        batch_size: 8,
        ..main_queue()
    };

    assert_eq!(after_a_batch(1, &rules, true), 2);
    assert_eq!(after_a_batch(4, &rules, true), 5);
    assert_eq!(after_a_batch(8, &rules, true), 8, "never past the ceiling");
    assert_eq!(after_a_batch(8, &rules, false), 4);
    assert_eq!(after_a_batch(2, &rules, false), 1);
    assert_eq!(
        after_a_batch(1, &rules, false),
        1,
        "one that failed on its own has nowhere smaller to go"
    );
}

#[test]
fn a_queue_never_verifies_more_than_it_is_allowed_or_more_than_it_has() {
    let rules = Queue {
        batch_size: 3,
        ..main_queue()
    };

    assert_eq!(how_many_to_verify(&rules, 5, 9), 3, "the ceiling holds");
    assert_eq!(how_many_to_verify(&rules, 2, 9), 2);
    assert_eq!(how_many_to_verify(&rules, 3, 1), 1, "only one is ready");
    assert_eq!(
        how_many_to_verify(&rules, 0, 9),
        1,
        "a queue that has never verified anything still verifies one"
    );
}

#[test]
fn turning_batching_off_leaves_one_at_a_time() {
    let rules = Queue {
        batch_size: 1,
        ..main_queue()
    };

    assert_eq!(how_many_to_verify(&rules, 9, 9), 1);
    assert_eq!(after_a_batch(1, &rules, true), 1);
}
