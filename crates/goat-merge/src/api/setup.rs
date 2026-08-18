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
use crate::github::look::{FORK_WORKFLOW, declares_no_secrets};
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

    let diagnosed = Diagnosed {
        web: &engine.settings.github_web,
        full: &full,
        branch: &branch,
        protected: protection.declared_anywhere,
        enforced: false,
        others_required: protection.checks_somebody_else_runs().len(),
        methods: allowed.len(),
        label: label_is_there,
        fork: fork_workflow.as_deref(),
        config: configured.as_deref(),
    };

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
        "fork_workflow": match &fork_workflow {
            None => "missing",
            Some(workflow) if declares_no_secrets(workflow) => "safe",
            Some(_) => "refused",
        },
        "knows_how_to_merge": allowed.len() == 1
            || names_a_merge_method(&diagnosed.what_the_configuration_says()),
        "advice": advice(&Diagnosed {
            enforced: our_check_is_required,
            ..diagnosed
        }),
    }))
    .into_response())
}

#[derive(Debug, Deserialize)]
pub struct AboutBranch {
    branch: Option<String>,
}

struct Diagnosed<'a> {
    web: &'a str,
    full: &'a str,
    branch: &'a str,
    protected: bool,
    enforced: bool,
    others_required: usize,
    methods: usize,
    label: bool,
    fork: Option<&'a str>,
    config: Option<&'a str>,
}

impl Diagnosed<'_> {
    fn a_new_ruleset(&self) -> String {
        format!(
            "{}/{}/settings/rules/new?target=branch",
            self.web, self.full
        )
    }

    fn the_repositorys_settings(&self) -> String {
        format!("{}/{}/settings", self.web, self.full)
    }

    fn the_configuration(&self) -> String {
        match self.config {
            Some(_) => format!("{}/{}/blob/{}/{FILE}", self.web, self.full, self.branch),
            None => format!(
                "{}/{}/new/{}?filename={FILE}",
                self.web, self.full, self.branch
            ),
        }
    }

    fn the_fork_workflow(&self) -> String {
        format!(
            "{}/{}/blob/{}/{FORK_WORKFLOW}",
            self.web, self.full, self.branch
        )
    }

    fn fork_is_trusted(&self) -> bool {
        self.fork.is_some_and(declares_no_secrets)
    }

    fn the_example_fork_workflow() -> String {
        format!(
            "{}/blob/main/.github/merge-queue-fork.example.yml",
            env!("CARGO_PKG_REPOSITORY")
        )
    }

    fn what_the_configuration_says(&self) -> WhatTheConfigurationSays {
        let Some(written) = self.config else {
            return WhatTheConfigurationSays::NothingIsWritten;
        };
        match goat_merge_core::Config::parse(written) {
            Err(problem) => WhatTheConfigurationSays::ItWillNotParse {
                problem: problem.to_string(),
            },
            Ok(config) => match config.queue_for(self.branch) {
                None => WhatTheConfigurationSays::NoQueueForThisBranch,
                Some(queue) => WhatTheConfigurationSays::AQueue {
                    names_a_merge_method: queue.merge_method.is_some(),
                },
            },
        }
    }
}

enum WhatTheConfigurationSays {
    NothingIsWritten,
    ItWillNotParse { problem: String },
    NoQueueForThisBranch,
    AQueue { names_a_merge_method: bool },
}

fn said(level: &str, text: String, where_to_go: Option<String>, follow: Option<&str>) -> Value {
    json!({
        "level": level,
        "text": text,
        "where": where_to_go,
        "follow": follow,
    })
}

fn advice(found: &Diagnosed<'_>) -> Vec<Value> {
    let mut all = Vec::new();
    let written = found.what_the_configuration_says();

    if found.methods == 0 {
        all.push(said(
            "error",
            "This repository allows no merge method at all, so nothing can be merged. Turn one \
             on in the repository's settings, or in whichever ruleset restricts them."
                .to_owned(),
            Some(found.the_repositorys_settings()),
            Some("Open the repository's settings"),
        ));
    }

    match &written {
        WhatTheConfigurationSays::ItWillNotParse { problem } => all.push(said(
            "error",
            format!(
                "{FILE} {problem}. Until it parses, the queue ignores the whole file and falls \
                 back to whatever this repository already does."
            ),
            Some(found.the_configuration()),
            Some("Open the file on GitHub"),
        )),
        WhatTheConfigurationSays::NoQueueForThisBranch => all.push(said(
            "warning",
            format!(
                "{FILE} does not declare a queue for {}, so nothing in it applies to this \
                 branch.",
                found.branch
            ),
            Some(found.the_configuration()),
            Some("Open the file on GitHub"),
        )),
        WhatTheConfigurationSays::NothingIsWritten | WhatTheConfigurationSays::AQueue { .. } => {}
    }

    if found.methods > 1 && !names_a_merge_method(&written) {
        all.push(said(
            "error",
            format!(
                "This repository allows more than one merge method, and {FILE} does not say \
                 which to use. Merge, squash and rebase are three different histories, so the \
                 queue will not guess: until one is named, every pull request here is blocked. \
                 Pick one below and let the queue write the file, or write merge_method \
                 yourself."
            ),
            Some(found.the_configuration()),
            Some("Open the file on GitHub"),
        ));
    }

    if !found.protected {
        all.push(said(
            "warning",
            "This branch has no ruleset or branch protection. A queue can order merges, but \
             nothing stops anyone merging around it."
                .to_owned(),
            Some(found.a_new_ruleset()),
            Some("Add a ruleset on GitHub"),
        ));
    }

    if !found.enforced {
        all.push(said(
            "warning",
            format!(
                "Make {CHECK_NAME} a required check on {}, or a pull request can be merged \
                 without going through the queue. On a new branch ruleset: set Enforcement to \
                 Active, add this branch under Target branches, tick Require status checks to \
                 pass — the Add checks button only appears once you do — then type {CHECK_NAME} \
                 into the search box and choose Add {CHECK_NAME}. It will not be in the list \
                 until the queue has published it once, which is expected. Leave Require \
                 branches to be up to date off: keeping branches current is the work the queue \
                 is here to do. goat-merge will not change your protection rules for you.",
                found.branch
            ),
            Some(found.a_new_ruleset()),
            Some("Add a ruleset on GitHub"),
        ));
    }

    if found.enforced && found.others_required == 0 {
        all.push(said(
            "error",
            format!(
                "{CHECK_NAME} is the only required check on this branch. The queue decides \
                 whether a candidate passed by watching the other required checks, so with none \
                 to watch it merges everything the moment it is labelled, without waiting for \
                 anything to run. Add the checks that actually test this repository."
            ),
            Some(found.a_new_ruleset()),
            Some("Add a ruleset on GitHub"),
        ));
    }

    match found.fork {
        None => {}
        Some(_) if found.fork_is_trusted() => {}
        Some(_) => all.push(said(
            "warning",
            format!(
                "{FORK_WORKFLOW} is there, but it asks for something a stranger's code must not \
                 have, so fork pull requests are still blocked. It has to declare a permissions \
                 block, ask for no write permission, and never mention secrets. Comments are \
                 ignored, so only what the workflow runs counts."
            ),
            Some(found.the_fork_workflow()),
            Some("Open the workflow on GitHub"),
        )),
    }

    if found.fork.is_none() {
        all.push(said(
            "info",
            format!(
                "There is no {FORK_WORKFLOW}, so pull requests from forks are blocked. Copying \
                 a fork's commits onto a branch here would hand a stranger's code the secrets \
                 and write token this repository gives its own workflows. Add one that runs \
                 fork code without either."
            ),
            Some(Diagnosed::the_example_fork_workflow()),
            Some("Copy the example workflow"),
        ));
    }

    if !found.label {
        all.push(said(
            "info",
            format!("The {LABEL} label does not exist yet. Enabling the queue creates it."),
            None,
            None,
        ));
    }

    all
}

fn names_a_merge_method(written: &WhatTheConfigurationSays) -> bool {
    matches!(
        written,
        WhatTheConfigurationSays::AQueue {
            names_a_merge_method: true
        }
    )
}

#[derive(Debug, Deserialize)]
pub struct HowToEnable {
    branch: Option<String>,
    merge_method: Option<MergeMethod>,
    batch_size: Option<usize>,
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
        opened = self_serve_config(
            &engine,
            who,
            &full,
            &branch,
            how.merge_method,
            how.batch_size,
        )
        .await?;
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
    batch_size: Option<usize>,
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
    if let Some(most) = batch_size {
        written.push_str(&format!("    batch_size: {most}\n"));
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

#[cfg(test)]
mod tests {
    use super::{Diagnosed, advice};

    fn everything_in_order<'a>() -> Diagnosed<'a> {
        Diagnosed {
            web: "https://github.com",
            full: "acme/api",
            branch: "main",
            protected: true,
            enforced: true,
            others_required: 2,
            methods: 1,
            label: true,
            fork: Some("permissions:\n  contents: read\n"),
            config: Some("version: 1\nqueues:\n  - branch: main\n"),
        }
    }

    fn what_it_says(found: &Diagnosed<'_>) -> Vec<(String, String)> {
        advice(found)
            .into_iter()
            .map(|one| {
                (
                    one["text"].as_str().unwrap_or_default().to_owned(),
                    one["where"].as_str().unwrap_or_default().to_owned(),
                )
            })
            .collect()
    }

    fn about(found: &Diagnosed<'_>, wanted: &str) -> Option<(String, String)> {
        what_it_says(found)
            .into_iter()
            .find(|(text, _)| text.contains(wanted))
    }

    #[test]
    fn a_repository_with_nothing_left_to_do_is_told_nothing() {
        assert!(what_it_says(&everything_in_order()).is_empty());
    }

    #[test]
    fn every_thing_worth_doing_says_where_to_go_and_do_it() {
        let nothing_done = Diagnosed {
            protected: false,
            enforced: false,
            others_required: 0,
            methods: 3,
            label: false,
            fork: None,
            config: None,
            ..everything_in_order()
        };

        for (text, where_to_go) in what_it_says(&nothing_done) {
            let is_the_label_line = text.contains("label does not exist yet");
            assert!(
                is_the_label_line || !where_to_go.is_empty(),
                "a person reading this has to work out where to go on their own: {text}"
            );
        }
    }

    #[test]
    fn naming_a_merge_method_in_the_file_settles_the_question() {
        let asked = Diagnosed {
            methods: 3,
            config: Some("version: 1\nqueues:\n  - branch: main\n"),
            ..everything_in_order()
        };
        let answered = Diagnosed {
            config: Some("version: 1\nqueues:\n  - branch: main\n    merge_method: squash\n"),
            ..asked
        };

        assert!(about(&asked, "does not say which to use").is_some());
        assert!(
            about(&answered, "does not say which to use").is_none(),
            "the file answers the question, so the screen must stop asking it"
        );
    }

    #[test]
    fn a_configuration_that_will_not_parse_is_not_passed_over_in_silence() {
        let broken = Diagnosed {
            config: Some("version: 1\nqueues:\n  - branch: main\n    batchSize: 5\n"),
            ..everything_in_order()
        };

        let (text, where_to_go) =
            about(&broken, "falls back").expect("a file the queue ignores is worth saying");
        assert!(text.contains("unknown field"), "{text}");
        assert!(where_to_go.contains("blob/main/.github/merge-queue.yml"));
    }

    #[test]
    fn a_configuration_that_says_nothing_about_this_branch_is_not_passed_over_either() {
        let elsewhere = Diagnosed {
            config: Some("version: 1\nqueues:\n  - branch: develop\n    merge_method: squash\n"),
            ..everything_in_order()
        };

        assert!(about(&elsewhere, "does not declare a queue for main").is_some());
    }

    #[test]
    fn our_own_check_standing_alone_is_called_out_as_testing_nothing() {
        let only_ours = Diagnosed {
            others_required: 0,
            ..everything_in_order()
        };

        let (text, _) = about(&only_ours, "only required check")
            .expect("a queue that merges everything untested is the worst state on this screen");
        assert!(
            text.contains("without waiting for anything to run"),
            "{text}"
        );
    }

    #[test]
    fn a_fork_workflow_we_refuse_is_told_apart_from_one_that_is_missing() {
        let refused = Diagnosed {
            fork: Some("permissions:\n  contents: write\n"),
            ..everything_in_order()
        };
        let missing = Diagnosed {
            fork: None,
            ..everything_in_order()
        };

        assert!(about(&refused, "is there, but it asks for something").is_some());
        assert!(about(&refused, "There is no").is_none());
        assert!(about(&missing, "There is no").is_some());
        assert!(about(&missing, "is there, but it asks for something").is_none());
    }

    #[test]
    fn the_way_to_require_our_check_names_the_step_people_miss() {
        let unenforced = Diagnosed {
            enforced: false,
            ..everything_in_order()
        };

        let (text, where_to_go) = about(&unenforced, "Make Merge Queue a required check")
            .expect("this is the whole reason the screen exists");
        assert!(text.contains("Add checks button only appears"), "{text}");
        assert!(
            text.contains("type Merge Queue into the search box"),
            "{text}"
        );
        assert!(where_to_go.contains("settings/rules/new"));
    }
}
