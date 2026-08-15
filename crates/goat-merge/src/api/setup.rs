use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use goat_merge_core::config::{FILE, LABEL};
use goat_merge_core::{CHECK_NAME, MergeMethod};
use serde::Deserialize;
use serde_json::{Value, json};

use super::auth::{Standing, whoever_is_asking};
use super::queues::{found_for_reading, ready_to_set_up};
use super::{Answer, Fault, Incident, Named, Sent, Wanting};
use crate::engine::Engine;
use crate::github::look::FORK_WORKFLOW;
use crate::github::{AppAuth, As};
use crate::store::credentials::AppCredentials;

pub async fn manifest(State(engine): State<Engine>) -> Response {
    let settings = &engine.settings;
    let manifest = json!({
        "url": settings.public_url,
        "hook_attributes": { "url": settings.webhook_url(), "active": true },
        "redirect_url": settings.setup_url(),
        "callback_urls": [settings.callback_url()],
        "setup_url": settings.callback_url(),
        "request_oauth_on_install": true,
        "setup_on_update": false,
        "public": true,
        "default_permissions": {
            "metadata": "read",
            "contents": "write",
            "pull_requests": "write",
            "checks": "write",
            "statuses": "read",
            "administration": "read",
        },
        "default_events": [
            "pull_request",
            "pull_request_review",
            "check_run",
            "check_suite",
            "status",
            "push",
        ],
    });
    Answer(json!({
        "where": format!("{}/settings/apps/new", settings.github_web),
        "manifest": manifest.to_string(),
        "already_set_up": engine.github.is_set_up(),
    }))
    .into_response()
}

#[derive(Debug, Deserialize)]
pub struct Handback {
    code: Option<String>,
}

pub async fn app_was_created(
    State(engine): State<Engine>,
    headers: HeaderMap,
    Wanting(handback): Wanting<Handback>,
) -> Response {
    match taking_the_new_app(&engine, &headers, handback).await {
        Ok(response) => response,
        Err(fault) => fault.told_to_the_browser(),
    }
}

async fn taking_the_new_app(
    engine: &Engine,
    headers: &HeaderMap,
    handback: Handback,
) -> Result<Response, Fault> {
    if engine.github.is_set_up() {
        whoever_is_asking(engine, headers).await?;
    }
    let Some(code) = handback.code else {
        return Err(Fault::GithubRefused {
            said: "it handed back no code for the new App".to_owned(),
        });
    };

    #[derive(Deserialize)]
    struct Converted {
        id: i64,
        slug: String,
        client_id: String,
        client_secret: String,
        webhook_secret: Option<String>,
        pem: String,
    }
    let converted: Converted = engine
        .github
        .http()
        .post(format!(
            "{}/app-manifests/{code}/conversions",
            engine.settings.github_api
        ))
        .header("accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|_| Fault::GithubIsUnreachable)?
        .json()
        .await
        .map_err(|problem| Fault::Broken {
            incident: Incident::written_down("reading the new App's credentials", &problem),
        })?;

    let credentials = AppCredentials {
        app_id: converted.id,
        slug: converted.slug.clone(),
        client_id: converted.client_id,
        private_key: converted.pem,
        webhook_secret: converted.webhook_secret.unwrap_or_default(),
        client_secret: converted.client_secret,
    };
    engine.store.remember_app(&credentials).await?;
    let auth =
        AppAuth::holding(credentials.app_id, &credentials.private_key).map_err(|problem| {
            Fault::Broken {
                incident: Incident::written_down("using the new App's private key", &problem),
            }
        })?;
    engine.github.adopt(auth);

    where_somebody_installs_it(engine, &converted.slug)
}

pub async fn install(State(engine): State<Engine>) -> Response {
    match engine.store.app_credentials().await {
        Ok(Some(app)) => match where_somebody_installs_it(&engine, &app.slug) {
            Ok(response) => response,
            Err(fault) => fault.told_to_the_browser(),
        },
        Ok(None) => Fault::NotSetUp.told_to_the_browser(),
        Err(problem) => Fault::from(problem).told_to_the_browser(),
    }
}

fn where_somebody_installs_it(engine: &Engine, slug: &str) -> Result<Response, Fault> {
    super::auth::a_state_to_come_back_with(
        &engine.settings,
        &format!(
            "{}/apps/{slug}/installations/new?state=",
            engine.settings.github_web
        ),
    )
}

pub async fn diagnose(
    State(engine): State<Engine>,
    headers: HeaderMap,
    Named((owner, name)): Named<(String, String)>,
    Wanting(about): Wanting<AboutBranch>,
) -> Result<Response, Fault> {
    let repository = found_for_reading(&engine, &headers, &owner, &name).await?;
    let who = As(repository.installation_id);
    let full = repository.full_name();

    let settings = engine.github.repository_settings(who, &full).await?;
    let branch = about
        .branch
        .unwrap_or_else(|| settings.default_branch.clone());
    let protection = engine.github.protection_of(who, &full, &branch).await?;
    let label_is_there = engine.github.label_exists(who, &full, LABEL).await?;
    let configured = engine.github.file(who, &full, FILE, &branch).await?;
    let fork_workflow = engine
        .github
        .file(who, &full, FORK_WORKFLOW, &branch)
        .await?;

    let allowed: Vec<String> = crate::github::look::allowed_by(&settings, &protection)
        .iter()
        .map(std::string::ToString::to_string)
        .collect();

    let our_check_is_required = protection
        .required_checks
        .iter()
        .any(|check| check == CHECK_NAME);

    Ok(Answer(json!({
        "owner": owner,
        "name": name,
        "branch": branch,
        "active": repository.active && repository.we_can_still_see_it(),
        "protection": {
            "declared": protection.declared_anywhere,
            "required_checks": protection.required_checks,
            "required_approvals": protection.required_approvals,
        },
        "merge_methods": allowed,
        "label": { "name": LABEL, "exists": label_is_there },
        "enforced": our_check_is_required,
        "check_name": CHECK_NAME,
        "config": configured,
        "fork_workflow": fork_workflow.is_some(),
        "advice": advice(
            protection.declared_anywhere,
            our_check_is_required,
            allowed.len(),
            label_is_there,
        ),
    }))
    .into_response())
}

#[derive(Debug, Deserialize)]
pub struct AboutBranch {
    branch: Option<String>,
}

fn advice(protected: bool, enforced: bool, methods: usize, label: bool) -> Vec<Value> {
    let mut said = Vec::new();
    if !protected {
        said.push(json!({
            "level": "warning",
            "text": "This branch has no ruleset or branch protection. A queue can order merges, \
                     but nothing stops anyone merging around it.",
        }));
    }
    if !enforced {
        said.push(json!({
            "level": "warning",
            "text": format!(
                "Add the {CHECK_NAME} check to this branch's required checks. Until you do, a \
                 pull request can be merged without going through the queue. goat-merge will not \
                 change your protection rules for you."
            ),
        }));
    }
    if methods == 0 {
        said.push(json!({
            "level": "error",
            "text": "This repository allows no merge method at all, so nothing can be merged.",
        }));
    }
    if methods > 1 {
        said.push(json!({
            "level": "info",
            "text": format!(
                "This repository allows more than one merge method, so say which one in {FILE}."
            ),
        }));
    }
    if !label {
        said.push(json!({
            "level": "info",
            "text": format!("The {LABEL} label does not exist yet. Enabling the queue creates it."),
        }));
    }
    said
}

#[derive(Debug, Deserialize)]
pub struct HowToEnable {
    branch: Option<String>,
    merge_method: Option<MergeMethod>,
    write_config: Option<bool>,
}

pub async fn enable(
    State(engine): State<Engine>,
    headers: HeaderMap,
    Named((owner, name)): Named<(String, String)>,
    Sent(how): Sent<HowToEnable>,
) -> Result<Response, Fault> {
    let (repository, actor) =
        ready_to_set_up(&engine, &headers, &owner, &name, Standing::Admin).await?;
    let who = As(repository.installation_id);
    let full = repository.full_name();

    let settings = engine.github.repository_settings(who, &full).await?;
    let branch = how
        .branch
        .unwrap_or_else(|| settings.default_branch.clone());

    engine
        .github
        .make_sure_label_exists(who, &full, LABEL)
        .await?;
    engine
        .store
        .set_repository_active(repository.id, true)
        .await?;
    let queue = engine.store.queue_for(repository.id, &branch).await?;

    let mut opened = None;
    if how.write_config.unwrap_or(false) {
        opened = self_serve_config(&engine, who, &full, &branch, how.merge_method).await?;
    }

    engine
        .store
        .write_down(
            &actor,
            "enabled the queue",
            Some(repository.id),
            None,
            &branch,
        )
        .await?;
    engine.tend_queue_soon(queue.id).await?;
    engine.announce(&full);

    Ok(
        Answer(json!({ "ok": true, "branch": branch, "config_pull_request": opened }))
            .into_response(),
    )
}

async fn self_serve_config(
    engine: &Engine,
    who: As,
    full: &str,
    branch: &str,
    method: Option<MergeMethod>,
) -> Result<Option<i32>, Fault> {
    if already_says_so(engine, who, full, branch).await? {
        return Ok(None);
    }
    let tip = engine.github.branch_tip(who, full, branch).await?;
    let working = "merge-queue/setup";
    engine.github.make_branch(who, full, working, &tip).await?;
    let mut written = format!("version: 1\n\nqueues:\n  - branch: {branch}\n");
    if let Some(method) = method {
        written.push_str(&format!("    merge_method: {method}\n"));
    }
    engine
        .github
        .write_file(
            who,
            full,
            FILE,
            working,
            &written,
            "Add a merge queue for this branch",
        )
        .await?;
    let opened = engine
        .github
        .open_pull_request(
            who,
            full,
            "Add a merge queue",
            working,
            branch,
            &format!(
                "This adds `{FILE}`, which is where the queue's settings live so they can be \
                 reviewed and rolled back like anything else.\n\nWith this merged, adding the \
                 `{LABEL}` label to a pull request queues it for `{branch}`."
            ),
        )
        .await?;
    Ok(Some(opened.number))
}

async fn already_says_so(
    engine: &Engine,
    who: As,
    full: &str,
    branch: &str,
) -> Result<bool, Fault> {
    let Some(written) = engine.github.file(who, full, FILE, branch).await? else {
        return Ok(false);
    };
    Ok(goat_merge_core::Config::parse(&written)
        .is_ok_and(|config| config.queue_for(branch).is_some()))
}

pub async fn disable(
    State(engine): State<Engine>,
    headers: HeaderMap,
    Named((owner, name)): Named<(String, String)>,
) -> Result<Response, Fault> {
    let (repository, actor) =
        ready_to_set_up(&engine, &headers, &owner, &name, Standing::Admin).await?;
    let full = repository.full_name();
    engine
        .store
        .set_repository_active(repository.id, false)
        .await?;
    engine
        .store
        .write_down(
            &actor,
            "turned the queue off",
            Some(repository.id),
            None,
            "",
        )
        .await?;
    engine.announce(&full);
    Ok(Answer(json!({ "ok": true })).into_response())
}
