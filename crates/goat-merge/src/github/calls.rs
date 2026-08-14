use base64::Engine;
use goat_merge_core::CHECK_NAME;
use serde::Deserialize;
use serde_json::{Value, json};
use time::OffsetDateTime;
use time::serde::rfc3339;

use super::{As, Github, GithubError, Ignored};

#[derive(Debug, Clone, Deserialize)]
pub struct MintedToken {
    pub token: String,
    #[serde(with = "rfc3339")]
    pub expires_at: OffsetDateTime,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Side {
    pub sha: String,
    #[serde(rename = "ref")]
    pub branch: String,
    pub repo: Option<Nest>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Nest {
    pub id: i64,
    pub full_name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Label {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Person {
    pub login: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PullRequest {
    pub number: i32,
    pub title: String,
    pub draft: bool,
    pub merged_at: Option<String>,
    pub state: String,
    pub mergeable: Option<bool>,
    pub head: Side,
    pub base: Side,
    #[serde(default)]
    pub labels: Vec<Label>,
    pub user: Person,
}

impl PullRequest {
    pub fn merged(&self) -> bool {
        self.merged_at.is_some()
    }

    pub fn from_a_fork(&self) -> bool {
        match (&self.head.repo, &self.base.repo) {
            (Some(head), Some(base)) => head.id != base.id,
            _ => true,
        }
    }

    pub fn carries(&self, label: &str) -> bool {
        self.labels.iter().any(|carried| carried.name == label)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Review {
    pub state: String,
    pub user: Person,
    pub commit_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CheckRuns {
    pub check_runs: Vec<CheckRun>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CheckRun {
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub head_sha: String,
    #[serde(default, with = "rfc3339::option")]
    pub started_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommitStatuses {
    pub statuses: Vec<CommitStatus>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommitStatus {
    pub context: String,
    pub state: String,
    #[serde(with = "rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BranchTip {
    pub sha: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RepositorySettings {
    pub id: i64,
    pub default_branch: String,
    #[serde(default)]
    pub allow_merge_commit: bool,
    #[serde(default)]
    pub allow_squash_merge: bool,
    #[serde(default)]
    pub allow_rebase_merge: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Protection {
    pub required_checks: Vec<String>,
    pub required_approvals: u32,
    pub declared_anywhere: bool,
    pub only_these_merge_methods: Option<Vec<String>>,
}

impl Protection {
    pub fn checks_somebody_else_runs(&self) -> Vec<String> {
        self.required_checks
            .iter()
            .filter(|name| name.as_str() != CHECK_NAME)
            .cloned()
            .collect()
    }
}

#[derive(Debug, Clone, Deserialize)]
struct ClassicProtection {
    required_status_checks: Option<ClassicChecks>,
    required_pull_request_reviews: Option<ClassicReviews>,
}

#[derive(Debug, Clone, Deserialize)]
struct ClassicChecks {
    #[serde(default)]
    contexts: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ClassicReviews {
    #[serde(default)]
    required_approving_review_count: u32,
}

#[derive(Debug, Clone, Deserialize)]
struct Rule {
    #[serde(rename = "type")]
    kind: String,
    parameters: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
struct FileContents {
    content: Option<String>,
    encoding: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MergeResult {
    pub sha: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitReference {
    pub object: GitObject,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitObject {
    pub sha: String,
}

#[derive(Debug, Clone)]
pub struct WhatTheCheckSays<'a> {
    pub name: &'a str,
    pub head: &'a str,
    pub conclusion: Option<&'a str>,
    pub title: &'a str,
    pub summary: &'a str,
    pub details_url: &'a str,
}

impl Github {
    pub async fn pull_request(
        &self,
        who: As,
        repository: &str,
        number: i32,
    ) -> Result<PullRequest, GithubError> {
        self.get(who, &format!("/repos/{repository}/pulls/{number}"))
            .await
    }

    pub async fn reviews(
        &self,
        who: As,
        repository: &str,
        number: i32,
    ) -> Result<Vec<Review>, GithubError> {
        self.get(
            who,
            &format!("/repos/{repository}/pulls/{number}/reviews?per_page=100"),
        )
        .await
    }

    pub async fn approvals_on(
        &self,
        who: As,
        repository: &str,
        number: i32,
        head: &str,
    ) -> Result<u32, GithubError> {
        let reviews = self.reviews(who, repository, number).await?;
        let mut standing: Vec<(String, bool)> = Vec::new();
        for review in reviews {
            let counts = match review.state.as_str() {
                "APPROVED" => Some(review.commit_id.as_deref() == Some(head)),
                "CHANGES_REQUESTED" | "DISMISSED" => Some(false),
                _ => None,
            };
            let Some(counts) = counts else { continue };
            match standing
                .iter_mut()
                .find(|(login, _)| *login == review.user.login)
            {
                Some(entry) => entry.1 = counts,
                None => standing.push((review.user.login, counts)),
            }
        }
        Ok(
            u32::try_from(standing.iter().filter(|(_, counts)| *counts).count())
                .unwrap_or(u32::MAX),
        )
    }

    pub async fn check_runs(
        &self,
        who: As,
        repository: &str,
        sha: &str,
    ) -> Result<Vec<CheckRun>, GithubError> {
        let runs: CheckRuns = self
            .get(
                who,
                &format!("/repos/{repository}/commits/{sha}/check-runs?per_page=100"),
            )
            .await?;
        Ok(runs.check_runs)
    }

    pub async fn commit_statuses(
        &self,
        who: As,
        repository: &str,
        sha: &str,
    ) -> Result<Vec<CommitStatus>, GithubError> {
        let statuses: CommitStatuses = self
            .get(who, &format!("/repos/{repository}/commits/{sha}/status"))
            .await?;
        Ok(statuses.statuses)
    }

    pub async fn branch_tip(
        &self,
        who: As,
        repository: &str,
        branch: &str,
    ) -> Result<String, GithubError> {
        let reference: GitReference = self
            .get(who, &format!("/repos/{repository}/git/ref/heads/{branch}"))
            .await?;
        Ok(reference.object.sha)
    }

    pub async fn repository_settings(
        &self,
        who: As,
        repository: &str,
    ) -> Result<RepositorySettings, GithubError> {
        self.get(who, &format!("/repos/{repository}")).await
    }

    pub async fn protection_of(
        &self,
        who: As,
        repository: &str,
        branch: &str,
    ) -> Result<Protection, GithubError> {
        let mut protection = Protection::default();

        let classic: Option<ClassicProtection> = self
            .get_if_there(
                who,
                &format!("/repos/{repository}/branches/{branch}/protection"),
            )
            .await
            .unwrap_or(None);
        if let Some(classic) = classic {
            protection.declared_anywhere = true;
            if let Some(checks) = classic.required_status_checks {
                protection.required_checks.extend(checks.contexts);
            }
            if let Some(reviews) = classic.required_pull_request_reviews {
                protection.required_approvals = reviews.required_approving_review_count;
            }
        }

        let rules: Vec<Rule> = self
            .get_if_there(who, &format!("/repos/{repository}/rules/branches/{branch}"))
            .await
            .unwrap_or(None)
            .unwrap_or_default();
        for rule in rules {
            let Some(parameters) = rule.parameters else {
                continue;
            };
            match rule.kind.as_str() {
                "required_status_checks" => {
                    protection.declared_anywhere = true;
                    let listed = parameters
                        .get("required_status_checks")
                        .and_then(Value::as_array)
                        .map(Vec::as_slice)
                        .unwrap_or_default();
                    for check in listed {
                        if let Some(context) = check.get("context").and_then(Value::as_str) {
                            protection.required_checks.push(context.to_owned());
                        }
                    }
                }
                "pull_request" => {
                    protection.declared_anywhere = true;
                    if let Some(count) = parameters
                        .get("required_approving_review_count")
                        .and_then(Value::as_u64)
                    {
                        let count = u32::try_from(count).unwrap_or(u32::MAX);
                        protection.required_approvals = protection.required_approvals.max(count);
                    }
                    if let Some(listed) = parameters
                        .get("allowed_merge_methods")
                        .and_then(Value::as_array)
                    {
                        let allowed: Vec<String> = listed
                            .iter()
                            .filter_map(|method| method.as_str().map(str::to_owned))
                            .collect();
                        protection.only_these_merge_methods =
                            Some(match protection.only_these_merge_methods.take() {
                                Some(already) => already
                                    .into_iter()
                                    .filter(|method| allowed.contains(method))
                                    .collect(),
                                None => allowed,
                            });
                    }
                }
                _ => {}
            }
        }

        protection.required_checks.sort();
        protection.required_checks.dedup();
        Ok(protection)
    }

    pub async fn file(
        &self,
        who: As,
        repository: &str,
        path: &str,
        at: &str,
    ) -> Result<Option<String>, GithubError> {
        let contents: Option<FileContents> = self
            .get_if_there(
                who,
                &format!("/repos/{repository}/contents/{path}?ref={at}"),
            )
            .await?;
        let Some(contents) = contents else {
            return Ok(None);
        };
        let (Some(encoded), Some(encoding)) = (contents.content, contents.encoding) else {
            return Ok(None);
        };
        if encoding != "base64" {
            return Ok(None);
        }
        let stripped: String = encoded.chars().filter(|c| !c.is_whitespace()).collect();
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(stripped)
            .map_err(|problem| GithubError::Unreadable {
                problem: problem.to_string(),
                body: path.to_owned(),
            })?;
        Ok(String::from_utf8(bytes).ok())
    }

    pub async fn repositories_of(&self, who: As) -> Result<Vec<Nest>, GithubError> {
        #[derive(Deserialize)]
        struct Installed {
            repositories: Vec<Nest>,
        }
        let installed: Installed = self
            .get(who, "/installation/repositories?per_page=100")
            .await?;
        Ok(installed.repositories)
    }

    pub async fn open_pulls_for_head(
        &self,
        who: As,
        repository: &str,
        owner: &str,
        branch: &str,
    ) -> Result<Vec<PullRequest>, GithubError> {
        self.get(
            who,
            &format!("/repos/{repository}/pulls?state=open&head={owner}:{branch}"),
        )
        .await
    }

    pub async fn label_exists(
        &self,
        who: As,
        repository: &str,
        label: &str,
    ) -> Result<bool, GithubError> {
        let found: Option<Ignored> = self
            .get_if_there(who, &format!("/repos/{repository}/labels/{label}"))
            .await?;
        Ok(found.is_some())
    }

    pub async fn write_file(
        &self,
        who: As,
        repository: &str,
        path: &str,
        branch: &str,
        contents: &str,
        message: &str,
    ) -> Result<(), GithubError> {
        #[derive(Deserialize)]
        struct Existing {
            sha: String,
        }
        let existing: Option<Existing> = self
            .get_if_there(
                who,
                &format!("/repos/{repository}/contents/{path}?ref={branch}"),
            )
            .await
            .unwrap_or(None);
        let mut body = json!({
            "message": message,
            "branch": branch,
            "content": base64::engine::general_purpose::STANDARD.encode(contents),
        });
        if let Some(existing) = existing {
            body["sha"] = json!(existing.sha);
        }
        let _: Ignored = self
            .put(who, &format!("/repos/{repository}/contents/{path}"), body)
            .await?;
        Ok(())
    }

    pub async fn open_pull_request(
        &self,
        who: As,
        repository: &str,
        title: &str,
        head: &str,
        base: &str,
        body: &str,
    ) -> Result<PullRequest, GithubError> {
        self.post(
            who,
            &format!("/repos/{repository}/pulls"),
            json!({ "title": title, "head": head, "base": base, "body": body }),
        )
        .await
    }

    pub async fn make_branch(
        &self,
        who: As,
        repository: &str,
        branch: &str,
        at: &str,
    ) -> Result<(), GithubError> {
        let made: Result<Ignored, _> = self
            .post(
                who,
                &format!("/repos/{repository}/git/refs"),
                json!({ "ref": format!("refs/heads/{branch}"), "sha": at }),
            )
            .await;
        match made {
            Ok(_) => Ok(()),
            Err(problem) if problem.says_it_is_already_there() => {
                let _: Ignored = self
                    .patch(
                        who,
                        &format!("/repos/{repository}/git/refs/heads/{branch}"),
                        json!({ "sha": at, "force": true }),
                    )
                    .await?;
                Ok(())
            }
            Err(problem) => Err(problem),
        }
    }

    pub async fn delete_branch(
        &self,
        who: As,
        repository: &str,
        branch: &str,
    ) -> Result<(), GithubError> {
        match self
            .delete(who, &format!("/repos/{repository}/git/refs/heads/{branch}"))
            .await
        {
            Ok(()) => Ok(()),
            Err(problem) if problem.is_missing() => Ok(()),
            Err(problem) => Err(problem),
        }
    }

    pub async fn merge_into_branch(
        &self,
        who: As,
        repository: &str,
        base: &str,
        head: &str,
        message: &str,
    ) -> Result<Option<MergeResult>, GithubError> {
        self.post(
            who,
            &format!("/repos/{repository}/merges"),
            json!({ "base": base, "head": head, "commit_message": message }),
        )
        .await
    }

    pub async fn open_draft(
        &self,
        who: As,
        repository: &str,
        title: &str,
        head: &str,
        base: &str,
        body: &str,
    ) -> Result<PullRequest, GithubError> {
        self.post(
            who,
            &format!("/repos/{repository}/pulls"),
            json!({ "title": title, "head": head, "base": base, "body": body, "draft": true }),
        )
        .await
    }

    pub async fn close_pull_request(
        &self,
        who: As,
        repository: &str,
        number: i32,
    ) -> Result<(), GithubError> {
        let closed: Result<Ignored, _> = self
            .patch(
                who,
                &format!("/repos/{repository}/pulls/{number}"),
                json!({ "state": "closed" }),
            )
            .await;
        match closed {
            Ok(_) => Ok(()),
            Err(problem) if problem.is_missing() => Ok(()),
            Err(problem) => Err(problem),
        }
    }

    pub async fn merge_pull_request(
        &self,
        who: As,
        repository: &str,
        number: i32,
        method: &str,
        head: &str,
    ) -> Result<String, GithubError> {
        let merged: MergeResult = self
            .put(
                who,
                &format!("/repos/{repository}/pulls/{number}/merge"),
                json!({ "merge_method": method, "sha": head }),
            )
            .await?;
        Ok(merged.sha)
    }

    pub async fn publish_check(
        &self,
        who: As,
        repository: &str,
        saying: &WhatTheCheckSays<'_>,
    ) -> Result<(), GithubError> {
        let WhatTheCheckSays {
            name,
            head,
            conclusion,
            title,
            summary,
            details_url,
        } = saying;
        let mut body = json!({
            "name": name,
            "head_sha": head,
            "details_url": details_url,
            "output": { "title": title, "summary": summary },
        });
        match *conclusion {
            Some(conclusion) => {
                body["status"] = json!("completed");
                body["conclusion"] = json!(conclusion);
            }
            None => body["status"] = json!("in_progress"),
        }
        let _: Ignored = self
            .post(who, &format!("/repos/{repository}/check-runs"), body)
            .await?;
        Ok(())
    }

    pub async fn make_sure_label_exists(
        &self,
        who: As,
        repository: &str,
        label: &str,
    ) -> Result<(), GithubError> {
        let made: Result<Ignored, _> = self
            .post(
                who,
                &format!("/repos/{repository}/labels"),
                json!({
                    "name": label,
                    "color": "8b5cf6",
                    "description": "Queue this pull request for merging",
                }),
            )
            .await;
        match made {
            Ok(_) => Ok(()),
            Err(problem) if problem.says_it_is_already_there() => Ok(()),
            Err(problem) => Err(problem),
        }
    }

    pub async fn add_label(
        &self,
        who: As,
        repository: &str,
        number: i32,
        label: &str,
    ) -> Result<(), GithubError> {
        let _: Ignored = self
            .post(
                who,
                &format!("/repos/{repository}/issues/{number}/labels"),
                json!({ "labels": [label] }),
            )
            .await?;
        Ok(())
    }

    pub async fn remove_label(
        &self,
        who: As,
        repository: &str,
        number: i32,
        label: &str,
    ) -> Result<(), GithubError> {
        match self
            .delete(
                who,
                &format!("/repos/{repository}/issues/{number}/labels/{label}"),
            )
            .await
        {
            Ok(()) => Ok(()),
            Err(problem) if problem.is_missing() => Ok(()),
            Err(problem) => Err(problem),
        }
    }

    pub async fn say_something(
        &self,
        who: As,
        repository: &str,
        number: i32,
        body: &str,
    ) -> Result<(), GithubError> {
        let _: Ignored = self
            .post(
                who,
                &format!("/repos/{repository}/issues/{number}/comments"),
                json!({ "body": body }),
            )
            .await?;
        Ok(())
    }

    pub async fn what_may_this_person_do(
        &self,
        who: As,
        repository: &str,
        login: &str,
    ) -> Result<String, GithubError> {
        #[derive(Deserialize)]
        struct Standing {
            permission: String,
        }
        let standing: Standing = self
            .get(
                who,
                &format!("/repos/{repository}/collaborators/{login}/permission"),
            )
            .await?;
        Ok(standing.permission)
    }
}
