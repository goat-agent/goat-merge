mod support;

use std::sync::Arc;

use goat_merge::engine::Engine;
use goat_merge::github::{AppAuth, Github};
use goat_merge::settings::Settings;
use support::fake_github::{
    INSTALLATION, NAME, OWNER, World, a_private_key, failing, listening, passing, running,
};
use support::{a_seal, a_store_sealed_with, an_id_of_its_own};

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
        .settle(entry.id, "Failed", "a check failed", None)
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
        .verify_this_many_next_time(standing.queue_id, at_once)
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
