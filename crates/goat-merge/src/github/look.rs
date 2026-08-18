use goat_merge_core::config::LABEL;
use goat_merge_core::{Base, Check, Conclusion, MergeMethod, Mergeable, Sha, Snapshot};
use time::OffsetDateTime;

use super::calls::PullRequest;
use super::{As, Github, GithubError};

pub const FORK_WORKFLOW: &str = ".github/workflows/merge-queue-fork.yml";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhereTheBaseIs {
    pub sha: String,
    pub moved_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct Seen {
    pub pull_request: PullRequest,
    pub snapshot: Snapshot,
}

impl Github {
    pub async fn look_at(
        &self,
        who: As,
        repository: &str,
        number: i32,
        base: &WhereTheBaseIs,
        settings: &super::calls::RepositorySettings,
        protection: &super::calls::Protection,
    ) -> Result<Seen, GithubError> {
        let pull_request = self.pull_request(who, repository, number).await?;
        let head = pull_request.head.sha.clone();
        let branch = pull_request.base.branch.clone();
        let from_fork = pull_request.from_a_fork();

        let wanted = protection.checks_somebody_else_runs();
        let (approvals, required_checks, fork_workflow_declared_safe) = tokio::try_join!(
            self.approvals_on(who, repository, number, &head),
            self.required_checks_on(who, repository, &head, &wanted),
            async {
                if from_fork {
                    self.fork_workflow_looks_safe(who, repository, &branch)
                        .await
                } else {
                    Ok(false)
                }
            },
        )?;

        let snapshot = Snapshot {
            number: u64::try_from(number).unwrap_or_default(),
            head: Sha::from(head),
            base: Base {
                branch,
                sha: Sha::from(base.sha.clone()),
                last_moved_at: base.moved_at,
            },
            draft: pull_request.draft,
            mergeable: match pull_request.mergeable {
                Some(true) => Mergeable::Clean,
                Some(false) => Mergeable::Conflict,
                None => Mergeable::Unknown,
            },
            approvals,
            approvals_required: protection.required_approvals,
            labelled: pull_request.carries(LABEL),
            from_fork,
            fork_workflow_declared_safe,
            required_checks,
            allowed_merge_methods: allowed_by(settings, protection),
        };
        Ok(Seen {
            pull_request,
            snapshot,
        })
    }

    pub async fn required_checks_on(
        &self,
        who: As,
        repository: &str,
        head: &str,
        required: &[String],
    ) -> Result<Vec<Check>, GithubError> {
        if required.is_empty() {
            return Ok(Vec::new());
        }
        let (runs, statuses) = tokio::try_join!(
            self.check_runs(who, repository, head),
            self.commit_statuses(who, repository, head),
        )?;

        Ok(required
            .iter()
            .map(|name| {
                if let Some(run) = runs.iter().rfind(|run| &run.name == name) {
                    return Check {
                        name: name.clone(),
                        conclusion: from_a_check_run(&run.status, run.conclusion.as_deref()),
                        head: Sha::from(run.head_sha.clone()),
                        started_at: run.started_at,
                    };
                }
                if let Some(status) = statuses.iter().find(|status| &status.context == name) {
                    return Check {
                        name: name.clone(),
                        conclusion: from_a_commit_status(&status.state),
                        head: Sha::from(head.to_owned()),
                        started_at: None,
                    };
                }
                Check {
                    name: name.clone(),
                    conclusion: Conclusion::Pending,
                    head: Sha::from(head.to_owned()),
                    started_at: None,
                }
            })
            .collect())
    }

    async fn fork_workflow_looks_safe(
        &self,
        who: As,
        repository: &str,
        branch: &str,
    ) -> Result<bool, GithubError> {
        let Some(workflow) = self.file(who, repository, FORK_WORKFLOW, branch).await? else {
            return Ok(false);
        };
        Ok(declares_no_secrets(&workflow))
    }
}

pub fn declares_no_secrets(workflow: &str) -> bool {
    let runs = what_it_actually_runs(workflow);
    let mentions_secrets = runs.contains("secrets.") || runs.contains("secrets:");
    let declares_permissions = runs
        .lines()
        .any(|line| line.trim_start().starts_with("permissions:"));
    let writes = runs.contains("write-all") || runs.lines().any(|line| line.contains(": write"));
    declares_permissions && !mentions_secrets && !writes
}

fn what_it_actually_runs(workflow: &str) -> String {
    workflow
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn allowed_by(
    settings: &super::calls::RepositorySettings,
    protection: &super::calls::Protection,
) -> Vec<MergeMethod> {
    let mut allowed = Vec::new();
    if settings.allow_merge_commit {
        allowed.push(MergeMethod::Merge);
    }
    if settings.allow_squash_merge {
        allowed.push(MergeMethod::Squash);
    }
    if settings.allow_rebase_merge {
        allowed.push(MergeMethod::Rebase);
    }
    match &protection.only_these_merge_methods {
        Some(only) => allowed
            .into_iter()
            .filter(|method| only.iter().any(|named| named == &method.to_string()))
            .collect(),
        None => allowed,
    }
}

fn from_a_check_run(status: &str, conclusion: Option<&str>) -> Conclusion {
    if status != "completed" {
        return Conclusion::Pending;
    }
    match conclusion {
        Some("success" | "neutral" | "skipped") => Conclusion::Success,
        Some("failure" | "timed_out" | "cancelled" | "action_required" | "stale") => {
            Conclusion::Failure
        }
        _ => Conclusion::Pending,
    }
}

fn from_a_commit_status(state: &str) -> Conclusion {
    match state {
        "success" => Conclusion::Success,
        "failure" | "error" => Conclusion::Failure,
        _ => Conclusion::Pending,
    }
}
