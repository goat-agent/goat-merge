use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use goat_merge_core::{Aboard, Config, InQueue, Queue as QueueRules, assess, decide};
use serde::Deserialize;
use serde_json::json;

use super::queues::found_for_reading;
use super::{Answer, Fault, Named, Sent, Wanting};
use crate::engine::Engine;
use crate::github::As;
use crate::github::look::WhereTheBaseIs;

#[derive(Debug, Deserialize)]
pub struct WhatIf {
    config: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OnBranch {
    branch: Option<String>,
}

pub async fn find(
    State(engine): State<Engine>,
    headers: HeaderMap,
    Named((owner, name)): Named<(String, String)>,
    Wanting(on): Wanting<OnBranch>,
) -> Result<Response, Fault> {
    let repository = found_for_reading(&engine, &headers, &owner, &name).await?;
    let branch = match on.branch {
        Some(branch) => branch,
        None => {
            engine
                .github
                .repository_settings(As(repository.installation_id), &repository.full_name())
                .await?
                .default_branch
        }
    };
    let found = engine
        .github
        .open_pulls_for_head(
            As(repository.installation_id),
            &repository.full_name(),
            &owner,
            &branch,
        )
        .await?;
    Ok(Answer(json!({
        "pull_requests": found.iter().map(|pull| json!({
            "number": pull.number,
            "title": pull.title,
            "base": pull.base.branch,
        })).collect::<Vec<_>>(),
    }))
    .into_response())
}

pub async fn simulate(
    State(engine): State<Engine>,
    headers: HeaderMap,
    Named((owner, name, number)): Named<(String, String, i32)>,
    Sent(what_if): Sent<WhatIf>,
) -> Result<Response, Fault> {
    let repository = found_for_reading(&engine, &headers, &owner, &name).await?;
    let who = As(repository.installation_id);
    let full = repository.full_name();

    let pull = engine.github.pull_request(who, &full, number).await?;
    let branch = pull.base.branch.clone();
    let tip = engine.github.branch_tip(who, &full, &branch).await?;
    let queue = engine
        .store
        .queue_on(repository.id, &branch)
        .await?
        .ok_or_else(|| Fault::NoQueueHere {
            owner: owner.clone(),
            name: name.clone(),
            branch: branch.clone(),
        })?;
    let settings = engine.github.repository_settings(who, &full).await?;
    let protection = engine.github.protection_of(who, &full, &branch).await?;

    let rules = match what_if.config.as_deref() {
        Some(written) => match Config::parse(written) {
            Ok(parsed) => match parsed.queue_for(&branch) {
                Some(found) => found.clone(),
                None => {
                    return Ok(Answer(json!({
                        "status": "Not queued",
                        "detail": format!("this configuration has no queue for {branch}"),
                        "next": "nothing",
                    }))
                    .into_response());
                }
            },
            Err(problem) => {
                return Err(Fault::ConfigurationIsInvalid {
                    owner: owner.clone(),
                    name: name.clone(),
                    branch: branch.clone(),
                    problem: problem.to_string(),
                });
            }
        },
        None => QueueRules::following_github(&branch),
    };

    let looked = engine
        .github
        .look_at(
            who,
            &full,
            number,
            &WhereTheBaseIs {
                sha: tip,
                moved_at: queue.base_moved_at,
            },
            &settings,
            &protection,
        )
        .await?;

    let readiness = assess(&looked.snapshot);
    let on_its_own = [Aboard {
        number: looked.snapshot.number,
        head: looked.snapshot.head.clone(),
    }];
    let decision = decide(
        &looked.snapshot,
        &rules,
        &InQueue {
            ahead: 0,
            aboard: &on_its_own,
            paused: queue.paused,
            verification: None,
        },
    );

    Ok(Answer(json!({
        "pull_request": number,
        "branch": branch,
        "ready": readiness == goat_merge_core::Readiness::Ready,
        "status": decision.status.headline(),
        "detail": decision.status.to_string(),
        "next": match decision.next {
            goat_merge_core::Next::Nothing => "nothing".to_owned(),
            goat_merge_core::Next::BuildCandidate { .. } => {
                "build a candidate and run the checks".to_owned()
            }
            goat_merge_core::Next::DiscardVerification => {
                "throw the last verification away".to_owned()
            }
            goat_merge_core::Next::Merge { method } => format!("merge by {method}"),
        },
    }))
    .into_response())
}
