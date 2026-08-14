mod support;

use goat_merge_core::{CHECK_NAME, Check, Conclusion, Sha, already_verified};
use support::{a_pull_request, oclock, passing};

#[test]
fn a_quiet_base_lets_the_existing_checks_stand_in_for_a_candidate() {
    let snapshot = a_pull_request()
        .whose_base_last_moved_at(oclock(9, 0))
        .with_checks(vec![passing("test", "head-one", oclock(9, 30))])
        .done();

    assert!(
        already_verified(&snapshot),
        "the check ran after the base last moved, so it tested the tree that is about to land"
    );
}

#[test]
fn a_check_that_started_before_the_last_push_to_base_is_not_trusted() {
    let snapshot = a_pull_request()
        .whose_base_last_moved_at(oclock(10, 0))
        .with_checks(vec![passing("test", "head-one", oclock(9, 50))])
        .done();

    assert!(
        !already_verified(&snapshot),
        "a check that started before the base moved tested the base as it was, not as it is"
    );
}

#[test]
fn finishing_after_the_base_moved_is_not_enough() {
    let long_check = Check {
        name: "test".to_owned(),
        conclusion: Conclusion::Success,
        head: Sha::from("head-one"),
        started_at: Some(oclock(9, 50)),
    };
    let snapshot = a_pull_request()
        .whose_base_last_moved_at(oclock(10, 0))
        .with_checks(vec![long_check])
        .done();

    assert!(
        !already_verified(&snapshot),
        "a check running from 09:50 across a 10:00 push still checked out the pre-push base"
    );
}

#[test]
fn a_check_with_no_start_time_is_not_trusted() {
    let undated = Check {
        name: "test".to_owned(),
        conclusion: Conclusion::Success,
        head: Sha::from("head-one"),
        started_at: None,
    };
    let snapshot = a_pull_request().with_checks(vec![undated]).done();

    assert!(
        !already_verified(&snapshot),
        "without a start time there is no way to place the check against the base's history"
    );
}

#[test]
fn a_check_attached_to_an_older_head_is_not_trusted() {
    let snapshot = a_pull_request()
        .at_head("head-two")
        .with_checks(vec![passing("test", "head-one", oclock(9, 30))])
        .done();

    assert!(!already_verified(&snapshot));
}

#[test]
fn every_required_check_has_to_clear_the_base_move() {
    let snapshot = a_pull_request()
        .whose_base_last_moved_at(oclock(10, 0))
        .with_checks(vec![
            passing("lint", "head-one", oclock(10, 5)),
            passing("test", "head-one", oclock(9, 40)),
        ])
        .done();

    assert!(
        !already_verified(&snapshot),
        "one stale check spoils the whole verification"
    );
}

#[test]
fn a_repository_with_no_required_checks_has_nothing_left_to_verify() {
    let snapshot = a_pull_request().with_no_required_checks().done();

    assert!(already_verified(&snapshot));
}

#[test]
fn our_own_check_is_not_evidence_for_the_fast_path() {
    let ours = Check {
        name: CHECK_NAME.to_owned(),
        conclusion: Conclusion::Success,
        head: Sha::from("head-one"),
        started_at: None,
    };
    let snapshot = a_pull_request().with_checks(vec![ours]).done();

    assert!(
        already_verified(&snapshot),
        "our check is excluded, so what is left is a repository with no required checks"
    );
}
