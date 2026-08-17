mod support;

use goat_merge::store::credentials::AppCredentials;
use goat_merge::store::{Seal, fresh_key, key_from_hex};
use support::{a_repository, a_seal, a_store, a_store_sealed_with};
use time::Duration;

fn an_app() -> AppCredentials {
    AppCredentials {
        app_id: 42,
        slug: "merge-queue".to_owned(),
        client_id: "Iv1.abcdef".to_owned(),
        private_key: "-----BEGIN RSA PRIVATE KEY-----\nnot really\n".to_owned(),
        webhook_secret: "a shared secret".to_owned(),
        client_secret: "another shared secret".to_owned(),
    }
}

#[tokio::test]
async fn the_schema_can_be_brought_up_to_date_twice() {
    let Some(_store) = a_store().await else {
        return;
    };
    let Some(_again) = a_store().await else {
        return;
    };
}

#[tokio::test]
async fn a_secret_is_not_stored_in_the_clear() {
    let key = fresh_key();
    let seal = Seal::holding(key_from_hex(&key).expect("a fresh key is 32 bytes"));
    let Some(store) = a_store_sealed_with(seal).await else {
        return;
    };
    store
        .remember_app(&an_app())
        .await
        .expect("the app should be remembered");

    let stored: (Vec<u8>,) =
        sqlx::query_as("select webhook_secret from app_credentials where only_row = 1")
            .fetch_one(store.pool())
            .await
            .expect("the row should be there");

    assert!(
        !String::from_utf8_lossy(&stored.0).contains("a shared secret"),
        "the webhook secret must not be readable straight out of the table"
    );
}

#[tokio::test]
async fn a_secret_written_under_one_key_will_not_open_under_another() {
    let Some(store) = a_store().await else { return };
    store
        .remember_app(&an_app())
        .await
        .expect("the app should be remembered");

    let Some(stranger) = a_store_sealed_with(a_seal()).await else {
        return;
    };

    assert!(
        stranger.app_credentials().await.is_err(),
        "a different master key must not be able to read the private key"
    );
}

#[tokio::test]
async fn a_pull_request_goes_in_waits_and_settles() {
    let Some(store) = a_store().await else { return };
    let repository = a_repository(&store).await;
    let queue = store
        .queue_for(repository.id, "main")
        .await
        .expect("a queue");

    let entry = store
        .enter(queue.id, 4835, "frank", "a change")
        .await
        .expect("an entry");
    assert_eq!(
        entry.queued_at, None,
        "an entry states an intent before it takes a number"
    );

    store.take_a_number(entry.id).await.expect("a number");
    let running = store.running(queue.id).await.expect("the running queue");
    assert_eq!(running.len(), 1);
    assert!(running[0].queued_at.is_some());

    store
        .record_status(entry.id, "Checking", "4/5 checks passed")
        .await
        .expect("a status");
    store
        .settle(entry.id, "Merged", "merged", Some("merged-sha"))
        .await
        .expect("a settlement");

    assert!(
        store
            .running(queue.id)
            .await
            .expect("the running queue")
            .is_empty()
    );
    let history = store.history(queue.id, 10).await.expect("the history");
    assert_eq!(history[0].merged_sha.as_deref(), Some("merged-sha"));
}

#[tokio::test]
async fn an_expedited_pull_request_goes_to_the_front() {
    let Some(store) = a_store().await else { return };
    let repository = a_repository(&store).await;
    let queue = store
        .queue_for(repository.id, "main")
        .await
        .expect("a queue");

    let first = store
        .enter(queue.id, 1, "frank", "a change")
        .await
        .expect("an entry");
    store.take_a_number(first.id).await.expect("a number");
    let second = store
        .enter(queue.id, 2, "alex", "a change")
        .await
        .expect("an entry");
    store.take_a_number(second.id).await.expect("a number");

    let order: Vec<i32> = store
        .running(queue.id)
        .await
        .expect("the running queue")
        .iter()
        .map(|entry| entry.pull_request)
        .collect();
    assert_eq!(order, vec![1, 2]);

    store
        .expedite(second.id, "remy", "production incident")
        .await
        .expect("an expedite");

    let order: Vec<i32> = store
        .running(queue.id)
        .await
        .expect("the running queue")
        .iter()
        .map(|entry| entry.pull_request)
        .collect();
    assert_eq!(
        order,
        vec![2, 1],
        "the expedited pull request is verified first"
    );
}

#[tokio::test]
async fn re_entering_clears_an_earlier_result_without_losing_the_attempts() {
    let Some(store) = a_store().await else { return };
    let repository = a_repository(&store).await;
    let queue = store
        .queue_for(repository.id, "main")
        .await
        .expect("a queue");

    let entry = store
        .enter(queue.id, 7, "frank", "a change")
        .await
        .expect("an entry");
    let attempt = store
        .open_attempt(
            queue.id,
            "base-one",
            "merge-queue/candidate-1",
            &[(entry.id, "head-one".to_owned())],
            None,
        )
        .await
        .expect("an attempt");
    store
        .conclude_attempt(attempt.id, "failure", &["test".to_owned()])
        .await
        .expect("a conclusion");
    store
        .settle(entry.id, "Failed", "check test failed", None)
        .await
        .expect("a settlement");

    let again = store
        .enter(queue.id, 7, "frank", "a change")
        .await
        .expect("the same entry");
    assert_eq!(again.id, entry.id);
    assert_eq!(again.settled_at, None);
    assert_eq!(
        store
            .attempts_carrying(entry.id)
            .await
            .expect("the attempts")
            .len(),
        1,
        "a retry adds an attempt, it does not overwrite the last one"
    );
}

#[tokio::test]
async fn a_discarded_attempt_stops_being_the_live_one() {
    let Some(store) = a_store().await else { return };
    let repository = a_repository(&store).await;
    let queue = store
        .queue_for(repository.id, "main")
        .await
        .expect("a queue");
    let entry = store
        .enter(queue.id, 9, "frank", "a change")
        .await
        .expect("an entry");

    let stale = store
        .open_attempt(
            queue.id,
            "base-one",
            "merge-queue/candidate-1",
            &[(entry.id, "head-one".to_owned())],
            None,
        )
        .await
        .expect("an attempt");
    assert_eq!(
        store
            .verification_carrying(entry.id)
            .await
            .expect("the live attempt")
            .map(|a| a.id),
        Some(stale.id)
    );

    store
        .discard_attempt(stale.id, "the base moved")
        .await
        .expect("a discard");

    assert_eq!(
        store
            .verification_carrying(entry.id)
            .await
            .expect("no live attempt"),
        None
    );
    assert_eq!(
        store
            .attempts_carrying(entry.id)
            .await
            .expect("the attempts")
            .len(),
        1
    );
}

#[tokio::test]
async fn the_same_delivery_is_only_written_down_once() {
    let Some(store) = a_store().await else { return };
    let id = format!("delivery-{}", support::a_repository(&store).await.id);
    let payload = serde_json::json!({ "action": "labeled" });

    assert!(
        store
            .write_down_delivery(&id, "pull_request", &payload)
            .await
            .expect("first")
    );
    assert!(
        !store
            .write_down_delivery(&id, "pull_request", &payload)
            .await
            .expect("second"),
        "GitHub retries deliveries, and a retry must not be treated as a new event"
    );
}

#[tokio::test]
async fn only_one_worker_claims_a_job() {
    let Some(store) = a_store().await else { return };
    let _alone = support::alone_with_the_jobs(&store).await;

    store.ask_for("sweep", "acme/api").await.expect("a job");
    store
        .ask_for("sweep", "acme/api")
        .await
        .expect("the same job again");

    let first = store.claim_a_job("one").await.expect("a claim");
    let second = store.claim_a_job("two").await.expect("a claim");

    assert!(
        first.is_some(),
        "asking twice still queues one claimable job"
    );
    assert!(
        second.is_none(),
        "the second worker must not get the same job"
    );
}

#[tokio::test]
async fn a_job_that_went_wrong_comes_back_later() {
    let Some(store) = a_store().await else { return };
    let _alone = support::alone_with_the_jobs(&store).await;

    store.ask_for("sweep", "acme/api").await.expect("a job");
    let job = store
        .claim_a_job("one")
        .await
        .expect("a claim")
        .expect("the job we just asked for");

    store
        .job_went_wrong(job.id, "GitHub was busy", Duration::minutes(5))
        .await
        .expect("a failure");

    assert!(
        store.claim_a_job("one").await.expect("a claim").is_none(),
        "a job that failed waits before it is tried again"
    );
}

#[tokio::test]
async fn a_worker_that_died_holding_a_job_does_not_keep_it_forever() {
    let Some(store) = a_store().await else { return };
    let _alone = support::alone_with_the_jobs(&store).await;

    store.ask_for("sweep", "acme/api").await.expect("a job");
    store
        .claim_a_job("one")
        .await
        .expect("a claim")
        .expect("the job");

    let freed = store
        .release_jobs_held_since_before(Duration::seconds(-1))
        .await
        .expect("a sweep");

    assert_eq!(freed, 1);
    assert!(
        store.claim_a_job("two").await.expect("a claim").is_some(),
        "after a restart the work has to be picked up again"
    );
}

#[tokio::test]
async fn a_job_asked_for_again_while_it_ran_runs_again() {
    let Some(store) = a_store().await else { return };
    let _alone = support::alone_with_the_jobs(&store).await;

    store.ask_for("sweep", "acme/api").await.expect("a job");
    let job = store
        .claim_a_job("one")
        .await
        .expect("a claim")
        .expect("the job we just asked for");

    store
        .ask_for("sweep", "acme/api")
        .await
        .expect("somebody asks while it is running");
    store.job_is_done(job.id).await.expect("it finished");

    assert!(
        store.claim_a_job("two").await.expect("a claim").is_some(),
        "a request that arrived while the job ran must survive the job finishing"
    );
}

#[tokio::test]
async fn a_job_asked_for_later_while_it_ran_keeps_the_delay() {
    let Some(store) = a_store().await else { return };
    let _alone = support::alone_with_the_jobs(&store).await;

    store.ask_for("sweep", "acme/api").await.expect("a job");
    let job = store
        .claim_a_job("one")
        .await
        .expect("a claim")
        .expect("the job we just asked for");

    store
        .ask_for_later("sweep", "acme/api", Duration::minutes(5))
        .await
        .expect("the job asks to be looked at again in a while");
    store.job_is_done(job.id).await.expect("it finished");

    assert!(
        store.claim_a_job("two").await.expect("a claim").is_none(),
        "a job that asked to run again later must not run again straight away"
    );
}

#[tokio::test]
async fn the_soonest_of_two_requests_made_while_it_ran_is_the_one_that_wins() {
    let Some(store) = a_store().await else { return };
    let _alone = support::alone_with_the_jobs(&store).await;

    store.ask_for("sweep", "acme/api").await.expect("a job");
    let job = store
        .claim_a_job("one")
        .await
        .expect("a claim")
        .expect("the job we just asked for");

    store
        .ask_for_later("sweep", "acme/api", Duration::minutes(5))
        .await
        .expect("the job asks to be looked at again in a while");
    store
        .ask_for("sweep", "acme/api")
        .await
        .expect("a webhook arrives before that while is up");
    store.job_is_done(job.id).await.expect("it finished");

    assert!(
        store.claim_a_job("two").await.expect("a claim").is_some(),
        "news that arrived while the job ran must not wait behind the job's own longer wait"
    );
}

#[tokio::test]
async fn asking_twice_while_it_ran_only_runs_it_once_more() {
    let Some(store) = a_store().await else { return };
    let _alone = support::alone_with_the_jobs(&store).await;

    store.ask_for("sweep", "acme/api").await.expect("a job");
    let job = store
        .claim_a_job("one")
        .await
        .expect("a claim")
        .expect("the job we just asked for");

    store.ask_for("sweep", "acme/api").await.expect("once");
    store.ask_for("sweep", "acme/api").await.expect("twice");
    store.job_is_done(job.id).await.expect("it finished");

    let again = store
        .claim_a_job("two")
        .await
        .expect("a claim")
        .expect("the one job those two requests became");
    store.job_is_done(again.id).await.expect("it finished");

    assert!(
        store.claim_a_job("two").await.expect("a claim").is_none(),
        "two requests for the same work are one job, not two"
    );
}

#[tokio::test]
async fn a_job_nobody_asked_for_again_is_cleared() {
    let Some(store) = a_store().await else { return };
    let _alone = support::alone_with_the_jobs(&store).await;

    store.ask_for("sweep", "acme/api").await.expect("a job");
    let job = store
        .claim_a_job("one")
        .await
        .expect("a claim")
        .expect("the job we just asked for");
    store.job_is_done(job.id).await.expect("it finished");

    assert!(
        store.claim_a_job("two").await.expect("a claim").is_none(),
        "work nobody asked for again is finished with"
    );
}

#[tokio::test]
async fn a_job_asked_for_again_does_not_carry_the_last_run_s_attempts() {
    let Some(store) = a_store().await else { return };
    let _alone = support::alone_with_the_jobs(&store).await;

    store.ask_for("sweep", "acme/api").await.expect("a job");
    let job = store
        .claim_a_job("one")
        .await
        .expect("a claim")
        .expect("the job we just asked for");
    store
        .ask_for("sweep", "acme/api")
        .await
        .expect("somebody asks while it is running");
    store.job_is_done(job.id).await.expect("it finished");

    let again = store
        .claim_a_job("two")
        .await
        .expect("a claim")
        .expect("the work that was asked for again");

    assert_eq!(
        again.attempts, 1,
        "how long a failure waits must depend on how often this work has failed, not on how \
         busy the queue has been"
    );
}
