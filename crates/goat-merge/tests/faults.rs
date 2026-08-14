mod support;

use std::sync::Arc;

use axum::body::to_bytes;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use goat_merge::api::{Fault, Incident};
use goat_merge::engine::Engine;
use goat_merge::github::{AppAuth, Github, GithubError};
use goat_merge::settings::Settings;
use serde_json::Value;
use support::fake_github::{World, a_private_key, listening};
use support::{a_seal, a_store_sealed_with};

async fn answered(fault: Fault) -> (StatusCode, Value) {
    let response = fault.into_response();
    let status = response.status();
    let body = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("a body");
    (status, serde_json::from_slice(&body).expect("json"))
}

fn a_database_that_gave_up() -> Fault {
    Fault::Broken {
        incident: Incident::written_down(
            "reading or writing the database",
            &"the database refused a query: error returned from database: LIMIT must not be \
              negative",
        ),
    }
}

fn github_answering_something_we_cannot_read() -> Fault {
    GithubError::Unreadable {
        problem: "missing field `mergeable` at line 1 column 900".to_owned(),
        body: r#"{"url":"https://api.github.com/repos/acme/web/pulls/1","node_id":"PR_kwDO"}"#
            .to_owned(),
    }
    .into()
}

fn github_answering_html() -> Fault {
    GithubError::Refused {
        status: StatusCode::BAD_GATEWAY,
        said: None,
        body: "<html><head><title>502 Bad Gateway</title></head></html>".to_owned(),
    }
    .into()
}

fn a_private_key_that_will_not_sign() -> Fault {
    GithubError::PrivateKeyUnusable {
        problem: "InvalidKeyFormat".to_owned(),
    }
    .into()
}

#[tokio::test]
async fn no_fault_carries_text_this_server_did_not_write() {
    let leaks = [
        "sqlx",
        "missing field",
        "<html",
        "InvalidKeyFormat",
        "line 1 column",
        "node_id",
        "LIMIT must not be negative",
        "Unprocessable Entity",
        "502 Bad Gateway",
    ];

    for fault in [
        a_database_that_gave_up(),
        github_answering_something_we_cannot_read(),
        github_answering_html(),
        a_private_key_that_will_not_sign(),
    ] {
        let (status, body) = answered(fault).await;
        let said = body["error"].as_str().expect("a sentence").to_owned();

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body["kind"], "broken");
        assert!(
            body["incident"].is_string(),
            "a person can only tell an operator which failure was theirs if we give them a \
             number to quote"
        );
        for leak in leaks {
            assert!(
                !said.contains(leak),
                "an internal failure was repeated to whoever asked: {said}"
            );
        }
    }
}

#[tokio::test]
async fn what_github_said_reaches_the_person_who_asked() {
    let (status, body) = answered(
        GithubError::Refused {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            said: Some("No commits between main and merge-queue/setup".to_owned()),
            body: r#"{"message":"Validation Failed"}"#.to_owned(),
        }
        .into(),
    )
    .await;

    let said = body["error"].as_str().expect("a sentence");
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["kind"], "github_refused");
    assert!(
        said.contains("No commits between main and merge-queue/setup"),
        "GitHub is describing the reader's own repository, so its words are the answer: {said}"
    );
    assert!(
        !said.contains("Unprocessable Entity") && !said.contains("422"),
        "the HTTP status name is ours to log, not theirs to read: {said}"
    );
}

#[tokio::test]
async fn every_fault_says_what_it_is_and_where_to_go() {
    let expected = [
        ("not_signed_in", StatusCode::UNAUTHORIZED, true),
        ("not_set_up", StatusCode::SERVICE_UNAVAILABLE, true),
        ("not_installed_here", StatusCode::NOT_FOUND, true),
        ("no_queue_here", StatusCode::NOT_FOUND, true),
        ("never_queued", StatusCode::NOT_FOUND, true),
        ("queue_is_off", StatusCode::CONFLICT, true),
        (
            "configuration_is_invalid",
            StatusCode::UNPROCESSABLE_ENTITY,
            true,
        ),
        ("reason_required", StatusCode::UNPROCESSABLE_ENTITY, false),
        ("github_is_unreachable", StatusCode::BAD_GATEWAY, false),
        (
            "github_is_rate_limiting",
            StatusCode::SERVICE_UNAVAILABLE,
            false,
        ),
    ];

    let faults = [
        Fault::NotSignedIn,
        Fault::NotSetUp,
        Fault::NotInstalledHere {
            owner: "acme".to_owned(),
            name: "web".to_owned(),
        },
        Fault::NoQueueHere {
            owner: "acme".to_owned(),
            name: "web".to_owned(),
            branch: "release".to_owned(),
        },
        Fault::NeverQueued {
            owner: "acme".to_owned(),
            name: "web".to_owned(),
            number: 91,
        },
        Fault::QueueIsOff {
            owner: "acme".to_owned(),
            name: "web".to_owned(),
        },
        Fault::ConfigurationIsInvalid {
            owner: "acme".to_owned(),
            name: "web".to_owned(),
            branch: "main".to_owned(),
            problem: "is not valid YAML: mapping values are not allowed at line 4".to_owned(),
        },
        Fault::ReasonRequired,
        Fault::GithubIsUnreachable,
        Fault::GithubIsRateLimiting { wait: Some(600) },
    ];

    for (fault, (kind, status, goes_somewhere)) in faults.into_iter().zip(expected) {
        let (answered_with, body) = answered(fault).await;
        let said = body["error"].as_str().expect("a sentence");

        assert_eq!(body["kind"], kind);
        assert_eq!(answered_with, status, "{kind}");
        assert!(
            said.ends_with('.') && said.split_whitespace().count() > 5,
            "every one of these is read by a person, so it is a sentence: {said}"
        );
        assert_eq!(
            body["where"].is_string(),
            goes_somewhere,
            "{kind} either has one place to send somebody or none"
        );
        assert!(
            body["incident"].is_null(),
            "{kind} is something they can act on, so there is nothing to quote"
        );
    }
}

#[tokio::test]
async fn a_broken_configuration_names_the_file_it_is_talking_about() {
    let (_, body) = answered(Fault::ConfigurationIsInvalid {
        owner: "acme".to_owned(),
        name: "web".to_owned(),
        branch: "main".to_owned(),
        problem: "is not valid YAML: mapping values are not allowed at line 4".to_owned(),
    })
    .await;

    let said = body["error"].as_str().expect("a sentence");
    assert!(
        said.starts_with(goat_merge_core::config::FILE),
        "the parser writes a fragment with no subject, and a fragment on its own is not an \
         answer: {said}"
    );
    assert_eq!(
        body["where"],
        "https://github.com/acme/web/blob/main/.github/merge-queue.yml"
    );
}

#[tokio::test]
async fn a_rate_limit_says_when_to_come_back() {
    let response = Fault::GithubIsRateLimiting { wait: Some(600) }.into_response();

    assert_eq!(
        response
            .headers()
            .get("retry-after")
            .and_then(|after| after.to_str().ok()),
        Some("600")
    );
}

async fn a_server() -> Option<String> {
    a_server_for(false).await
}

async fn a_server_for(with_a_repository: bool) -> Option<String> {
    a_server_seen_by(with_a_repository, &[])
        .await
        .map(|it| it.0)
}

async fn a_server_seen_by(
    with_a_repository: bool,
    permissions: &[(&str, &str)],
) -> Option<(String, String)> {
    let key = a_private_key()?;
    let store = a_store_sealed_with(a_seal()).await?;
    let world = World::holding_one_ready_pull_request();
    for (login, permission) in permissions {
        world
            .permissions
            .lock()
            .expect("permissions")
            .insert((*login).to_owned(), (*permission).to_owned());
    }
    let api = listening(world).await;
    let github = Github::new(&api).expect("a client");
    github.adopt(AppAuth::holding(1, key).expect("a usable key"));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a port should be free");
    let at = listener.local_addr().expect("an address");
    let settings = Arc::new(Settings {
        public_url: format!("http://{at}"),
        listen: at,
        database_url: String::new(),
        master_key: [0; 32],
        github_api: api,
        github_web: "https://github.com".to_owned(),
    });
    if with_a_repository {
        store
            .remember_installation(4242, "acme")
            .await
            .expect("an installation");
        store
            .remember_repository(770_000, 4242, "acme", "web")
            .await
            .expect("a repository");
    }
    let token = store
        .start_session("stranger", "gho_whatever")
        .await
        .expect("a session");
    let engine = Engine::new(store, github, settings);
    let app = goat_merge::api::router()
        .fallback(goat_merge::web::serve)
        .with_state(engine);
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Some((format!("http://{at}"), token))
}

#[tokio::test]
async fn every_way_of_asking_wrongly_answers_the_same_json() {
    let Some(server) = a_server().await else {
        return;
    };
    let asking = reqwest::Client::new();
    let wrong = [
        asking.get(format!("{server}/api/pull/acme/web/not-a-number")),
        asking.post(format!("{server}/api/pull/acme/web/1/expedite")),
        asking.get(format!("{server}/api/nothing/like/this")),
        asking.get(format!("{server}/api/repositories")),
        asking.get(format!("{server}/api/pull/acme/web/1/enqueue")),
    ];

    for request in wrong {
        let response = request.send().await.expect("an answer");
        let status = response.status();
        let carried = response
            .headers()
            .get("content-type")
            .and_then(|kind| kind.to_str().ok())
            .unwrap_or_default()
            .to_owned();
        let body: Value = response.json().await.expect("json, whatever went wrong");

        assert!(
            status.is_client_error() || status.is_server_error(),
            "{body}"
        );
        assert!(
            carried.starts_with("application/json"),
            "the console and the CLI both parse this, so a plain-text answer is unreadable to \
             both: {carried}"
        );
        assert!(
            body["error"].is_string() && body["kind"].is_string(),
            "an answer with no kind leaves the CLI saying nothing but \"the server refused\": \
             {body}"
        );
    }
}

#[tokio::test]
async fn a_queue_that_is_switched_off_says_so_instead_of_answering_ok() {
    let Some((server, token)) = a_server_seen_by(true, &[("stranger", "admin")]).await else {
        return;
    };

    let response = reqwest::Client::new()
        .post(format!("{server}/api/pull/acme/web/1/enqueue"))
        .header("cookie", format!("goat_merge_session={token}"))
        .send()
        .await
        .expect("an answer");
    let status = response.status();
    let body: Value = response.json().await.expect("json");

    assert_eq!(
        body["kind"], "queue_is_off",
        "answering ok and then doing nothing is worse than refusing: {body}"
    );
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn signing_in_does_not_let_somebody_read_a_repository_they_cannot_see() {
    let Some((server, token)) = a_server_seen_by(true, &[("stranger", "none")]).await else {
        return;
    };

    for path in [
        "/api/repository/acme/web/diagnose",
        "/api/queue/acme/web/main",
        "/api/pull/acme/web/123",
        "/api/history/acme/web/main",
        "/api/insights/acme/web/main",
    ] {
        let response = reqwest::Client::new()
            .get(format!("{server}{path}"))
            .header("cookie", format!("goat_merge_session={token}"))
            .send()
            .await
            .expect("an answer");
        let status = response.status();
        let body: Value = response.json().await.expect("json");

        assert_eq!(
            body["kind"], "not_allowed",
            "any GitHub user could sign in here and read {path}, including a private \
             repository's configuration and pull request titles: {body}"
        );
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
}

#[tokio::test]
async fn somebody_with_read_access_can_still_read() {
    let Some((server, token)) = a_server_seen_by(true, &[("stranger", "read")]).await else {
        return;
    };

    let response = reqwest::Client::new()
        .get(format!("{server}/api/repository/acme/web/diagnose"))
        .header("cookie", format!("goat_merge_session={token}"))
        .send()
        .await
        .expect("an answer");

    assert_eq!(response.status(), StatusCode::OK);
}
