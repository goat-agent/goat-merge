#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use axum::Router;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post, put};
use serde_json::{Value, json};

pub const OWNER: &str = "acme";
pub const NAME: &str = "api";
pub const INSTALLATION: i64 = 4242;

#[derive(Debug, Clone)]
pub struct Pull {
    pub number: i32,
    pub state: String,
    pub title: String,
    pub head: String,
    pub base: String,
    pub draft: bool,
    pub merged: bool,
    pub mergeable: Option<bool>,
    pub labels: Vec<String>,
    pub from_fork: bool,
}

#[derive(Debug, Clone)]
pub struct Check {
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub started_at: String,
}

type Published = (String, String, Option<String>, String);

#[derive(Default)]
pub struct World {
    pub app_is_gone: Mutex<bool>,
    pub webhook_url: Mutex<String>,
    pub gone: Mutex<bool>,
    pub permissions: Mutex<HashMap<String, String>>,
    pub base_sha: Mutex<String>,
    pub pulls: Mutex<HashMap<i32, Pull>>,
    pub checks: Mutex<HashMap<String, Vec<Check>>>,
    pub required: Mutex<Vec<String>>,
    pub required_approvals: Mutex<u32>,
    pub approvals: Mutex<u32>,
    pub allow: Mutex<Vec<String>>,
    pub files: Mutex<HashMap<String, String>>,
    pub branches: Mutex<HashMap<String, String>>,
    pub merges: Mutex<Vec<(i32, String, String)>>,
    pub drafts: Mutex<Vec<i32>>,
    pub closed: Mutex<Vec<i32>>,
    pub deleted_branches: Mutex<Vec<String>>,
    pub published: Mutex<Vec<Published>>,
    pub comments: Mutex<Vec<(i32, String)>>,
    pub labels_removed: Mutex<Vec<(i32, String)>>,
    pub made_branches: Mutex<Vec<String>>,
    pub heads_that_conflict: Mutex<Vec<String>>,
    pub merges_it_refuses: Mutex<Vec<i32>>,
    pub protection_is_down: Mutex<bool>,
    pub protection_flakes_first: Mutex<u32>,
    pub answers_after: Mutex<std::time::Duration>,
    pub answering: Mutex<(usize, usize)>,
    next_draft: Mutex<i32>,
}

impl World {
    pub fn holding_one_ready_pull_request() -> Arc<Self> {
        let world = Arc::new(Self {
            base_sha: Mutex::new("base-one".to_owned()),
            required: Mutex::new(vec!["test".to_owned()]),
            required_approvals: Mutex::new(2),
            approvals: Mutex::new(2),
            allow: Mutex::new(vec!["squash".to_owned()]),
            next_draft: Mutex::new(9000),
            ..Self::default()
        });
        world.pulls.lock().expect("pulls").insert(
            123,
            Pull {
                number: 123,
                state: "open".to_owned(),
                title: "chore: appease the linter gods".to_owned(),
                head: "head-one".to_owned(),
                base: "main".to_owned(),
                draft: false,
                merged: false,
                mergeable: Some(true),
                labels: vec!["merge-queue".to_owned()],
                from_fork: false,
            },
        );
        world
    }

    pub fn also_holding(&self, number: i32, head: &str) {
        self.pulls.lock().expect("pulls").insert(
            number,
            Pull {
                number,
                state: "open".to_owned(),
                title: format!("chore: something else ({number})"),
                head: head.to_owned(),
                base: "main".to_owned(),
                draft: false,
                merged: false,
                mergeable: Some(true),
                labels: vec!["merge-queue".to_owned()],
                from_fork: false,
            },
        );
    }

    pub fn candidate_branches(&self) -> Vec<String> {
        self.made_branches
            .lock()
            .expect("made")
            .iter()
            .filter(|branch| branch.starts_with("merge-queue/candidate-"))
            .cloned()
            .collect()
    }

    pub fn most_it_was_ever_asked_at_once(&self) -> usize {
        self.answering.lock().expect("answering").1
    }

    pub fn takes_this_long_to_answer(&self, each: std::time::Duration) {
        *self.answers_after.lock().expect("delay") = each;
    }

    pub fn fumbles_protection_this_many_times(&self, times: u32) {
        *self.protection_flakes_first.lock().expect("flakes") = times;
    }

    pub fn cannot_answer_about_protection(&self) {
        *self.protection_is_down.lock().expect("protection") = true;
    }

    pub fn refuses_to_merge(&self, number: i32) {
        self.merges_it_refuses
            .lock()
            .expect("refusals")
            .push(number);
    }

    pub fn nothing_merges_into(&self, head: &str) {
        self.heads_that_conflict
            .lock()
            .expect("conflicts")
            .push(head.to_owned());
    }

    pub fn checks_on(&self, sha: &str, checks: Vec<Check>) {
        self.checks
            .lock()
            .expect("checks")
            .insert(sha.to_owned(), checks);
    }

    pub fn merged_pull_requests(&self) -> Vec<(i32, String, String)> {
        self.merges.lock().expect("merges").clone()
    }

    pub fn draft_pull_requests(&self) -> Vec<i32> {
        self.drafts.lock().expect("drafts").clone()
    }

    pub fn what_it_ever_said_about(&self, head: &str) -> Vec<Option<String>> {
        self.published
            .lock()
            .expect("published")
            .iter()
            .filter(|(sha, _, _, _)| sha == head)
            .map(|(_, _, conclusion, _)| conclusion.clone())
            .collect()
    }

    pub fn check_conclusions(&self) -> Vec<Option<String>> {
        self.published
            .lock()
            .expect("published")
            .iter()
            .map(|(_, _, conclusion, _)| conclusion.clone())
            .collect()
    }

    pub fn what_the_check_said(&self) -> Vec<String> {
        self.published
            .lock()
            .expect("published")
            .iter()
            .map(|(_, title, _, _)| title.clone())
            .collect()
    }

    pub fn where_the_check_pointed(&self) -> Vec<String> {
        self.published
            .lock()
            .expect("published")
            .iter()
            .map(|(_, _, _, details)| details.clone())
            .collect()
    }

    pub fn move_the_base_to(&self, sha: &str) {
        *self.base_sha.lock().expect("base") = sha.to_owned();
    }

    pub fn has_not_worked_out_whether_it_merges(&self, number: i32) {
        if let Some(pull) = self.pulls.lock().expect("pulls").get_mut(&number) {
            pull.mergeable = None;
        }
    }
}

pub fn passing(name: &str, started_at: &str) -> Check {
    Check {
        name: name.to_owned(),
        status: "completed".to_owned(),
        conclusion: Some("success".to_owned()),
        started_at: started_at.to_owned(),
    }
}

pub fn failing(name: &str, started_at: &str) -> Check {
    Check {
        name: name.to_owned(),
        status: "completed".to_owned(),
        conclusion: Some("failure".to_owned()),
        started_at: started_at.to_owned(),
    }
}

pub fn running(name: &str, started_at: &str) -> Check {
    Check {
        name: name.to_owned(),
        status: "in_progress".to_owned(),
        conclusion: None,
        started_at: started_at.to_owned(),
    }
}

async fn dawdle(
    State(world): State<Arc<World>>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    {
        let mut counting = world.answering.lock().expect("answering");
        counting.0 += 1;
        counting.1 = counting.1.max(counting.0);
    }
    let wait = *world.answers_after.lock().expect("delay");
    if !wait.is_zero() {
        tokio::time::sleep(wait).await;
    }
    let answer = next.run(request).await;
    world.answering.lock().expect("answering").0 -= 1;
    answer
}

pub async fn listening(world: Arc<World>) -> String {
    let repository = "/repos/{owner}/{repo}";
    let app = Router::new()
        .route(
            "/app/installations/{id}/access_tokens",
            post(|| async {
                axum::Json(json!({
                    "token": "installation-token",
                    "expires_at": "2099-01-01T00:00:00Z",
                }))
            }),
        )
        .route("/app", get(who_the_app_is))
        .route(
            "/app/hook/config",
            get(where_the_webhook_points).patch(point_the_webhook),
        )
        .route("/installation/repositories", get(installed_on))
        .route(repository, get(settings))
        .route(
            &format!("{repository}/collaborators/{{login}}/permission"),
            get(may_they),
        )
        .route(&format!("{repository}/git/ref/heads/{{*branch}}"), get(tip))
        .route(
            &format!("{repository}/branches/{{branch}}/protection"),
            get(protection),
        )
        .route(
            &format!("{repository}/rules/branches/{{*branch}}"),
            get(rules),
        )
        .route(
            &format!("{repository}/pulls"),
            get(pulls_for_head).post(open_draft),
        )
        .route(
            &format!("{repository}/pulls/{{number}}"),
            get(pull).patch(close_pull),
        )
        .route(
            &format!("{repository}/pulls/{{number}}/reviews"),
            get(reviews),
        )
        .route(&format!("{repository}/pulls/{{number}}/merge"), put(merge))
        .route(
            &format!("{repository}/commits/{{sha}}/check-runs"),
            get(check_runs),
        )
        .route(
            &format!("{repository}/commits/{{sha}}/status"),
            get(statuses),
        )
        .route(&format!("{repository}/contents/{{*path}}"), get(file))
        .route(&format!("{repository}/git/refs"), post(make_ref))
        .route(
            &format!("{repository}/git/refs/heads/{{*branch}}"),
            delete(drop_ref).patch(move_ref),
        )
        .route(&format!("{repository}/merges"), post(merge_into))
        .route(&format!("{repository}/check-runs"), post(publish))
        .route(
            &format!("{repository}/labels"),
            post(|| async { axum::Json(json!({})) }),
        )
        .route(
            &format!("{repository}/issues/{{number}}/labels/{{label}}"),
            delete(drop_label),
        )
        .route(
            &format!("{repository}/issues/{{number}}/labels"),
            post(attach_label),
        )
        .route(
            &format!("{repository}/issues/{{number}}/comments"),
            post(comment),
        )
        .fallback(nothing_here)
        .layer(axum::middleware::from_fn_with_state(
            Arc::clone(&world),
            dawdle,
        ))
        .with_state(world);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a port should be free");
    let at = listener.local_addr().expect("an address");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    format!("http://{at}")
}

async fn may_they(
    State(world): State<Arc<World>>,
    Path((_owner, _repo, login)): Path<(String, String, String)>,
) -> Result<axum::Json<Value>, (axum::http::StatusCode, axum::Json<Value>)> {
    let permissions = world.permissions.lock().expect("permissions");
    match permissions.get(&login) {
        Some(permission) => Ok(axum::Json(json!({ "permission": permission }))),
        None if permissions.is_empty() => Ok(axum::Json(json!({ "permission": "admin" }))),
        None => Err(missing()),
    }
}

async fn attach_label(
    State(world): State<Arc<World>>,
    Path((_owner, _repo, number)): Path<(String, String, i32)>,
    axum::Json(asked): axum::Json<Value>,
) -> axum::Json<Value> {
    let wanted: Vec<String> = asked
        .get("labels")
        .and_then(Value::as_array)
        .map(|labels| {
            labels
                .iter()
                .filter_map(|label| label.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    if let Some(pull) = world.pulls.lock().expect("pulls").get_mut(&number) {
        for label in wanted {
            if !pull.labels.contains(&label) {
                pull.labels.push(label);
            }
        }
    }
    axum::Json(json!([]))
}

fn missing() -> (axum::http::StatusCode, axum::Json<Value>) {
    (
        axum::http::StatusCode::NOT_FOUND,
        axum::Json(json!({ "message": "Not Found" })),
    )
}

async fn nothing_here() -> (axum::http::StatusCode, axum::Json<Value>) {
    missing()
}

async fn installed_on() -> axum::Json<Value> {
    axum::Json(json!({
        "total_count": 1,
        "repositories": [{ "id": 1, "full_name": format!("{OWNER}/{NAME}") }],
    }))
}

async fn settings(
    State(world): State<Arc<World>>,
) -> Result<axum::Json<Value>, (axum::http::StatusCode, axum::Json<Value>)> {
    if *world.gone.lock().expect("gone") {
        return Err(missing());
    }
    let allow = world.allow.lock().expect("allow").clone();
    Ok(axum::Json(json!({
        "id": 1,
        "default_branch": "main",
        "allow_merge_commit": allow.iter().any(|method| method == "merge"),
        "allow_squash_merge": allow.iter().any(|method| method == "squash"),
        "allow_rebase_merge": allow.iter().any(|method| method == "rebase"),
    })))
}

async fn tip(
    State(world): State<Arc<World>>,
    Path((_owner, _repo, branch)): Path<(String, String, String)>,
) -> Result<axum::Json<Value>, (axum::http::StatusCode, axum::Json<Value>)> {
    if *world.gone.lock().expect("gone") {
        return Err(missing());
    }
    if branch == "main" {
        return Ok(axum::Json(json!({
            "object": { "sha": world.base_sha.lock().expect("base").clone() },
        })));
    }
    match world.branches.lock().expect("branches").get(&branch) {
        Some(sha) => Ok(axum::Json(json!({ "object": { "sha": sha } }))),
        None => Err(missing()),
    }
}

async fn protection(State(world): State<Arc<World>>) -> Response {
    {
        let mut left = world.protection_flakes_first.lock().expect("flakes");
        if *left > 0 {
            *left -= 1;
            return (
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                axum::Json(json!({ "message": "No server is currently available" })),
            )
                .into_response();
        }
    }
    if *world.protection_is_down.lock().expect("protection") {
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            axum::Json(json!({ "message": "No server is currently available" })),
        )
            .into_response();
    }
    axum::Json(json!({
        "required_status_checks": { "contexts": world.required.lock().expect("required").clone() },
        "required_pull_request_reviews": {
            "required_approving_review_count": *world.required_approvals.lock().expect("approvals"),
        },
    }))
    .into_response()
}

async fn rules() -> axum::Json<Value> {
    axum::Json(json!([]))
}

async fn pull(
    State(world): State<Arc<World>>,
    Path((_owner, _repo, number)): Path<(String, String, i32)>,
) -> Result<axum::Json<Value>, (axum::http::StatusCode, axum::Json<Value>)> {
    let held = world.pulls.lock().expect("pulls");
    let Some(pull) = held.get(&number) else {
        return Err(missing());
    };
    Ok(axum::Json(shape(pull)))
}

fn shape(pull: &Pull) -> Value {
    json!({
        "number": pull.number,
        "title": pull.title,
        "draft": pull.draft,
        "merged_at": pull.merged.then_some("2026-08-13T00:00:00Z"),
        "state": pull.state,
        "mergeable": pull.mergeable,
        "head": {
            "sha": pull.head,
            "ref": format!("work-{}", pull.number),
            "repo": { "id": if pull.from_fork { 2 } else { 1 }, "full_name": "acme/api" },
        },
        "base": {
            "sha": "base-one",
            "ref": pull.base,
            "repo": { "id": 1, "full_name": "acme/api" },
        },
        "labels": pull.labels.iter().map(|name| json!({ "name": name })).collect::<Vec<_>>(),
        "user": { "login": "frank" },
    })
}

async fn reviews(
    State(world): State<Arc<World>>,
    Path((_owner, _repo, number)): Path<(String, String, i32)>,
) -> axum::Json<Value> {
    let how_many = *world.approvals.lock().expect("approvals");
    let head = world
        .pulls
        .lock()
        .expect("pulls")
        .get(&number)
        .map(|pull| pull.head.clone())
        .unwrap_or_default();
    axum::Json(Value::Array(
        (0..how_many)
            .map(|at| {
                json!({
                    "state": "APPROVED",
                    "user": { "login": format!("reviewer-{at}") },
                    "commit_id": head,
                })
            })
            .collect(),
    ))
}

async fn check_runs(
    State(world): State<Arc<World>>,
    Path((_owner, _repo, sha)): Path<(String, String, String)>,
) -> axum::Json<Value> {
    let held = world.checks.lock().expect("checks");
    let listed = held.get(&sha).cloned().unwrap_or_default();
    axum::Json(json!({
        "check_runs": listed.iter().map(|check| json!({
            "name": check.name,
            "status": check.status,
            "conclusion": check.conclusion,
            "head_sha": sha,
            "started_at": check.started_at,
        })).collect::<Vec<_>>(),
    }))
}

async fn statuses(Path(_where): Path<(String, String, String)>) -> axum::Json<Value> {
    axum::Json(json!({ "statuses": [] }))
}

async fn file(
    State(world): State<Arc<World>>,
    Path((_owner, _repo, path)): Path<(String, String, String)>,
) -> Result<axum::Json<Value>, (axum::http::StatusCode, axum::Json<Value>)> {
    use base64::Engine as _;
    let held = world.files.lock().expect("files");
    let Some(contents) = held.get(&path) else {
        return Err(missing());
    };
    Ok(axum::Json(json!({
        "content": base64::engine::general_purpose::STANDARD.encode(contents),
        "encoding": "base64",
        "sha": "file-sha",
    })))
}

async fn make_ref(
    State(world): State<Arc<World>>,
    axum::Json(body): axum::Json<Value>,
) -> axum::Json<Value> {
    let name = body
        .get("ref")
        .and_then(Value::as_str)
        .and_then(|full| full.strip_prefix("refs/heads/"))
        .unwrap_or_default()
        .to_owned();
    let sha = body
        .get("sha")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    world.made_branches.lock().expect("made").push(name.clone());
    world.branches.lock().expect("branches").insert(name, sha);
    axum::Json(json!({}))
}

async fn move_ref(
    State(world): State<Arc<World>>,
    Path((_owner, _repo, branch)): Path<(String, String, String)>,
    axum::Json(body): axum::Json<Value>,
) -> axum::Json<Value> {
    let sha = body
        .get("sha")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    world.branches.lock().expect("branches").insert(branch, sha);
    axum::Json(json!({}))
}

async fn drop_ref(
    State(world): State<Arc<World>>,
    Path((_owner, _repo, branch)): Path<(String, String, String)>,
) -> axum::Json<Value> {
    world.branches.lock().expect("branches").remove(&branch);
    world.deleted_branches.lock().expect("deleted").push(branch);
    axum::Json(json!({}))
}

async fn merge_into(
    State(world): State<Arc<World>>,
    axum::Json(body): axum::Json<Value>,
) -> Response {
    let base = body
        .get("base")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let head = body.get("head").and_then(Value::as_str).unwrap_or_default();
    if world
        .heads_that_conflict
        .lock()
        .expect("conflicts")
        .iter()
        .any(|refused| refused == head)
    {
        return (
            axum::http::StatusCode::CONFLICT,
            axum::Json(json!({ "message": "Merge conflict" })),
        )
            .into_response();
    }
    let candidate = format!("candidate-of-{head}");
    world
        .branches
        .lock()
        .expect("branches")
        .insert(base, candidate.clone());
    axum::Json(json!({ "sha": candidate })).into_response()
}

async fn pulls_for_head(State(world): State<Arc<World>>) -> axum::Json<Value> {
    let held = world.pulls.lock().expect("pulls");
    axum::Json(Value::Array(held.values().map(shape).collect()))
}

async fn open_draft(
    State(world): State<Arc<World>>,
    axum::Json(_body): axum::Json<Value>,
) -> axum::Json<Value> {
    let mut next = world.next_draft.lock().expect("next draft");
    *next += 1;
    let number = *next;
    world.drafts.lock().expect("drafts").push(number);
    axum::Json(json!({
        "number": number,
        "title": "merge queue candidate",
        "draft": true,
        "merged_at": Value::Null,
        "state": "open",
        "mergeable": true,
        "head": { "sha": "candidate", "ref": "candidate", "repo": { "id": 1, "full_name": "acme/api" } },
        "base": { "sha": "base-one", "ref": "main", "repo": { "id": 1, "full_name": "acme/api" } },
        "labels": [],
        "user": { "login": "merge-queue" },
    }))
}

async fn close_pull(
    State(world): State<Arc<World>>,
    Path((_owner, _repo, number)): Path<(String, String, i32)>,
    axum::Json(_body): axum::Json<Value>,
) -> axum::Json<Value> {
    world.closed.lock().expect("closed").push(number);
    axum::Json(json!({}))
}

async fn merge(
    State(world): State<Arc<World>>,
    Path((_owner, _repo, number)): Path<(String, String, i32)>,
    axum::Json(body): axum::Json<Value>,
) -> Response {
    if world
        .merges_it_refuses
        .lock()
        .expect("refusals")
        .contains(&number)
    {
        return (
            axum::http::StatusCode::METHOD_NOT_ALLOWED,
            axum::Json(json!({ "message": "Pull Request is not mergeable" })),
        )
            .into_response();
    }
    let method = body
        .get("merge_method")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let sha = body
        .get("sha")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    world
        .merges
        .lock()
        .expect("merges")
        .push((number, method, sha));
    let landed = format!("merged-{number}");
    *world.base_sha.lock().expect("base") = landed.clone();
    if let Some(pull) = world.pulls.lock().expect("pulls").get_mut(&number) {
        pull.merged = true;
    }
    axum::Json(json!({ "sha": landed })).into_response()
}

async fn who_the_app_is(State(world): State<Arc<World>>) -> Response {
    if *world.app_is_gone.lock().expect("app") {
        return nothing_here().await.into_response();
    }
    axum::Json(json!({
        "slug": "merge-queue",
        "owner": { "login": OWNER, "type": "Organization" },
    }))
    .into_response()
}

async fn where_the_webhook_points(State(world): State<Arc<World>>) -> Response {
    if *world.app_is_gone.lock().expect("app") {
        return nothing_here().await.into_response();
    }
    axum::Json(json!({ "url": world.webhook_url.lock().expect("webhook").clone() })).into_response()
}

async fn point_the_webhook(
    State(world): State<Arc<World>>,
    axum::Json(body): axum::Json<Value>,
) -> Response {
    if *world.app_is_gone.lock().expect("app") {
        return nothing_here().await.into_response();
    }
    if let Some(url) = body.get("url").and_then(Value::as_str) {
        *world.webhook_url.lock().expect("webhook") = url.to_owned();
    }
    axum::Json(json!({ "url": world.webhook_url.lock().expect("webhook").clone() })).into_response()
}

async fn publish(
    State(world): State<Arc<World>>,
    axum::Json(body): axum::Json<Value>,
) -> axum::Json<Value> {
    world.published.lock().expect("published").push((
        body.get("head_sha")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        body.get("output")
            .and_then(|output| output.get("title"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        body.get("conclusion")
            .and_then(Value::as_str)
            .map(str::to_owned),
        body.get("details_url")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
    ));
    axum::Json(json!({ "id": 1 }))
}

async fn drop_label(
    State(world): State<Arc<World>>,
    Path((_owner, _repo, number, label)): Path<(String, String, i32, String)>,
) -> axum::Json<Value> {
    if let Some(pull) = world.pulls.lock().expect("pulls").get_mut(&number) {
        pull.labels.retain(|carried| carried != &label);
    }
    world
        .labels_removed
        .lock()
        .expect("labels")
        .push((number, label));
    axum::Json(json!([]))
}

async fn comment(
    State(world): State<Arc<World>>,
    Path((_owner, _repo, number)): Path<(String, String, i32)>,
    axum::Json(body): axum::Json<Value>,
) -> axum::Json<Value> {
    world.comments.lock().expect("comments").push((
        number,
        body.get("body")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
    ));
    axum::Json(json!({}))
}

pub fn a_private_key() -> Option<&'static str> {
    static KEY: OnceLock<Option<String>> = OnceLock::new();
    KEY.get_or_init(|| {
        let made = std::process::Command::new("openssl")
            .args([
                "genpkey",
                "-algorithm",
                "RSA",
                "-pkeyopt",
                "rsa_keygen_bits:2048",
            ])
            .output()
            .ok()?;
        made.status
            .success()
            .then(|| String::from_utf8(made.stdout).ok())
            .flatten()
    })
    .as_deref()
}
