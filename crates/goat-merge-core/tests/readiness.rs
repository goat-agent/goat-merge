mod support;

use goat_merge_core::{CHECK_NAME, Readiness, WhyBlocked, WhyWaiting, assess};
use support::{a_pull_request, failing, oclock, passing, pending};

#[test]
fn a_fork_pull_request_without_a_safe_workflow_is_blocked() {
    let snapshot = a_pull_request().sent_from_a_fork().done();

    assert_eq!(
        assess(&snapshot),
        Readiness::Blocked(WhyBlocked::ForkWithoutSafeWorkflow)
    );
}

#[test]
fn a_fork_pull_request_with_a_safe_workflow_is_assessed_like_any_other() {
    let snapshot = a_pull_request()
        .sent_from_a_fork()
        .whose_fork_workflow_is_declared_safe()
        .done();

    assert_eq!(assess(&snapshot), Readiness::Ready);
}

#[test]
fn a_fork_is_blocked_before_anything_else_is_reported() {
    let snapshot = a_pull_request()
        .sent_from_a_fork()
        .that_is_a_draft()
        .approved_by(0, 2)
        .done();

    assert_eq!(
        assess(&snapshot),
        Readiness::Blocked(WhyBlocked::ForkWithoutSafeWorkflow),
        "a contributor should learn the fork will never merge before they polish the draft"
    );
}

#[test]
fn readiness_ignores_our_own_merge_queue_check() {
    let snapshot = a_pull_request()
        .with_checks(vec![
            passing("test", "head-one", oclock(9, 30)),
            pending(CHECK_NAME, "head-one"),
        ])
        .done();

    assert_eq!(
        assess(&snapshot),
        Readiness::Ready,
        "our own check is required on the base branch, so counting it deadlocks the queue"
    );
}

#[test]
fn a_conflict_blocks_instead_of_waiting() {
    let snapshot = a_pull_request().that_conflicts().done();

    assert_eq!(assess(&snapshot), Readiness::Blocked(WhyBlocked::Conflict));
}

#[test]
fn an_unresolved_mergeability_waits_rather_than_guessing() {
    let snapshot = a_pull_request().whose_mergeability_is_unknown().done();

    assert_eq!(
        assess(&snapshot),
        Readiness::Waiting(WhyWaiting::MergeabilityUnknown)
    );
}

#[test]
fn a_draft_waits_even_with_every_approval() {
    let snapshot = a_pull_request().that_is_a_draft().approved_by(5, 2).done();

    assert_eq!(assess(&snapshot), Readiness::Waiting(WhyWaiting::Draft));
}

#[test]
fn a_missing_approval_says_how_many_are_missing() {
    let snapshot = a_pull_request().approved_by(1, 2).done();

    assert_eq!(
        assess(&snapshot),
        Readiness::Waiting(WhyWaiting::NeedsApproval { have: 1, want: 2 })
    );
}

#[test]
fn a_failed_required_check_blocks_and_names_the_check() {
    let snapshot = a_pull_request()
        .with_checks(vec![failing("test", "head-one")])
        .done();

    assert_eq!(
        assess(&snapshot),
        Readiness::Blocked(WhyBlocked::RequiredCheckFailed {
            checks: vec!["test".to_owned()]
        })
    );
}

#[test]
fn a_check_attached_to_an_older_head_does_not_count_as_passed() {
    let snapshot = a_pull_request()
        .at_head("head-two")
        .with_checks(vec![passing("test", "head-one", oclock(9, 30))])
        .done();

    assert_eq!(
        assess(&snapshot),
        Readiness::Waiting(WhyWaiting::RequiredChecksPending { done: 0, total: 1 }),
        "a check tested the commit it was attached to, not whatever was pushed afterwards"
    );
}

#[test]
fn a_pull_request_with_nothing_left_to_wait_for_is_ready() {
    assert_eq!(assess(&a_pull_request().done()), Readiness::Ready);
}
