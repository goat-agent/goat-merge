use std::time::Duration;

use goat_merge_core::{Config, ConfigError, Enqueue, MergeMethod};

#[test]
fn the_smallest_useful_config_is_a_version_and_a_branch() {
    let config = Config::parse("version: 1\nqueues:\n  - branch: main\n").expect("should parse");

    let queue = config.queue_for("main").expect("main should have a queue");
    assert_eq!(queue.enqueue, Enqueue::Manual);
    assert_eq!(queue.merge_method, None);
    assert_eq!(queue.check_timeout, Duration::from_secs(45 * 60));
}

#[test]
fn a_branch_with_no_queue_is_not_managed() {
    let config = Config::parse("version: 1\nqueues:\n  - branch: main\n").expect("should parse");

    assert!(config.queue_for("develop").is_none());
}

#[test]
fn a_queue_can_say_how_it_wants_to_merge() {
    let config = Config::parse(
        "version: 1\nqueues:\n  - branch: main\n    enqueue: automatic\n    merge_method: squash\n",
    )
    .expect("should parse");

    let queue = config.queue_for("main").expect("main should have a queue");
    assert_eq!(queue.enqueue, Enqueue::Automatic);
    assert_eq!(queue.merge_method, Some(MergeMethod::Squash));
}

#[test]
fn an_unknown_field_is_an_error() {
    let problem = Config::parse("version: 1\nqueues:\n  - branch: main\n    strategy: serial\n")
        .expect_err("a misspelt field must not be ignored");

    assert!(
        matches!(problem, ConfigError::Malformed(_)),
        "got {problem:?}"
    );
}

#[test]
fn a_future_version_is_refused_rather_than_guessed_at() {
    let problem =
        Config::parse("version: 2\nqueues:\n  - branch: main\n").expect_err("should refuse");

    assert!(
        matches!(problem, ConfigError::UnknownVersion { found: 2 }),
        "got {problem:?}"
    );
}

#[test]
fn one_branch_gets_one_queue() {
    let problem = Config::parse("version: 1\nqueues:\n  - branch: main\n  - branch: main\n")
        .expect_err("should refuse");

    assert!(
        matches!(problem, ConfigError::BranchTwice { .. }),
        "got {problem:?}"
    );
}

#[test]
fn a_timeout_is_written_as_a_number_and_a_unit() {
    let config = Config::parse("version: 1\nqueues:\n  - branch: main\n    check_timeout: 90s\n")
        .expect("should parse");

    assert_eq!(
        config
            .queue_for("main")
            .expect("main should have a queue")
            .check_timeout,
        Duration::from_secs(90)
    );
}

#[test]
fn a_timeout_without_a_unit_is_refused() {
    let problem = Config::parse("version: 1\nqueues:\n  - branch: main\n    check_timeout: 45\n")
        .expect_err("a bare number is ambiguous");

    assert!(
        matches!(problem, ConfigError::Malformed(_)),
        "got {problem:?}"
    );
}

#[test]
fn a_config_that_lists_nothing_manages_nothing() {
    let config = Config::parse("version: 1\n").expect("should parse");

    assert!(config.queues.is_empty());
    assert_eq!(config.retry.ci_failures, 0);
}
