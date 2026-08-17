mod support;

use std::sync::Arc;

use goat_merge::engine::{Engine, TEND, subject};
use goat_merge::github::{AppAuth, Github};
use goat_merge::settings::Settings;
use support::fake_github::{
    INSTALLATION, NAME, OWNER, World, a_private_key, failing, listening, passing, running,
};
use support::{a_seal, a_store_sealed_with, an_id_of_its_own};
use time::{Duration, OffsetDateTime};

struct Standing {
    engine: Engine,
    world: Arc<World>,
    queue_id: i64,
    repository_id: i64,
}

async fn a_repository_with(world: Arc<World>) -> Option<Standing> {
    let Some(key) = a_private_key() else {
        eprintln!("SKIPPED: these tests need openssl to make a throwaway RSA key");
        return None;
    };
    let store = a_store_sealed_with(a_seal()).await?;
    let api = listening(Arc::clone(&world)).await;

    let github = Github::new(&api).expect("a client");
    github.adopt(AppAuth::holding(1, key).expect("a usable key"));

    let settings = Settings {
        public_url: "https://merge.example.com".to_owned(),
        listen: "127.0.0.1:0".parse().expect("an address"),
        database_url: String::new(),
        master_key: [0; 32],
        github_api: api,
        github_web: "https://github.com".to_owned(),
    };

    let id = an_id_of_its_own();
    store
        .remember_installation(INSTALLATION, OWNER)
        .await
        .expect("an installation");
    let repository = store
        .remember_repository(id, INSTALLATION, OWNER, &format!("{NAME}-{id}"))
        .await
        .expect("a repository");
    store
        .set_repository_active(repository.id, true)
        .await
        .expect("an active repository");
    let queue = store
        .queue_for(repository.id, "main")
        .await
        .expect("a queue");
    store
        .enter(queue.id, 123, "frank", "chore: appease the linter gods")
        .await
        .expect("an entry");

    let engine = Engine::new(store, github, Arc::new(settings));
    Some(Standing {
        engine,
        world,
        queue_id: queue.id,
        repository_id: repository.id,
    })
}

#[tokio::test]
async fn a_quiet_base_merges_without_building_a_candidate() {
    let world = World::holding_one_ready_pull_request();
    world.checks_on("head-one", vec![passing("test", "2099-01-01T00:00:00Z")]);
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("a tend");

    assert_eq!(
        standing.world.merged_pull_requests(),
        vec![(123, "squash".to_owned(), "head-one".to_owned())],
        "the existing checks already tested this tree, so nothing needed building"
    );
    assert!(
        standing.world.draft_pull_requests().is_empty(),
        "no candidate should have been opened"
    );
}

#[tokio::test]
async fn a_base_that_moved_after_the_checks_makes_a_candidate_instead_of_merging() {
    let world = World::holding_one_ready_pull_request();
    world.checks_on("head-one", vec![passing("test", "2000-01-01T00:00:00Z")]);
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("a tend");

    assert!(
        standing.world.merged_pull_requests().is_empty(),
        "a check that ran before the base moved is not evidence about what will land"
    );
    assert_eq!(
        standing.world.draft_pull_requests().len(),
        1,
        "a candidate should have been opened to run the checks again"
    );
}

#[tokio::test]
async fn a_candidate_is_judged_without_waiting_for_our_own_required_check() {
    let world = World::holding_one_ready_pull_request();
    *world.required.lock().expect("required") = vec!["test".to_owned(), "Merge Queue".to_owned()];
    world.checks_on("head-one", vec![passing("test", "2000-01-01T00:00:00Z")]);
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the first tend");
    standing.world.checks_on(
        "candidate-of-head-one",
        vec![passing("test", "2099-01-01T00:00:00Z")],
    );
    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the second tend");

    assert_eq!(
        standing.world.merged_pull_requests().len(),
        1,
        "every repository worth queueing makes Merge Queue a required check, and nothing ever \
         publishes it against a candidate, so counting it there leaves the candidate pending \
         until it times out"
    );
}

#[tokio::test]
async fn a_candidate_that_passes_merges_the_original_pull_request() {
    let world = World::holding_one_ready_pull_request();
    world.checks_on("head-one", vec![passing("test", "2000-01-01T00:00:00Z")]);
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the first tend");
    standing.world.checks_on(
        "candidate-of-head-one",
        vec![passing("test", "2099-01-01T00:00:00Z")],
    );
    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the second tend");

    assert_eq!(
        standing.world.merged_pull_requests(),
        vec![(123, "squash".to_owned(), "head-one".to_owned())],
        "the candidate is only a way to run the checks; the original pull request is what merges"
    );
}

#[tokio::test]
async fn a_candidate_still_running_does_not_merge_anything() {
    let world = World::holding_one_ready_pull_request();
    world.checks_on("head-one", vec![passing("test", "2000-01-01T00:00:00Z")]);
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the first tend");
    standing.world.checks_on(
        "candidate-of-head-one",
        vec![running("test", "2099-01-01T00:00:00Z")],
    );
    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the second tend");

    assert!(standing.world.merged_pull_requests().is_empty());
}

#[tokio::test]
async fn a_candidate_that_fails_takes_the_pull_request_out_of_the_queue() {
    let world = World::holding_one_ready_pull_request();
    world.checks_on("head-one", vec![passing("test", "2000-01-01T00:00:00Z")]);
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the first tend");
    standing.world.checks_on(
        "candidate-of-head-one",
        vec![failing("test", "2099-01-01T00:00:00Z")],
    );
    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the second tend");

    assert!(standing.world.merged_pull_requests().is_empty());
    assert!(
        standing
            .world
            .labels_removed
            .lock()
            .expect("labels")
            .iter()
            .any(|(number, label)| *number == 123 && label == "merge-queue"),
        "a pull request that failed should let go of the label so it can be added again"
    );
    assert!(
        !standing.world.comments.lock().expect("comments").is_empty(),
        "somebody has to be told why it left the queue"
    );
}

#[tokio::test]
async fn a_base_that_moves_during_verification_throws_the_result_away() {
    let world = World::holding_one_ready_pull_request();
    world.checks_on("head-one", vec![passing("test", "2000-01-01T00:00:00Z")]);
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the first tend");
    standing.world.checks_on(
        "candidate-of-head-one",
        vec![passing("test", "2099-01-01T00:00:00Z")],
    );
    standing.world.move_the_base_to("base-two");
    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the second tend");

    assert!(
        standing.world.merged_pull_requests().is_empty(),
        "the verification was about a base that no longer exists, so it must not be used"
    );
    assert!(
        !standing
            .world
            .deleted_branches
            .lock()
            .expect("deleted")
            .is_empty(),
        "the candidate branch should have been tidied away"
    );
}

#[tokio::test]
async fn a_fork_pull_request_without_a_safe_workflow_never_merges() {
    let world = World::holding_one_ready_pull_request();
    world.checks_on("head-one", vec![passing("test", "2099-01-01T00:00:00Z")]);
    world
        .pulls
        .lock()
        .expect("pulls")
        .entry(123)
        .and_modify(|pull| pull.from_fork = true);
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("a tend");

    assert!(
        standing.world.merged_pull_requests().is_empty(),
        "fork code must not be run in the base repository's trusted environment"
    );
    assert!(
        standing
            .world
            .check_conclusions()
            .iter()
            .any(|conclusion| conclusion.as_deref() == Some("failure"))
    );
}

#[tokio::test]
async fn our_check_only_turns_green_in_the_moment_before_the_merge() {
    let world = World::holding_one_ready_pull_request();
    world.checks_on("head-one", vec![passing("test", "2099-01-01T00:00:00Z")]);
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("a tend");

    let published = standing.world.published.lock().expect("published").clone();
    let green = published
        .iter()
        .filter(|(_, _, conclusion, _)| conclusion.as_deref() == Some("success"))
        .count();
    assert_eq!(green, 1, "the gate opens once, for the verified state");
    assert_eq!(
        published.last().map(|(_, title, _, _)| title.as_str()),
        Some("Merging"),
        "the last thing published before the merge is the state that allows it"
    );
}

#[tokio::test]
async fn an_installation_nobody_started_here_is_ignored() {
    let world = World::holding_one_ready_pull_request();
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };
    let a_stranger = an_id_of_its_own();

    standing
        .engine
        .react_to(
            "installation",
            &serde_json::json!({
                "action": "created",
                "installation": {
                    "id": a_stranger,
                    "account": { "login": "somebody-else" },
                },
                "repositories": [{ "id": 5_000_001, "full_name": "somebody-else/theirs" }],
            }),
        )
        .await
        .expect("a webhook is answered even when it is ignored");

    assert!(
        !standing
            .engine
            .store
            .installation_started_here(a_stranger)
            .await
            .expect("a lookup"),
        "the App is public, so anybody on GitHub can install it. Only an installation somebody \
         began on this server belongs to this server"
    );
}

#[tokio::test]
async fn moving_this_server_takes_the_app_webhook_with_it() {
    let world = World::holding_one_ready_pull_request();
    *world.webhook_url.lock().expect("webhook") =
        "https://where-it-used-to-be.example.com/api/github/webhook".to_owned();
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };

    standing.engine.catch_the_app_up_with_our_address().await;

    assert_eq!(
        *standing.world.webhook_url.lock().expect("webhook"),
        "https://merge.example.com/api/github/webhook",
        "a server that has moved must not need a whole new App to keep hearing from GitHub"
    );
}

#[tokio::test]
async fn an_app_that_still_points_at_us_is_left_alone() {
    let world = World::holding_one_ready_pull_request();
    *world.webhook_url.lock().expect("webhook") =
        "https://merge.example.com/api/github/webhook".to_owned();
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };

    standing.engine.catch_the_app_up_with_our_address().await;

    assert_eq!(
        *standing.world.webhook_url.lock().expect("webhook"),
        "https://merge.example.com/api/github/webhook"
    );
}

#[tokio::test]
async fn the_check_links_to_the_pull_request_it_is_about() {
    let world = World::holding_one_ready_pull_request();
    world.checks_on("head-one", vec![passing("test", "2099-01-01T00:00:00Z")]);
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("a tend");

    let pointed = standing.world.where_the_check_pointed();
    assert!(!pointed.is_empty(), "our check should have been published");
    assert!(
        pointed.iter().all(|url| url.ends_with("?pull=123")),
        "somebody following the check from their own pull request lands on that pull request, \
         not in the middle of a list: {pointed:?}"
    );
}

#[tokio::test]
async fn a_pull_request_waiting_on_approvals_merges_nothing() {
    let world = World::holding_one_ready_pull_request();
    world.checks_on("head-one", vec![passing("test", "2099-01-01T00:00:00Z")]);
    *world.approvals.lock().expect("approvals") = 1;
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("a tend");

    assert!(standing.world.merged_pull_requests().is_empty());
    assert!(
        standing
            .world
            .what_the_check_said()
            .iter()
            .any(|title| title == "Waiting")
    );
}

#[tokio::test]
async fn a_repository_allowing_two_merge_methods_refuses_to_guess() {
    let world = World::holding_one_ready_pull_request();
    world.checks_on("head-one", vec![passing("test", "2099-01-01T00:00:00Z")]);
    *world.allow.lock().expect("allow") = vec!["merge".to_owned(), "squash".to_owned()];
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("a tend");

    assert!(
        standing.world.merged_pull_requests().is_empty(),
        "picking a merge method for someone rewrites their history without asking"
    );
    assert!(
        standing
            .world
            .what_the_check_said()
            .iter()
            .any(|title| title == "Blocked")
    );
}

#[tokio::test]
async fn a_configuration_file_decides_the_merge_method() {
    let world = World::holding_one_ready_pull_request();
    world.checks_on("head-one", vec![passing("test", "2099-01-01T00:00:00Z")]);
    *world.allow.lock().expect("allow") = vec!["merge".to_owned(), "squash".to_owned()];
    world.files.lock().expect("files").insert(
        ".github/merge-queue.yml".to_owned(),
        "version: 1\nqueues:\n  - branch: main\n    merge_method: merge\n".to_owned(),
    );
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("a tend");

    assert_eq!(
        standing.world.merged_pull_requests(),
        vec![(123, "merge".to_owned(), "head-one".to_owned())]
    );
}

#[tokio::test]
async fn tending_twice_merges_once() {
    let world = World::holding_one_ready_pull_request();
    world.checks_on("head-one", vec![passing("test", "2099-01-01T00:00:00Z")]);
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the first tend");
    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the second tend");

    assert_eq!(
        standing.world.merged_pull_requests().len(),
        1,
        "webhooks arrive more than once, and a merge must not"
    );
}

#[tokio::test]
async fn a_pull_request_merged_by_hand_leaves_the_queue_at_the_next_look() {
    let world = World::holding_one_ready_pull_request();
    world.checks_on("head-one", vec![passing("test", "2000-01-01T00:00:00Z")]);
    world
        .pulls
        .lock()
        .expect("pulls")
        .entry(123)
        .and_modify(|pull| pull.merged = true);
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("a tend");

    assert!(
        standing.world.merged_pull_requests().is_empty(),
        "it is already merged, so there is nothing left to do to it"
    );
    assert!(
        standing.world.draft_pull_requests().is_empty(),
        "and nothing left to verify"
    );
}

#[tokio::test]
async fn a_label_taken_off_while_we_were_away_takes_the_pull_request_out() {
    let world = World::holding_one_ready_pull_request();
    world.checks_on("head-one", vec![passing("test", "2099-01-01T00:00:00Z")]);
    world
        .pulls
        .lock()
        .expect("pulls")
        .entry(123)
        .and_modify(|pull| pull.labels.clear());
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("a tend");

    assert!(
        standing.world.merged_pull_requests().is_empty(),
        "the webhook for the removed label may never have arrived, and the queue has to notice"
    );
}

#[tokio::test]
async fn a_pull_request_closed_without_merging_leaves_the_queue() {
    let world = World::holding_one_ready_pull_request();
    world.checks_on("head-one", vec![passing("test", "2000-01-01T00:00:00Z")]);
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the first tend");
    assert_eq!(standing.world.draft_pull_requests().len(), 1);

    standing
        .world
        .pulls
        .lock()
        .expect("pulls")
        .entry(123)
        .and_modify(|pull| pull.state = "closed".to_owned());
    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the second tend");

    assert!(
        !standing
            .world
            .deleted_branches
            .lock()
            .expect("deleted")
            .is_empty(),
        "the candidate it left behind has to be cleaned up"
    );
}

#[tokio::test]
async fn a_settled_status_is_not_republished_on_every_look() {
    let world = World::holding_one_ready_pull_request();
    world.checks_on("head-one", vec![passing("test", "2099-01-01T00:00:00Z")]);
    *world.approvals.lock().expect("approvals") = 1;
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the first tend");
    let after_one = standing.world.what_the_check_said().len();
    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the second tend");
    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the third tend");

    assert_eq!(
        standing.world.what_the_check_said().len(),
        after_one,
        "a queue that is quietly waiting must not spend a GitHub write on every sweep"
    );
}

#[tokio::test]
async fn a_repository_github_will_not_show_us_any_more_is_let_go_of_instead_of_retried_forever() {
    let world = World::holding_one_ready_pull_request();
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };
    *world.gone.lock().expect("gone") = true;

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("a tend that gives up quietly rather than failing");

    let queues = standing
        .engine
        .store
        .every_queue()
        .await
        .expect("the queues worth tending");
    assert!(
        !queues.iter().any(|queue| queue.id == standing.queue_id),
        "an uninstalled or deleted repository must drop out of the sweep, or every pass spends a \
         request finding out it is still gone"
    );

    let repository = standing
        .engine
        .store
        .repository_by_id(standing.repository_id)
        .await
        .expect("the repository")
        .expect("a repository row");
    assert!(
        repository.active,
        "losing sight of a repository is not the same as being told to switch its queue off, and \
         reinstalling must not leave somebody wondering why nothing merges"
    );
    assert!(
        !repository.we_can_still_see_it(),
        "and every screen that reports this repository as healthy has to be able to tell"
    );
}

#[tokio::test]
async fn the_label_we_put_on_ourselves_does_not_make_us_the_one_who_asked() {
    let world = World::holding_one_ready_pull_request();
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };
    let repository = standing
        .engine
        .store
        .repository_by_id(standing.repository_id)
        .await
        .expect("the repository")
        .expect("a repository row");

    standing
        .engine
        .react_to(
            "pull_request",
            &serde_json::json!({
                "action": "labeled",
                "label": { "name": "merge-queue" },
                "sender": { "login": "merge-queue[bot]" },
                "repository": {
                    "id": repository.id,
                    "full_name": repository.full_name(),
                },
                "installation": { "id": repository.installation_id },
                "pull_request": {
                    "number": 123,
                    "title": "chore: appease the linter gods",
                    "base": { "ref": "main" },
                },
            }),
        )
        .await
        .expect("a reaction to the label we added ourselves");

    let entry = standing
        .engine
        .store
        .entry(standing.queue_id, 123)
        .await
        .expect("the entry")
        .expect("an entry row");
    assert_eq!(
        entry.requested_by, "frank",
        "asking through the CLI adds the label, and the webhook for that label must not rewrite \
         history so that the App appears to have asked for its own merge"
    );
}

#[tokio::test]
async fn retrying_puts_the_label_back_so_the_next_look_does_not_cancel_it() {
    let world = World::holding_one_ready_pull_request();
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };
    world
        .pulls
        .lock()
        .expect("pulls")
        .entry(123)
        .and_modify(|pull| pull.labels.clear());
    let entry = standing
        .engine
        .store
        .entry(standing.queue_id, 123)
        .await
        .expect("the entry")
        .expect("an entry row");
    standing
        .engine
        .store
        .settle(entry.id, "Failed", "a check failed", None, None)
        .await
        .expect("a settled entry");

    let repository = standing
        .engine
        .store
        .repository_by_id(standing.repository_id)
        .await
        .expect("the repository")
        .expect("a repository row");
    standing
        .engine
        .put_it_in_the_queue(&repository, 123, "frank", "retried")
        .await
        .expect("a retry");

    assert!(
        standing
            .world
            .pulls
            .lock()
            .expect("pulls")
            .get(&123)
            .is_some_and(|pull| pull.labels.iter().any(|label| label == "merge-queue")),
        "giving up strips the label, so a retry that does not put it back is cancelled by the \
         very next tend — which is the one thing retry exists to avoid"
    );
}

#[tokio::test]
async fn taking_a_pull_request_out_settles_it_here_rather_than_waiting_for_a_webhook() {
    let world = World::holding_one_ready_pull_request();
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };
    let repository = standing
        .engine
        .store
        .repository_by_id(standing.repository_id)
        .await
        .expect("the repository")
        .expect("a repository row");

    let was_there = standing
        .engine
        .take_it_out_of_the_queue(&repository, 123, "frank")
        .await
        .expect("a dequeue");

    let entry = standing
        .engine
        .store
        .entry(standing.queue_id, 123)
        .await
        .expect("the entry")
        .expect("an entry row");
    assert!(was_there);
    assert_eq!(
        entry.status, "Cancelled",
        "answering ok while leaving the entry running means the console shows it queued forever \
         whenever the webhook is slow, lost, or the label was never on it"
    );
    assert!(entry.settled_at.is_some());
}

#[tokio::test]
async fn taking_out_a_pull_request_that_was_never_in_says_so() {
    let world = World::holding_one_ready_pull_request();
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };
    let repository = standing
        .engine
        .store
        .repository_by_id(standing.repository_id)
        .await
        .expect("the repository")
        .expect("a repository row");

    let was_there = standing
        .engine
        .take_it_out_of_the_queue(&repository, 999, "frank")
        .await
        .expect("a dequeue");

    assert!(
        !was_there,
        "removing a label that is not there looks exactly like removing one that is, so the \
         caller has to be told nothing happened"
    );
}

#[tokio::test]
async fn a_pull_request_that_leaves_the_queue_does_not_leave_our_check_running() {
    let world = World::holding_one_ready_pull_request();
    world.checks_on("head-one", vec![passing("test", "2000-01-01T00:00:00Z")]);
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };
    world
        .pulls
        .lock()
        .expect("pulls")
        .entry(123)
        .and_modify(|pull| pull.labels.clear());

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("a tend");

    let published = standing.world.published.lock().expect("published").clone();
    let last = published
        .last()
        .expect("our check should have been settled");
    assert_eq!(
        last.2.as_deref(),
        Some("cancelled"),
        "Merge Queue is a required check, so leaving it in progress after the pull request has \
         left the queue blocks even a merge by hand, for ever: {published:?}"
    );
}

async fn a_queue_of(world: Arc<World>, more: &[(i32, &str)], at_once: usize) -> Option<Standing> {
    for (number, head) in more {
        world.also_holding(*number, head);
    }
    let standing = a_repository_with(Arc::clone(&world)).await?;
    for (number, _) in more {
        standing
            .engine
            .store
            .enter(
                standing.queue_id,
                *number,
                "frank",
                &format!("chore: something else ({number})"),
            )
            .await
            .expect("another entry");
    }
    standing
        .engine
        .store
        .verify_this_many_next_time(standing.queue_id, at_once, "the test asked for it")
        .await
        .expect("a batch size");
    Some(standing)
}

fn stale(name: &str) -> support::fake_github::Check {
    passing(name, "2000-01-01T00:00:00Z")
}

fn merged_numbers(world: &World) -> Vec<i32> {
    let mut numbers: Vec<i32> = world
        .merged_pull_requests()
        .iter()
        .map(|(number, _, _)| *number)
        .collect();
    numbers.sort_unstable();
    numbers
}

#[tokio::test]
async fn three_ready_pull_requests_are_verified_on_one_candidate() {
    let world = World::holding_one_ready_pull_request();
    for head in ["head-one", "head-two", "head-three"] {
        world.checks_on(head, vec![stale("test")]);
    }
    let Some(standing) = a_queue_of(
        Arc::clone(&world),
        &[(124, "head-two"), (125, "head-three")],
        3,
    )
    .await
    else {
        return;
    };

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the first tend");

    assert_eq!(
        standing.world.candidate_branches().len(),
        1,
        "three pull requests that can be tested together cost one candidate, not three"
    );
    assert_eq!(standing.world.draft_pull_requests().len(), 1);

    standing.world.checks_on(
        "candidate-of-head-three",
        vec![passing("test", "2099-01-01T00:00:00Z")],
    );
    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the second tend");

    assert_eq!(
        merged_numbers(&standing.world),
        vec![123, 124, 125],
        "one passing candidate merges everything that was on it"
    );
}

#[tokio::test]
async fn a_queue_that_has_never_verified_anything_still_starts_with_one() {
    let world = World::holding_one_ready_pull_request();
    for head in ["head-one", "head-two", "head-three"] {
        world.checks_on(head, vec![stale("test")]);
    }
    let Some(standing) = a_queue_of(
        Arc::clone(&world),
        &[(124, "head-two"), (125, "head-three")],
        1,
    )
    .await
    else {
        return;
    };

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("a tend");

    assert_eq!(
        standing.world.candidate_branches(),
        vec!["merge-queue/candidate-123-base-on".to_owned()],
        "a queue proves itself one pull request at a time before it risks a batch"
    );
}

#[tokio::test]
async fn a_batch_that_fails_takes_nobody_out_and_tries_again_with_fewer() {
    let world = World::holding_one_ready_pull_request();
    for head in ["head-one", "head-two"] {
        world.checks_on(head, vec![stale("test")]);
    }
    let Some(standing) = a_queue_of(Arc::clone(&world), &[(124, "head-two")], 2).await else {
        return;
    };

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the first tend");
    standing.world.checks_on(
        "candidate-of-head-two",
        vec![failing("test", "2099-01-01T00:00:00Z")],
    );
    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the second tend");

    for number in [123, 124] {
        let entry = standing
            .engine
            .store
            .entry(standing.queue_id, number)
            .await
            .expect("the entry")
            .expect("an entry");
        assert_eq!(
            entry.settled_at, None,
            "#{number} failed alongside somebody else, so nothing yet says it is the one at fault"
        );
    }
    let queue = standing
        .engine
        .store
        .queue_by_id(standing.queue_id)
        .await
        .expect("the queue")
        .expect("a queue");
    assert_eq!(
        queue.verify_at_once, 1,
        "a failure halves how many are verified together, which is what finds the culprit"
    );
}

#[tokio::test]
async fn narrowing_down_stops_at_the_one_that_actually_broke_it() {
    let world = World::holding_one_ready_pull_request();
    for head in ["head-one", "head-two"] {
        world.checks_on(head, vec![stale("test")]);
    }
    let Some(standing) = a_queue_of(Arc::clone(&world), &[(124, "head-two")], 2).await else {
        return;
    };

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the first tend");
    standing.world.checks_on(
        "candidate-of-head-two",
        vec![failing("test", "2099-01-01T00:00:00Z")],
    );
    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the second tend");
    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the third tend, now one at a time");
    standing.world.checks_on(
        "candidate-of-head-one",
        vec![failing("test", "2099-01-01T00:00:00Z")],
    );
    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the fourth tend");

    let alone = standing
        .engine
        .store
        .entry(standing.queue_id, 123)
        .await
        .expect("the entry")
        .expect("an entry");
    assert_eq!(
        alone.status, "Failed",
        "#123 failed on its own, so it is the one at fault"
    );
    let other = standing
        .engine
        .store
        .entry(standing.queue_id, 124)
        .await
        .expect("the entry")
        .expect("an entry");
    assert_eq!(
        other.settled_at, None,
        "#124 was never shown to be at fault and must stay in the queue"
    );
}

#[tokio::test]
async fn a_pull_request_that_stops_merging_leaves_the_batch_and_the_rest_carry_on() {
    let world = World::holding_one_ready_pull_request();
    for head in ["head-one", "head-two"] {
        world.checks_on(head, vec![stale("test")]);
    }
    world.nothing_merges_into("head-two");
    let Some(standing) = a_queue_of(Arc::clone(&world), &[(124, "head-two")], 2).await else {
        return;
    };

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the first tend");
    standing.world.checks_on(
        "candidate-of-head-one",
        vec![passing("test", "2099-01-01T00:00:00Z")],
    );
    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the second tend");

    assert_eq!(
        merged_numbers(&standing.world),
        vec![123],
        "one pull request going bad must not hold up the ones it was travelling with"
    );
    let gone = standing
        .engine
        .store
        .entry(standing.queue_id, 124)
        .await
        .expect("the entry")
        .expect("an entry");
    assert_eq!(gone.status, "Failed");
}

#[tokio::test]
async fn somebody_riding_with_others_is_told_who_else_is_on_the_candidate() {
    let world = World::holding_one_ready_pull_request();
    for head in ["head-one", "head-two"] {
        world.checks_on(head, vec![stale("test")]);
    }
    let Some(standing) = a_queue_of(Arc::clone(&world), &[(124, "head-two")], 2).await else {
        return;
    };

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the first tend");

    let entry = standing
        .engine
        .store
        .entry(standing.queue_id, 124)
        .await
        .expect("the entry")
        .expect("an entry");
    assert!(
        entry.status_detail.contains("#123"),
        "somebody whose own checks are green needs to know whose work they are waiting on, and \
         instead they were told {:?}",
        entry.status_detail
    );
}

#[tokio::test]
async fn a_merge_github_refuses_stops_the_batch_and_leaves_the_rest_in_the_queue() {
    let world = World::holding_one_ready_pull_request();
    for head in ["head-one", "head-two"] {
        world.checks_on(head, vec![stale("test")]);
    }
    world.refuses_to_merge(123);
    let Some(standing) = a_queue_of(Arc::clone(&world), &[(124, "head-two")], 2).await else {
        return;
    };

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the first tend");
    standing.world.checks_on(
        "candidate-of-head-two",
        vec![passing("test", "2099-01-01T00:00:00Z")],
    );
    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the second tend");

    assert!(
        merged_numbers(&standing.world).is_empty(),
        "the first merge was refused, so nothing behind it should have been merged on a \
         verification that is now about a tree nobody asked for"
    );
    let refused = standing
        .engine
        .store
        .entry(standing.queue_id, 123)
        .await
        .expect("the entry")
        .expect("an entry");
    assert_eq!(refused.status, "Failed");
    let behind = standing
        .engine
        .store
        .entry(standing.queue_id, 124)
        .await
        .expect("the entry")
        .expect("an entry");
    assert_eq!(
        behind.settled_at, None,
        "#124 did nothing wrong and must still be in the queue"
    );
    assert!(
        standing
            .engine
            .store
            .verification_carrying(behind.id)
            .await
            .expect("the verification")
            .is_none(),
        "the candidate it rode was tidied away, so the next tend builds a fresh one"
    );
}

#[tokio::test]
async fn a_batch_that_runs_out_of_time_narrows_instead_of_failing_everybody() {
    let world = World::holding_one_ready_pull_request();
    for head in ["head-one", "head-two"] {
        world.checks_on(head, vec![stale("test")]);
    }
    let Some(standing) = a_queue_of(Arc::clone(&world), &[(124, "head-two")], 2).await else {
        return;
    };
    standing.world.files.lock().expect("files").insert(
        ".github/merge-queue.yml".to_owned(),
        "version: 1\nqueues:\n  - branch: main\n    check_timeout: 1s\n".to_owned(),
    );

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the first tend");
    standing.world.checks_on(
        "candidate-of-head-two",
        vec![running("test", "2099-01-01T00:00:00Z")],
    );
    tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the second tend");

    for number in [123, 124] {
        let entry = standing
            .engine
            .store
            .entry(standing.queue_id, number)
            .await
            .expect("the entry")
            .expect("an entry");
        assert_eq!(
            entry.settled_at, None,
            "#{number} shared a candidate that ran out of time, which accuses nobody"
        );
    }
    let queue = standing
        .engine
        .store
        .queue_by_id(standing.queue_id)
        .await
        .expect("the queue")
        .expect("a queue");
    assert_eq!(
        queue.verify_at_once, 1,
        "a batch that never finished is a reason to try fewer, the same as one that failed"
    );
}

#[tokio::test]
async fn a_batch_that_fails_writes_down_that_nobody_is_at_fault_yet() {
    let world = World::holding_one_ready_pull_request();
    for head in ["head-one", "head-two"] {
        world.checks_on(head, vec![stale("test")]);
    }
    let Some(standing) = a_queue_of(Arc::clone(&world), &[(124, "head-two")], 2).await else {
        return;
    };

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the first tend");
    standing.world.checks_on(
        "candidate-of-head-two",
        vec![failing("test", "2099-01-01T00:00:00Z")],
    );
    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the second tend");

    let notes = standing
        .engine
        .store
        .what_happened_to(standing.repository_id, Some(124), 20)
        .await
        .expect("the timeline");
    assert!(
        notes
            .iter()
            .any(|note| note.detail.contains("#123") && note.detail.contains("one candidate")),
        "somebody watching #124 has to be able to see it was held up by a shared failure, and \
         instead the timeline said {:?}",
        notes.iter().map(|note| &note.detail).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn a_repository_that_reports_no_merge_method_at_all_is_a_bad_read_not_a_verdict() {
    let world = World::holding_one_ready_pull_request();
    world.checks_on("head-one", vec![passing("test", "2099-01-01T00:00:00Z")]);
    *world.allow.lock().expect("allow") = Vec::new();
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("a tend");

    let entry = standing
        .engine
        .store
        .entry(standing.queue_id, 123)
        .await
        .expect("the entry")
        .expect("an entry");
    assert_ne!(
        entry.status, "Blocked",
        "github cannot mean that a repository allows no merge method, so believing it and \
         telling somebody their pull request is blocked writes a false verdict onto their work"
    );
    assert!(
        standing.world.merged_pull_requests().is_empty(),
        "nothing should merge from a read the queue does not trust either"
    );
}

#[tokio::test]
async fn a_github_blip_reading_protection_settles_nobody() {
    let world = World::holding_one_ready_pull_request();
    world.checks_on("head-one", vec![passing("test", "2099-01-01T00:00:00Z")]);
    world.cannot_answer_about_protection();
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };

    let outcome = standing.engine.tend(standing.queue_id).await;

    assert!(
        outcome.is_err(),
        "a 503 from github is a blip to come back from, not an answer to decide on"
    );
    let entry = standing
        .engine
        .store
        .entry(standing.queue_id, 123)
        .await
        .expect("the entry")
        .expect("an entry");
    assert_eq!(
        entry.settled_at, None,
        "swallowing the blip is how a pull request gets settled over nothing"
    );
    assert!(standing.world.merged_pull_requests().is_empty());
}

#[tokio::test]
async fn a_deep_queue_is_read_from_github_a_few_at_a_time() {
    let world = World::holding_one_ready_pull_request();
    let deep: Vec<(i32, &str)> = (0..29)
        .map(|at| {
            (
                200 + at,
                Box::leak(format!("head-{at}").into_boxed_str()) as &str,
            )
        })
        .collect();
    for (_, head) in &deep {
        world.checks_on(head, vec![stale("test")]);
    }
    world.checks_on("head-one", vec![stale("test")]);
    let Some(standing) = a_queue_of(Arc::clone(&world), &deep, 1).await else {
        return;
    };

    let each = std::time::Duration::from_millis(60);
    standing.world.takes_this_long_to_answer(each);
    let began = std::time::Instant::now();
    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("a tend");
    let took = began.elapsed();

    let entries = 30;
    let calls_each = 4;
    let one_after_another = each * entries * calls_each;
    assert!(
        took < one_after_another / 2,
        "reading {entries} pull requests one after another, and telling each of them where it \
         stands one after another, would take well over {one_after_another:?}; this tend took \
         {took:?}, which is not the saving concurrency is supposed to buy"
    );
}

#[tokio::test]
async fn a_github_five_hundred_that_clears_on_a_second_ask_does_not_stop_the_queue() {
    let world = World::holding_one_ready_pull_request();
    world.checks_on("head-one", vec![passing("test", "2099-01-01T00:00:00Z")]);
    world.fumbles_protection_this_many_times(1);
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("one 503 that clears is a blip to ride out, not a reason to give up the tend");

    assert_eq!(
        merged_numbers(&standing.world),
        vec![123],
        "the queue should have carried on once github answered"
    );
}

#[tokio::test]
async fn a_labelled_pull_request_no_webhook_told_us_about_is_found_anyway() {
    let world = World::holding_one_ready_pull_request();
    world.checks_on("head-one", vec![passing("test", "2099-01-01T00:00:00Z")]);
    let Some(key) = a_private_key() else { return };
    let store = match a_store_sealed_with(a_seal()).await {
        Some(store) => store,
        None => return,
    };
    let api = listening(Arc::clone(&world)).await;
    let github = Github::new(&api).expect("a client");
    github.adopt(AppAuth::holding(1, key).expect("a usable key"));
    let settings = Settings {
        public_url: "https://merge.example.com".to_owned(),
        listen: "127.0.0.1:0".parse().expect("an address"),
        database_url: String::new(),
        master_key: [0; 32],
        github_api: api,
        github_web: "https://github.com".to_owned(),
    };
    let id = an_id_of_its_own();
    store
        .remember_installation(INSTALLATION, OWNER)
        .await
        .expect("an installation");
    let repository = store
        .remember_repository(id, INSTALLATION, OWNER, &format!("{NAME}-{id}"))
        .await
        .expect("a repository");
    store
        .set_repository_active(repository.id, true)
        .await
        .expect("an active repository");
    let queue = store
        .queue_for(repository.id, "main")
        .await
        .expect("a queue");
    let engine = Engine::new(store, github, Arc::new(settings));

    engine.tend(queue.id).await.expect("a tend");

    assert_eq!(
        world.merged_pull_requests(),
        vec![(123, "squash".to_owned(), "head-one".to_owned())],
        "the label was on the pull request and no webhook ever arrived, so a queue that only \
         learns from webhooks would never have touched it"
    );
}

#[tokio::test]
async fn a_verification_that_never_got_a_candidate_does_not_hold_the_queue_forever() {
    let world = World::holding_one_ready_pull_request();
    world.checks_on("head-one", vec![stale("test")]);
    let Some(standing) = a_queue_of(Arc::clone(&world), &[(124, "head-two")], 1).await else {
        return;
    };
    let entry = standing
        .engine
        .store
        .entry(standing.queue_id, 123)
        .await
        .expect("the entry")
        .expect("an entry");
    standing
        .engine
        .store
        .open_attempt(
            standing.queue_id,
            "base-one",
            "merge-queue/candidate-that-never-was",
            &[(entry.id, "head-one".to_owned())],
            None,
            None,
        )
        .await
        .expect("a verification with no candidate, as a killed tend would leave");
    sqlx::query("update verifications set started_at = now() - interval '10 minutes'")
        .execute(standing.engine.store.pool())
        .await
        .expect("an old verification");

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("a tend");

    let live = standing
        .engine
        .store
        .live_attempts_on(standing.queue_id)
        .await
        .expect("the live attempts");
    assert!(
        live.iter().all(|attempt| attempt.candidate_sha.is_some()),
        "a verification with no candidate can never be concluded by anything, so leaving it \
         live blocks every pull request behind it for good"
    );
}

#[tokio::test]
async fn putting_the_label_back_on_gets_a_pull_request_in_again_without_a_webhook() {
    let world = World::holding_one_ready_pull_request();
    world.checks_on("head-one", vec![passing("test", "2099-01-01T00:00:00Z")]);
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };
    let entry = standing
        .engine
        .store
        .entry(standing.queue_id, 123)
        .await
        .expect("the entry")
        .expect("an entry");
    standing
        .engine
        .store
        .settle(entry.id, "Cancelled", "somebody took it out", None, None)
        .await
        .expect("a settled entry, label still on because the person put it back");

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("a tend");

    assert_eq!(
        standing.world.merged_pull_requests(),
        vec![(123, "squash".to_owned(), "head-one".to_owned())],
        "every way out of the queue takes the label off, so a label that is on again is \
         somebody asking to go back in"
    );
}

async fn the_worker_takes_a_turn(standing: &Standing) {
    let store = &standing.engine.store;
    let ours = subject(standing.queue_id);
    store.ask_for(TEND, &ours).await.expect("a job");
    let job = loop {
        let job = store
            .claim_a_job("test")
            .await
            .expect("a claim")
            .expect("the tend we just asked for");
        if job.subject == ours {
            break job;
        }
        sqlx::query("delete from jobs where id = $1")
            .bind(job.id)
            .execute(store.pool())
            .await
            .expect("another test in this binary left this behind");
    };
    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("a tend");
    store.job_is_done(job.id).await.expect("it finished");
}

async fn when_it_is_to_be_looked_at_again(standing: &Standing) -> Option<OffsetDateTime> {
    sqlx::query_scalar("select run_after from jobs where kind = $1 and subject = $2")
        .bind(TEND)
        .bind(subject(standing.queue_id))
        .fetch_optional(standing.engine.store.pool())
        .await
        .expect("the jobs table should be readable")
}

#[tokio::test]
async fn a_merge_leaves_a_request_to_look_again_behind_it() {
    let world = World::holding_one_ready_pull_request();
    world.checks_on("head-one", vec![passing("test", "2099-01-01T00:00:00Z")]);
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };
    let _alone = support::alone_with_the_jobs(&standing.engine.store).await;

    the_worker_takes_a_turn(&standing).await;

    assert!(
        !standing.world.merged_pull_requests().is_empty(),
        "this tend should have merged something"
    );
    let again = when_it_is_to_be_looked_at_again(&standing)
        .await
        .expect("a merge moves the base, so the queue has to be looked at again");
    assert!(
        again <= OffsetDateTime::now_utc(),
        "the base has moved already, so there is nothing to wait for"
    );
}

#[tokio::test]
async fn a_candidate_still_running_asks_the_queue_to_look_again_in_a_while() {
    let world = World::holding_one_ready_pull_request();
    world.checks_on("head-one", vec![passing("test", "2000-01-01T00:00:00Z")]);
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };
    let _alone = support::alone_with_the_jobs(&standing.engine.store).await;

    the_worker_takes_a_turn(&standing).await;
    standing.world.checks_on(
        "candidate-of-head-one",
        vec![running("test", "2099-01-01T00:00:00Z")],
    );
    the_worker_takes_a_turn(&standing).await;

    let again = when_it_is_to_be_looked_at_again(&standing)
        .await
        .expect("a candidate that is still running has to be looked at again");
    assert!(
        again > OffsetDateTime::now_utc(),
        "nothing has happened yet, so looking again immediately would only spin"
    );
}

#[tokio::test]
async fn a_discarded_verification_leaves_a_request_to_start_again() {
    let world = World::holding_one_ready_pull_request();
    world.checks_on("head-one", vec![passing("test", "2000-01-01T00:00:00Z")]);
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };
    let _alone = support::alone_with_the_jobs(&standing.engine.store).await;

    the_worker_takes_a_turn(&standing).await;
    standing.world.checks_on(
        "candidate-of-head-one",
        vec![passing("test", "2099-01-01T00:00:00Z")],
    );
    standing.world.move_the_base_to("base-two");
    the_worker_takes_a_turn(&standing).await;

    let again = when_it_is_to_be_looked_at_again(&standing)
        .await
        .expect("the result was thrown away, so the candidate has to be built again");
    assert!(
        again <= OffsetDateTime::now_utc(),
        "there is nothing left running, so nothing to wait for"
    );
}

#[tokio::test]
async fn a_pull_request_github_has_not_finished_judging_is_looked_at_again_without_being_asked() {
    let world = World::holding_one_ready_pull_request();
    world.checks_on("head-one", vec![passing("test", "2099-01-01T00:00:00Z")]);
    world.has_not_worked_out_whether_it_merges(123);
    let Some(standing) = a_repository_with(Arc::clone(&world)).await else {
        return;
    };
    let _alone = support::alone_with_the_jobs(&standing.engine.store).await;

    the_worker_takes_a_turn(&standing).await;

    assert!(
        standing.world.merged_pull_requests().is_empty(),
        "nothing merges while GitHub is still working out whether it merges cleanly"
    );
    let again = when_it_is_to_be_looked_at_again(&standing)
        .await
        .expect("GitHub sends no event when it settles, so the queue has to come back and ask");
    assert!(
        again <= OffsetDateTime::now_utc() + Duration::seconds(15),
        "waiting on the sweep leaves a pull request stuck for minutes over a question GitHub \
         answers in seconds"
    );
}

async fn a_queue_speculating_two_deep(world: Arc<World>) -> Option<Standing> {
    world
        .files
        .lock()
        .expect("files")
        .entry(".github/merge-queue.yml".to_owned())
        .or_insert_with(|| {
            "version: 1\nqueues:\n  - branch: main\n    batch_size: 1\n    speculate: 2\n"
                .to_owned()
        });
    let standing = a_queue_of(world, &[(124, "head-two")], 1).await?;
    standing
        .engine
        .store
        .speculate_this_deep_next_time(standing.queue_id, 2, "the test asked for it")
        .await
        .expect("a depth");
    Some(standing)
}

fn two_ready_pull_requests() -> Arc<World> {
    let world = World::holding_one_ready_pull_request();
    world.checks_on("head-one", vec![passing("test", "2000-01-01T00:00:00Z")]);
    world.checks_on("head-two", vec![passing("test", "2000-01-01T00:00:00Z")]);
    world
}

#[tokio::test]
async fn the_next_candidate_is_verified_before_the_one_ahead_has_landed() {
    let world = two_ready_pull_requests();
    let Some(standing) = a_queue_speculating_two_deep(Arc::clone(&world)).await else {
        return;
    };

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the first tend builds the front candidate");
    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the second tend builds one on top of it");

    let live = standing
        .engine
        .store
        .live_attempts_on(standing.queue_id)
        .await
        .expect("the live attempts");
    assert_eq!(
        live.len(),
        2,
        "the second candidate's checks should be running while the first one's still are"
    );
    assert_eq!(live[0].depth, 0);
    assert_eq!(live[1].depth, 1);
    assert_eq!(live[1].built_on, Some(live[0].id));
    assert_eq!(
        live[1].base,
        live[0].candidate_sha.clone().unwrap_or_default(),
        "the one behind is verified on top of the tree the one in front would land"
    );
    assert!(
        standing.world.merged_pull_requests().is_empty(),
        "nothing has passed yet"
    );
}

#[tokio::test]
async fn a_candidate_verified_ahead_of_the_queue_merges_once_the_one_in_front_lands() {
    let world = two_ready_pull_requests();
    let Some(standing) = a_queue_speculating_two_deep(Arc::clone(&world)).await else {
        return;
    };

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the front candidate");
    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the one on top of it");
    standing.world.checks_on(
        "candidate-of-head-one",
        vec![passing("test", "2099-01-01T00:00:00Z")],
    );
    standing.world.checks_on(
        "candidate-of-head-two",
        vec![passing("test", "2099-01-01T00:00:00Z")],
    );
    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the front merges");
    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the one behind merges on the strength of what it already ran");

    assert_eq!(
        standing
            .world
            .merged_pull_requests()
            .iter()
            .map(|(number, _, _)| *number)
            .collect::<Vec<_>>(),
        vec![123, 124],
        "both should land, in order, without the second one being verified again"
    );
    assert_eq!(
        standing.world.draft_pull_requests().len(),
        2,
        "two pull requests landed on two candidate runs, not three"
    );
}

#[tokio::test]
async fn a_candidate_built_on_one_that_failed_is_thrown_away_and_accuses_nobody() {
    let world = two_ready_pull_requests();
    let Some(standing) = a_queue_speculating_two_deep(Arc::clone(&world)).await else {
        return;
    };

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the front candidate");
    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the one on top of it");
    let before = standing
        .engine
        .store
        .queue_by_id(standing.queue_id)
        .await
        .expect("the queue")
        .expect("a queue")
        .verify_at_once;
    standing.world.checks_on(
        "candidate-of-head-one",
        vec![failing("test", "2099-01-01T00:00:00Z")],
    );
    standing.world.checks_on(
        "candidate-of-head-two",
        vec![failing("test", "2099-01-01T00:00:00Z")],
    );
    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the front fails");

    let behind = standing
        .engine
        .store
        .entry(standing.queue_id, 124)
        .await
        .expect("the entry")
        .expect("the one behind");
    assert!(
        behind.settled_at.is_none(),
        "the failure may belong to the work it assumed, so it has not been shown to be at fault"
    );
    assert!(
        standing
            .engine
            .store
            .live_attempts_on(standing.queue_id)
            .await
            .expect("the live attempts")
            .is_empty(),
        "a result about a tree nobody will build is thrown away, not kept"
    );
    let after = standing
        .engine
        .store
        .queue_by_id(standing.queue_id)
        .await
        .expect("the queue")
        .expect("a queue");
    assert_eq!(
        after.verify_at_once, before,
        "how many the queue verifies together is about candidates on the branch's own tip, and \
         a speculative failure says nothing about it"
    );
    assert_eq!(
        after.speculate_to, 1,
        "a chain that was thrown away without being used makes the queue speculate less"
    );
}

#[tokio::test]
async fn a_candidate_does_not_merge_when_something_else_landed_on_the_base_first() {
    let world = two_ready_pull_requests();
    let Some(standing) = a_queue_speculating_two_deep(Arc::clone(&world)).await else {
        return;
    };

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the front candidate");
    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the one on top of it");
    standing.world.checks_on(
        "candidate-of-head-one",
        vec![passing("test", "2099-01-01T00:00:00Z")],
    );
    standing.world.checks_on(
        "candidate-of-head-two",
        vec![passing("test", "2099-01-01T00:00:00Z")],
    );
    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the front merges");
    standing.world.move_the_base_to("somebody-pushed-by-hand");
    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("a tend that finds the base somewhere else");

    assert_eq!(
        standing
            .world
            .merged_pull_requests()
            .iter()
            .map(|(number, _, _)| *number)
            .collect::<Vec<_>>(),
        vec![123],
        "the tree it was verified on top of is not the tree that is there now"
    );
    let behind = standing
        .engine
        .store
        .entry(standing.queue_id, 124)
        .await
        .expect("the entry")
        .expect("the one behind");
    assert!(behind.settled_at.is_none());
}

#[tokio::test]
async fn a_queue_that_halved_its_way_down_to_one_builds_nothing_ahead_of_itself() {
    let world = two_ready_pull_requests();
    world.files.lock().expect("files").insert(
        ".github/merge-queue.yml".to_owned(),
        "version: 1\nqueues:\n  - branch: main\n    batch_size: 3\n    speculate: 2\n".to_owned(),
    );
    let Some(standing) = a_queue_speculating_two_deep(Arc::clone(&world)).await else {
        return;
    };
    standing
        .engine
        .store
        .verify_this_many_next_time(standing.queue_id, 1, "the checks here are flaky")
        .await
        .expect("a width");

    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("the front candidate");
    standing
        .engine
        .tend(standing.queue_id)
        .await
        .expect("a tend that must not build ahead");

    assert_eq!(
        standing
            .engine
            .store
            .live_attempts_on(standing.queue_id)
            .await
            .expect("the live attempts")
            .len(),
        1,
        "a queue that halved its way down to one is telling us its checks are flaky, and every \
         run it makes ahead of itself there is thrown away"
    );
}
