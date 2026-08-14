use goat_merge::github::calls::{
    CheckRuns, CommitStatuses, GitReference, Protection, PullRequest, RepositorySettings, Review,
};
use serde_json::Value;

fn captured(name: &str) -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/from_github/");
    std::fs::read_to_string(format!("{path}{name}.json"))
        .expect("a captured answer from github.com")
}

#[test]
fn githubs_real_answers_still_fit_our_types() {
    let _: RepositorySettings = serde_json::from_str(&captured("repo")).expect("a repository");
    let pull: PullRequest = serde_json::from_str(&captured("pull")).expect("a pull request");
    let _: Vec<Review> = serde_json::from_str(&captured("reviews")).expect("reviews");
    let runs: CheckRuns = serde_json::from_str(&captured("checkruns")).expect("check runs");
    let _: CommitStatuses = serde_json::from_str(&captured("statuses")).expect("statuses");
    let _: GitReference = serde_json::from_str(&captured("ref")).expect("a ref");

    assert!(!pull.head.sha.is_empty());
    assert!(
        !pull.from_a_fork(),
        "this one came from a branch, not a fork"
    );
    assert!(runs.check_runs.iter().all(|run| !run.name.is_empty()));
}

#[test]
fn the_list_of_pull_requests_deserialises_even_though_it_says_less_than_one_pull_request() {
    let listed: Vec<PullRequest> =
        serde_json::from_str(&captured("pulls")).expect("the open pull requests");

    assert!(
        listed
            .iter()
            .any(|pull| pull.state == "open" && !pull.merged()),
        "the list form leaves the merged flag out entirely, so requiring it here would have made \
         every command that lists pull requests fail against the real GitHub"
    );
    assert!(
        listed.iter().any(PullRequest::merged),
        "and it still tells us which ones landed, through merged_at"
    );
}

#[test]
fn github_really_does_send_a_start_time_with_a_check_run() {
    let runs: CheckRuns = serde_json::from_str(&captured("checkruns")).expect("check runs");

    assert!(
        runs.check_runs.iter().all(|run| run.started_at.is_some()),
        "the fast path is only safe because a check says when it began. Without that we build a \
         candidate every time, which is slower but never wrong — so this is a warning, not a \
         disaster. It is still worth knowing if GitHub stops sending it."
    );
}

#[test]
fn a_ruleset_can_narrow_which_merge_methods_are_allowed() {
    let rules: Value = serde_json::from_str(&captured("rules")).expect("rules");
    let narrowing = rules
        .as_array()
        .into_iter()
        .flatten()
        .filter(|rule| rule.get("type").and_then(Value::as_str) == Some("pull_request"))
        .filter_map(|rule| rule.get("parameters")?.get("allowed_merge_methods"))
        .count();

    assert!(
        narrowing > 0,
        "a repository's settings are not the last word on merge methods; a ruleset can allow \
         fewer, and Protection has to carry that or we offer a method GitHub will refuse"
    );
}

#[test]
fn a_ruleset_gives_us_the_required_approvals() {
    let rules: Value = serde_json::from_str(&captured("rules")).expect("rules");
    let wanted = rules
        .as_array()
        .into_iter()
        .flatten()
        .filter(|rule| rule.get("type").and_then(Value::as_str) == Some("pull_request"))
        .filter_map(|rule| {
            rule.get("parameters")?
                .get("required_approving_review_count")?
                .as_u64()
        })
        .max();

    assert!(
        wanted.is_some(),
        "this is where readiness gets the approval count from"
    );
}

#[test]
fn a_repository_with_no_rules_and_no_protection_is_simply_unprotected() {
    let bare = Protection::default();

    assert!(!bare.declared_anywhere);
    assert!(bare.required_checks.is_empty());
    assert_eq!(bare.required_approvals, 0);
    assert_eq!(bare.only_these_merge_methods, None);
}
