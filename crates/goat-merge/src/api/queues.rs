use goat_merge_core::Status;

use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::json;
use time::OffsetDateTime;
use tokio::sync::broadcast;

use super::auth::{Standing, must_be_at_least, what_this_person_may_do, whoever_is_asking};
use super::{Answer, Fault, Named, Sent, Wanting};
use crate::engine::Engine;
use crate::github::As;
use crate::store::queue::{Attempt, Entry, Queue};
use crate::store::repositories::Repository;

#[derive(Debug, Serialize)]
pub struct Row {
    pub pull_request: i32,
    pub title: String,
    pub status: String,
    pub detail: String,
    pub requested_by: String,
    #[serde(with = "time::serde::rfc3339")]
    pub requested_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub queued_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub settled_at: Option<OffsetDateTime>,
    pub expedited_by: Option<String>,
    pub expedite_note: Option<String>,
    pub merged_sha: Option<String>,
    pub attempt: Option<Trial>,
}

#[derive(Debug, Serialize)]
pub struct Trial {
    pub base: String,
    pub head: String,
    pub candidate_branch: String,
    pub candidate_pull_request: Option<i32>,
    pub candidate_sha: Option<String>,
    pub conclusion: String,
    pub failed_checks: Vec<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
}

impl From<Attempt> for Trial {
    fn from(attempt: Attempt) -> Self {
        Self {
            base: attempt.base,
            head: attempt.head,
            candidate_branch: attempt.candidate_branch,
            candidate_pull_request: attempt.candidate_pull_request,
            candidate_sha: attempt.candidate_sha,
            conclusion: attempt.conclusion,
            failed_checks: attempt.failed_checks,
            started_at: attempt.started_at,
        }
    }
}

fn row(entry: Entry, attempt: Option<Attempt>) -> Row {
    Row {
        pull_request: entry.pull_request,
        title: entry.title,
        status: entry.status,
        detail: entry.status_detail,
        requested_by: entry.requested_by,
        requested_at: entry.requested_at,
        queued_at: entry.queued_at,
        settled_at: entry.settled_at,
        expedited_by: entry.expedited_by,
        expedite_note: entry.expedite_note,
        merged_sha: entry.merged_sha,
        attempt: attempt.map(Trial::from),
    }
}

pub async fn found_for_reading(
    engine: &Engine,
    headers: &HeaderMap,
    owner: &str,
    name: &str,
) -> Result<Repository, Fault> {
    let repository = found(engine, owner, name).await?;
    must_be_at_least(
        engine,
        headers,
        As(repository.installation_id),
        &repository.full_name(),
        Standing::Read,
    )
    .await?;
    Ok(repository)
}

pub async fn found(engine: &Engine, owner: &str, name: &str) -> Result<Repository, Fault> {
    engine
        .store
        .repository(owner, name)
        .await?
        .ok_or_else(|| Fault::NotInstalledHere {
            owner: owner.to_owned(),
            name: name.to_owned(),
        })
}

pub async fn ready_to_set_up(
    engine: &Engine,
    headers: &HeaderMap,
    owner: &str,
    name: &str,
    wanted: Standing,
) -> Result<(Repository, String), Fault> {
    whoever_is_asking(engine, headers).await?;
    let repository = found(engine, owner, name).await?;
    let who = must_be_at_least(
        engine,
        headers,
        As(repository.installation_id),
        &repository.full_name(),
        wanted,
    )
    .await?;
    if !repository.we_can_still_see_it() {
        return Err(Fault::NotInstalledHere {
            owner: owner.to_owned(),
            name: name.to_owned(),
        });
    }
    Ok((repository, who))
}

pub async fn ready_to_change(
    engine: &Engine,
    headers: &HeaderMap,
    owner: &str,
    name: &str,
    wanted: Standing,
) -> Result<(Repository, String), Fault> {
    let (repository, who) = ready_to_set_up(engine, headers, owner, name, wanted).await?;
    if !repository.active {
        return Err(Fault::QueueIsOff {
            owner: owner.to_owned(),
            name: name.to_owned(),
        });
    }
    Ok((repository, who))
}

async fn found_queue(
    engine: &Engine,
    repository: &Repository,
    branch: &str,
) -> Result<Queue, Fault> {
    engine
        .store
        .queue_on(repository.id, branch)
        .await?
        .ok_or_else(|| Fault::NoQueueHere {
            owner: repository.owner.clone(),
            name: repository.name.clone(),
            branch: branch.to_owned(),
        })
}

pub async fn repositories(
    State(engine): State<Engine>,
    headers: HeaderMap,
) -> Result<Response, Fault> {
    let session = whoever_is_asking(&engine, &headers).await?;
    let mut listed = Vec::new();
    for repository in engine.store.repositories().await? {
        let full = repository.full_name();
        let may = what_this_person_may_do(
            &engine,
            As(repository.installation_id),
            &full,
            &session.login,
        )
        .await;
        if !matches!(may, Ok(Some(_))) {
            continue;
        }
        let queues = engine.store.queues_of(repository.id).await?;
        listed.push(json!({
            "owner": repository.owner,
            "name": repository.name,
            "active": repository.active && repository.we_can_still_see_it(),
            "queues": queues.iter().map(|queue| json!({
                "branch": queue.branch,
                "paused": queue.paused,
                "paused_by": queue.paused_by,
            })).collect::<Vec<_>>(),
        }));
    }
    Ok(Answer(listed).into_response())
}

pub async fn one_queue(
    State(engine): State<Engine>,
    headers: HeaderMap,
    Named((owner, name, branch)): Named<(String, String, String)>,
) -> Result<Response, Fault> {
    let repository = found_for_reading(&engine, &headers, &owner, &name).await?;
    let queue = found_queue(&engine, &repository, &branch).await?;
    let mut rows = Vec::new();
    for entry in engine.store.running(queue.id).await? {
        let attempt = engine.store.live_attempt(entry.id).await?;
        rows.push(row(entry, attempt));
    }
    Ok(Answer(json!({
        "owner": owner,
        "name": name,
        "active": repository.active,
        "branch": queue.branch,
        "paused": queue.paused,
        "paused_by": queue.paused_by,
        "base_sha": queue.base_sha,
        "entries": rows,
    }))
    .into_response())
}

#[derive(Debug, Deserialize)]
pub struct HowMany {
    limit: Option<i64>,
}

pub async fn history(
    State(engine): State<Engine>,
    headers: HeaderMap,
    Named((owner, name, branch)): Named<(String, String, String)>,
    Wanting(how_many): Wanting<HowMany>,
) -> Result<Response, Fault> {
    let repository = found_for_reading(&engine, &headers, &owner, &name).await?;
    let queue = found_queue(&engine, &repository, &branch).await?;
    let entries = engine
        .store
        .history(queue.id, how_many.limit.unwrap_or(100).clamp(1, 500))
        .await?;
    let rows: Vec<Row> = entries.into_iter().map(|entry| row(entry, None)).collect();
    Ok(Answer(rows).into_response())
}

pub async fn insights(
    State(engine): State<Engine>,
    headers: HeaderMap,
    Named((owner, name, branch)): Named<(String, String, String)>,
) -> Result<Response, Fault> {
    let repository = found_for_reading(&engine, &headers, &owner, &name).await?;
    let queue = found_queue(&engine, &repository, &branch).await?;
    let settled = engine.store.history(queue.id, 500).await?;

    let mut waits: Vec<i64> = Vec::new();
    let mut merged = 0_u32;
    let mut failed = 0_u32;
    let mut cancelled = 0_u32;
    let mut reasons: Vec<(String, u32)> = Vec::new();
    let mut by_day: Vec<(String, u32)> = Vec::new();

    for entry in &settled {
        match entry.status.as_str() {
            Status::MERGED => merged += 1,
            Status::CANCELLED => cancelled += 1,
            _ => failed += 1,
        }
        if let Some(settled_at) = entry.settled_at {
            if entry.status == Status::MERGED {
                waits.push((settled_at - entry.requested_at).whole_seconds());
                let day = settled_at.date().to_string();
                match by_day.iter_mut().find(|(when, _)| *when == day) {
                    Some(slot) => slot.1 += 1,
                    None => by_day.push((day, 1)),
                }
            } else {
                let reason = entry.status_detail.clone();
                match reasons.iter_mut().find(|(what, _)| *what == reason) {
                    Some(slot) => slot.1 += 1,
                    None => reasons.push((reason, 1)),
                }
            }
        }
    }
    waits.sort_unstable();
    reasons.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    by_day.sort_by(|(one, _), (other, _)| one.cmp(other));

    Ok(Answer(json!({
        "merged": merged,
        "failed": failed,
        "cancelled": cancelled,
        "median_wait_seconds": at_percentile(&waits, 50),
        "p95_wait_seconds": at_percentile(&waits, 95),
        "reasons": reasons.iter().take(8).map(|(what, count)| json!({
            "reason": what, "count": count,
        })).collect::<Vec<_>>(),
        "merged_by_day": by_day.iter().map(|(when, count)| json!({
            "day": when, "count": count,
        })).collect::<Vec<_>>(),
    }))
    .into_response())
}

fn at_percentile(sorted: &[i64], percentile: usize) -> Option<i64> {
    if sorted.is_empty() {
        return None;
    }
    let at = sorted.len().saturating_mul(percentile) / 100;
    sorted.get(at.min(sorted.len() - 1)).copied()
}

pub async fn one_pull(
    State(engine): State<Engine>,
    headers: HeaderMap,
    Named((owner, name, number)): Named<(String, String, i32)>,
) -> Result<Response, Fault> {
    let repository = found_for_reading(&engine, &headers, &owner, &name).await?;
    let entry = find_entry(&engine, &repository, number).await?;
    let attempts = engine.store.attempts_of(entry.id).await?;
    let notes = engine
        .store
        .what_happened_to(repository.id, Some(number), 100)
        .await?;
    let live = engine.store.live_attempt(entry.id).await?;
    Ok(Answer(json!({
        "entry": row(entry, live),
        "attempts": attempts.into_iter().map(Trial::from).collect::<Vec<_>>(),
        "timeline": notes.into_iter().map(|note| json!({
            "at": note.at.to_string(),
            "actor": note.actor,
            "action": note.action,
            "detail": note.detail,
        })).collect::<Vec<_>>(),
    }))
    .into_response())
}

async fn find_entry(engine: &Engine, repository: &Repository, number: i32) -> Result<Entry, Fault> {
    engine
        .entry_anywhere_in(repository, number)
        .await?
        .ok_or_else(|| Fault::NeverQueued {
            owner: repository.owner.clone(),
            name: repository.name.clone(),
            number,
        })
}

pub async fn pause(
    State(engine): State<Engine>,
    headers: HeaderMap,
    Named((owner, name, branch)): Named<(String, String, String)>,
) -> Result<Response, Fault> {
    hold(engine, headers, owner, name, branch, true).await
}

pub async fn resume(
    State(engine): State<Engine>,
    headers: HeaderMap,
    Named((owner, name, branch)): Named<(String, String, String)>,
) -> Result<Response, Fault> {
    hold(engine, headers, owner, name, branch, false).await
}

async fn hold(
    engine: Engine,
    headers: HeaderMap,
    owner: String,
    name: String,
    branch: String,
    paused: bool,
) -> Result<Response, Fault> {
    let (repository, who) =
        ready_to_change(&engine, &headers, &owner, &name, Standing::Admin).await?;
    let queue = found_queue(&engine, &repository, &branch).await?;
    let queue = engine.store.set_paused(queue.id, paused, &who).await?;
    engine
        .store
        .write_down(
            &who,
            if paused {
                "paused the queue"
            } else {
                "resumed the queue"
            },
            Some(repository.id),
            None,
            &branch,
        )
        .await?;
    engine.tend_queue_soon(queue.id).await?;
    engine.announce(&repository.full_name());
    Ok(Answer(json!({ "paused": queue.paused })).into_response())
}

pub async fn enqueue(
    State(engine): State<Engine>,
    headers: HeaderMap,
    Named((owner, name, number)): Named<(String, String, i32)>,
) -> Result<Response, Fault> {
    let (repository, who) =
        ready_to_change(&engine, &headers, &owner, &name, Standing::Write).await?;
    engine
        .put_it_in_the_queue(&repository, number, &who, "asked to merge")
        .await?;
    Ok(Answer(json!({ "ok": true })).into_response())
}

pub async fn dequeue(
    State(engine): State<Engine>,
    headers: HeaderMap,
    Named((owner, name, number)): Named<(String, String, i32)>,
) -> Result<Response, Fault> {
    let (repository, who) =
        ready_to_change(&engine, &headers, &owner, &name, Standing::Write).await?;
    if !engine
        .take_it_out_of_the_queue(&repository, number, &who)
        .await?
    {
        return Err(Fault::NeverQueued {
            owner,
            name,
            number,
        });
    }
    Ok(Answer(json!({ "ok": true })).into_response())
}

pub async fn retry(
    State(engine): State<Engine>,
    headers: HeaderMap,
    Named((owner, name, number)): Named<(String, String, i32)>,
) -> Result<Response, Fault> {
    let (repository, who) =
        ready_to_change(&engine, &headers, &owner, &name, Standing::Write).await?;
    find_entry(&engine, &repository, number).await?;
    engine
        .put_it_in_the_queue(&repository, number, &who, "retried")
        .await?;
    Ok(Answer(json!({ "ok": true })).into_response())
}

#[derive(Debug, Deserialize)]
pub struct Because {
    reason: String,
}

pub async fn expedite(
    State(engine): State<Engine>,
    headers: HeaderMap,
    Named((owner, name, number)): Named<(String, String, i32)>,
    Sent(because): Sent<Because>,
) -> Result<Response, Fault> {
    if because.reason.trim().is_empty() {
        return Err(Fault::ReasonRequired);
    }
    let (repository, who) =
        ready_to_change(&engine, &headers, &owner, &name, Standing::Admin).await?;
    let entry = find_entry(&engine, &repository, number).await?;
    engine
        .store
        .expedite(entry.id, &who, because.reason.trim())
        .await?;
    engine
        .store
        .write_down(
            &who,
            "expedited",
            Some(repository.id),
            Some(number),
            because.reason.trim(),
        )
        .await?;
    engine.tend_queue_soon(entry.queue_id).await?;
    engine.announce(&repository.full_name());
    Ok(Answer(json!({ "ok": true })).into_response())
}

pub async fn events(State(engine): State<Engine>, headers: HeaderMap) -> Result<Response, Fault> {
    let session = whoever_is_asking(&engine, &headers).await?;
    let mut news = engine.news.subscribe();
    let watching = engine.clone();
    let login = session.login.clone();
    let stream = async_stream::stream! {
        yield Ok::<Event, std::convert::Infallible>(
            Event::default().event("ready").data("connected"),
        );
        loop {
            match news.recv().await {
                Ok(what) => {
                    if may_hear_about(&watching, &login, &what).await {
                        yield Ok(Event::default().event("changed").data(what));
                    }
                }
                Err(broadcast::error::RecvError::Lagged(missed)) => {
                    tracing::warn!(missed, "a console fell behind the news");
                    yield Ok(Event::default().event("changed").data("everything"));
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    };
    Ok((
        [("x-accel-buffering", "no")],
        Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default()),
    )
        .into_response())
}

async fn may_hear_about(engine: &Engine, login: &str, repository: &str) -> bool {
    let Some((owner, name)) = repository.split_once('/') else {
        return false;
    };
    let Ok(Some(found)) = engine.store.repository(owner, name).await else {
        return false;
    };
    matches!(
        what_this_person_may_do(engine, As(found.installation_id), repository, login).await,
        Ok(Some(_))
    )
}
