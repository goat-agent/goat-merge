use goat_merge_core::config::{self, Config};
use goat_merge_core::{
    CHECK_NAME, Conclusion, Decision, Enqueue, InQueue, MergeMethod, Next, NotQueued,
    Queue as QueueRules, Readiness, Sha, Status, Verification, WhyFailed, assess, decide,
};
use time::{Duration, OffsetDateTime};

use super::{Engine, EngineError, LOOK_AGAIN_IN};
use crate::github::calls::WhatTheCheckSays;
use crate::github::look::{Seen, WhereTheBaseIs};
use crate::github::{As, GithubError};
use crate::store::audit::US;
use crate::store::queue::{Attempt, Entry, Queue};
use crate::store::repositories::Repository;

pub(crate) struct WhatTheCheckIsAbout<'a> {
    pub head: &'a str,
    pub pull_request: i32,
    pub branch: &'a str,
}

struct Tending {
    who: As,
    repository: Repository,
    name: String,
    queue: Queue,
    rules: QueueRules,
    base: WhereTheBaseIs,
    required: Vec<String>,
}

impl Engine {
    pub async fn tend(&self, queue_id: i64) -> Result<(), EngineError> {
        let held = self.store.hold_the_branch(queue_id).await?;
        let outcome = self.tend_while_holding(queue_id).await;
        held.commit()
            .await
            .map_err(crate::store::StoreError::from)?;
        match outcome {
            Err(EngineError::Github(problem)) if problem.is_missing() => {
                self.let_go_of_what_github_will_not_show_us(queue_id, problem)
                    .await
            }
            outcome => outcome,
        }
    }

    pub async fn put_it_in_the_queue(
        &self,
        repository: &Repository,
        number: i32,
        who: &str,
        doing: &'static str,
    ) -> Result<(), EngineError> {
        let installation = As(repository.installation_id);
        let full = repository.full_name();
        let pull = self
            .github
            .pull_request(installation, &full, number)
            .await?;
        let queue = self
            .store
            .queue_for(repository.id, &pull.base.branch)
            .await?;
        let entry = self.store.enter(queue.id, number, who, &pull.title).await?;
        if let Some(attempt) = self.store.live_attempt(entry.id).await? {
            self.store
                .discard_attempt(attempt.id, "asked to start again")
                .await?;
        }
        self.github
            .add_label(installation, &full, number, config::LABEL)
            .await?;
        self.store
            .write_down(
                who,
                doing,
                Some(repository.id),
                Some(number),
                &pull.base.branch,
            )
            .await?;
        self.announce(&full);
        self.tend_queue_soon(queue.id).await?;
        Ok(())
    }

    pub async fn take_it_out_of_the_queue(
        &self,
        repository: &Repository,
        number: i32,
        who: &str,
    ) -> Result<bool, EngineError> {
        let installation = As(repository.installation_id);
        let full = repository.full_name();
        self.github
            .remove_label(installation, &full, number, config::LABEL)
            .await?;
        let Some(entry) = self.entry_anywhere_in(repository, number).await? else {
            return Ok(false);
        };
        if entry.settled_at.is_some() {
            return Ok(true);
        }
        let queue = self
            .store
            .queue_by_id(entry.queue_id)
            .await?
            .ok_or(EngineError::NoQueue(entry.queue_id))?;
        if let Some(attempt) = self.store.live_attempt(entry.id).await? {
            self.store
                .discard_attempt(attempt.id, "the pull request left the queue")
                .await?;
        }
        let status = Status::Cancelled { by: who.to_owned() };
        self.store
            .settle(entry.id, status.headline(), &status.to_string(), None)
            .await?;
        self.store
            .write_down(
                who,
                "left the queue",
                Some(repository.id),
                Some(number),
                &status.to_string(),
            )
            .await?;
        let pull = self
            .github
            .pull_request(installation, &full, number)
            .await?;
        self.show(
            installation,
            &full,
            &WhatTheCheckIsAbout {
                head: &pull.head.sha,
                pull_request: number,
                branch: &queue.branch,
            },
            &status,
            Some("cancelled"),
        )
        .await?;
        self.announce(&full);
        self.tend_queue_soon(entry.queue_id).await?;
        Ok(true)
    }

    pub async fn entry_anywhere_in(
        &self,
        repository: &Repository,
        number: i32,
    ) -> Result<Option<Entry>, EngineError> {
        for queue in self.store.queues_of(repository.id).await? {
            if let Some(entry) = self.store.entry(queue.id, number).await? {
                return Ok(Some(entry));
            }
        }
        Ok(None)
    }

    async fn let_go_of_what_github_will_not_show_us(
        &self,
        queue_id: i64,
        problem: GithubError,
    ) -> Result<(), EngineError> {
        let Some(queue) = self.store.queue_by_id(queue_id).await? else {
            return Ok(());
        };
        let Some(repository) = self.store.repository_by_id(queue.repository_id).await? else {
            return Ok(());
        };
        let name = repository.full_name();
        if self
            .github
            .repository_settings(As(repository.installation_id), &name)
            .await
            .is_ok()
        {
            return Err(EngineError::Github(problem));
        }

        self.store.repository_is_out_of_reach(repository.id).await?;
        self.store
            .write_down(
                US,
                "stopped watching",
                Some(repository.id),
                None,
                "GitHub no longer shows us this repository. Reinstall the App on it to start again.",
            )
            .await?;
        tracing::warn!(repository = %name, "github no longer shows us this repository");
        Ok(())
    }

    async fn tend_while_holding(&self, queue_id: i64) -> Result<(), EngineError> {
        let Some(queue) = self.store.queue_by_id(queue_id).await? else {
            return Err(EngineError::NoQueue(queue_id));
        };
        let Some(repository) = self.store.repository_by_id(queue.repository_id).await? else {
            return Ok(());
        };
        if !repository.active || !repository.we_can_still_see_it() {
            return Ok(());
        }
        let who = As(repository.installation_id);
        let name = repository.full_name();

        let tip = self.github.branch_tip(who, &name, &queue.branch).await?;
        let queue = self.store.note_where_the_base_is(queue.id, &tip).await?;
        let base = WhereTheBaseIs {
            sha: tip,
            moved_at: queue.base_moved_at,
        };

        let settings = self.github.repository_settings(who, &name).await?;
        let protection = self.github.protection_of(who, &name, &queue.branch).await?;

        let Some(rules) = self.rules_for(who, &name, &queue.branch).await? else {
            return self
                .tell_everyone_this_branch_is_unmanaged(who, &name, &queue)
                .await;
        };

        let tending = Tending {
            who,
            repository,
            name,
            rules,
            base,
            required: protection.checks_somebody_else_runs(),
            queue,
        };

        let mut seen = Vec::new();
        for entry in self.store.running(tending.queue.id).await? {
            let looked = self
                .github
                .look_at(
                    who,
                    &tending.name,
                    entry.pull_request,
                    &tending.base,
                    &settings,
                    &protection,
                )
                .await?;
            self.store
                .remember_title(entry.id, &looked.pull_request.title)
                .await?;
            if assess(&looked.snapshot) == Readiness::Ready {
                self.store.take_a_number(entry.id).await?;
            } else if entry.queued_at.is_some() {
                self.store.give_up_the_number(entry.id).await?;
            }
            seen.push((entry.id, looked));
        }

        let entries = self.store.running(tending.queue.id).await?;
        let ready: Vec<i64> = entries
            .iter()
            .filter(|entry| entry.queued_at.is_some())
            .map(|entry| entry.id)
            .collect();

        let mut anything_in_flight = false;
        for entry in entries {
            let Some((_, looked)) = seen.iter().find(|(id, _)| *id == entry.id) else {
                continue;
            };
            if self.caught_up_with_github(&tending, &entry, looked).await? {
                continue;
            }
            let ahead = ready.iter().position(|id| *id == entry.id).unwrap_or(0);
            let attempt = match self.store.live_attempt(entry.id).await? {
                Some(attempt) => Some(self.catch_up_on(&tending, attempt).await?),
                None => None,
            };
            let decision = decide(
                &looked.snapshot,
                &tending.rules,
                &InQueue {
                    ahead,
                    paused: tending.queue.paused,
                    verification: attempt.as_ref().map(|(_, seen)| seen),
                },
            );
            anything_in_flight |= matches!(
                decision.status,
                Status::Preparing | Status::Validating | Status::Merging
            );
            let attempt = attempt.map(|(row, _)| row);
            self.act_on(&tending, &entry, looked, attempt, decision)
                .await?;
        }

        if anything_in_flight {
            self.store
                .ask_for_later(
                    super::TEND,
                    &super::subject(tending.queue.id),
                    LOOK_AGAIN_IN,
                )
                .await?;
        }
        Ok(())
    }

    async fn caught_up_with_github(
        &self,
        tending: &Tending,
        entry: &Entry,
        looked: &Seen,
    ) -> Result<bool, EngineError> {
        let settled = if looked.pull_request.merged() {
            Some((Status::Merged, "merged outside the queue".to_owned()))
        } else if looked.pull_request.state == "closed" {
            Some((
                Status::Cancelled {
                    by: "whoever closed it".to_owned(),
                },
                "the pull request was closed".to_owned(),
            ))
        } else if !looked.snapshot.labelled && tending.rules.enqueue == Enqueue::Manual {
            Some((
                Status::Cancelled {
                    by: "whoever took the label off".to_owned(),
                },
                format!("the {} label was taken off", config::LABEL),
            ))
        } else {
            None
        };
        let Some((status, detail)) = settled else {
            return Ok(false);
        };
        if let Some(attempt) = self.store.live_attempt(entry.id).await? {
            self.tidy_away(tending, &attempt, &detail).await?;
        }
        self.store
            .settle(entry.id, status.headline(), &detail, None)
            .await?;
        self.store
            .write_down(
                US,
                "left the queue",
                Some(tending.repository.id),
                Some(entry.pull_request),
                &detail,
            )
            .await?;
        if !matches!(status, Status::Merged) {
            self.show(
                tending.who,
                &tending.name,
                &WhatTheCheckIsAbout {
                    head: looked.snapshot.head.as_str(),
                    pull_request: entry.pull_request,
                    branch: &tending.queue.branch,
                },
                &status,
                Some("cancelled"),
            )
            .await?;
        }
        self.announce(&tending.name);
        Ok(true)
    }

    async fn rules_for(
        &self,
        who: As,
        name: &str,
        branch: &str,
    ) -> Result<Option<QueueRules>, EngineError> {
        let Some(written) = self.github.file(who, name, config::FILE, branch).await? else {
            return Ok(Some(QueueRules::following_github(branch)));
        };
        match Config::parse(&written) {
            Ok(parsed) => Ok(parsed.queue_for(branch).cloned()),
            Err(problem) => {
                tracing::warn!(%name, %branch, %problem, "the queue configuration was ignored");
                Ok(Some(QueueRules::following_github(branch)))
            }
        }
    }

    async fn tell_everyone_this_branch_is_unmanaged(
        &self,
        who: As,
        name: &str,
        queue: &Queue,
    ) -> Result<(), EngineError> {
        let status = Status::NotQueued(NotQueued::NoQueueForBranch {
            branch: queue.branch.clone(),
        });
        for entry in self.store.running(queue.id).await? {
            let changed = self
                .store
                .record_status(entry.id, status.headline(), &status.to_string())
                .await?;
            if !changed {
                continue;
            }
            let looked = self
                .github
                .pull_request(who, name, entry.pull_request)
                .await?;
            self.show(
                who,
                name,
                &WhatTheCheckIsAbout {
                    head: &looked.head.sha,
                    pull_request: entry.pull_request,
                    branch: &queue.branch,
                },
                &status,
                None,
            )
            .await?;
        }
        Ok(())
    }

    async fn catch_up_on(
        &self,
        tending: &Tending,
        attempt: Attempt,
    ) -> Result<(Attempt, Verification), EngineError> {
        let mut attempt = attempt;
        if attempt.conclusion == "pending"
            && let Some(candidate) = attempt.candidate_sha.clone()
        {
            let checks = self
                .github
                .required_checks_on(tending.who, &tending.name, &candidate, &tending.required)
                .await?;
            let failed: Vec<String> = checks
                .iter()
                .filter(|check| check.conclusion == Conclusion::Failure)
                .map(|check| check.name.clone())
                .collect();
            let all_passed = checks
                .iter()
                .all(|check| check.conclusion == Conclusion::Success);
            let allowed = Duration::seconds(
                i64::try_from(tending.rules.check_timeout.as_secs()).unwrap_or(i64::MAX),
            );
            let overdue = OffsetDateTime::now_utc() - attempt.started_at > allowed;

            if !failed.is_empty() {
                self.store
                    .conclude_attempt(attempt.id, "failure", &failed)
                    .await?;
                attempt.conclusion = "failure".to_owned();
                attempt.failed_checks = failed;
            } else if all_passed {
                self.store
                    .conclude_attempt(attempt.id, "success", &[])
                    .await?;
                attempt.conclusion = "success".to_owned();
            } else if overdue {
                self.store
                    .conclude_attempt(attempt.id, "timed_out", &[])
                    .await?;
                attempt.conclusion = "timed_out".to_owned();
            }
        }
        let verification = Verification {
            base: Sha::from(attempt.base.clone()),
            head: Sha::from(attempt.head.clone()),
            candidate: Sha::from(attempt.candidate_sha.clone().unwrap_or_default()),
            conclusion: match attempt.conclusion.as_str() {
                "success" => Conclusion::Success,
                "failure" | "timed_out" => Conclusion::Failure,
                _ => Conclusion::Pending,
            },
            ran_out_of_time: attempt.conclusion == "timed_out",
            failed_checks: attempt.failed_checks.clone(),
        };
        Ok((attempt, verification))
    }

    async fn act_on(
        &self,
        tending: &Tending,
        entry: &Entry,
        looked: &Seen,
        attempt: Option<Attempt>,
        decision: Decision,
    ) -> Result<(), EngineError> {
        let Decision { status, next } = decision;
        let head = looked.snapshot.head.as_str();
        let changed = self
            .store
            .record_status(entry.id, status.headline(), &status.to_string())
            .await?;

        match next {
            Next::Nothing if matches!(status, Status::Failed(_)) => {
                self.give_up(tending, entry, head, attempt.as_ref(), &status)
                    .await?;
            }
            Next::Nothing => {
                if changed {
                    self.show(
                        tending.who,
                        &tending.name,
                        &WhatTheCheckIsAbout {
                            head,
                            pull_request: entry.pull_request,
                            branch: &tending.queue.branch,
                        },
                        &status,
                        None,
                    )
                    .await?;
                    if matches!(status, Status::Blocked(_)) {
                        self.github
                            .say_something(
                                tending.who,
                                &tending.name,
                                entry.pull_request,
                                &format!("**Merge Queue** — blocked. {status}"),
                            )
                            .await?;
                    }
                }
            }
            Next::BuildCandidate { onto, head: at } => {
                if changed {
                    self.show(
                        tending.who,
                        &tending.name,
                        &WhatTheCheckIsAbout {
                            head,
                            pull_request: entry.pull_request,
                            branch: &tending.queue.branch,
                        },
                        &status,
                        None,
                    )
                    .await?;
                }
                self.build_a_candidate(tending, entry, looked, &onto, &at)
                    .await?;
            }
            Next::DiscardVerification => {
                if let Some(attempt) = attempt {
                    self.tidy_away(tending, &attempt, "the base or the head moved")
                        .await?;
                }
                if changed {
                    self.show(
                        tending.who,
                        &tending.name,
                        &WhatTheCheckIsAbout {
                            head,
                            pull_request: entry.pull_request,
                            branch: &tending.queue.branch,
                        },
                        &status,
                        None,
                    )
                    .await?;
                }
                self.store
                    .ask_for(super::TEND, &super::subject(tending.queue.id))
                    .await?;
            }
            Next::Merge { method } => {
                self.merge_it(tending, entry, head, attempt.as_ref(), method)
                    .await?;
            }
        }

        if changed {
            self.announce(&tending.name);
        }
        Ok(())
    }

    async fn build_a_candidate(
        &self,
        tending: &Tending,
        entry: &Entry,
        looked: &Seen,
        onto: &Sha,
        head: &Sha,
    ) -> Result<(), EngineError> {
        let branch = format!(
            "merge-queue/candidate-{}-{}",
            entry.pull_request,
            head.as_str().chars().take(7).collect::<String>()
        );
        self.github
            .make_branch(tending.who, &tending.name, &branch, onto.as_str())
            .await?;

        let merged = self
            .github
            .merge_into_branch(
                tending.who,
                &tending.name,
                &branch,
                head.as_str(),
                &format!(
                    "Merge queue candidate for #{} onto {}",
                    entry.pull_request, tending.queue.branch
                ),
            )
            .await;
        let merged = match merged {
            Ok(merged) => merged,
            Err(problem) if problem.is_conflict() => {
                self.github
                    .delete_branch(tending.who, &tending.name, &branch)
                    .await?;
                let status = Status::Failed(WhyFailed::ConflictWhilePreparing);
                self.store
                    .settle(entry.id, status.headline(), &status.to_string(), None)
                    .await?;
                self.show(
                    tending.who,
                    &tending.name,
                    &WhatTheCheckIsAbout {
                        head: head.as_str(),
                        pull_request: entry.pull_request,
                        branch: &tending.queue.branch,
                    },
                    &status,
                    None,
                )
                .await?;
                return Ok(());
            }
            Err(problem) => return Err(problem.into()),
        };
        let candidate = merged.map_or_else(|| onto.as_str().to_owned(), |result| result.sha);

        let attempt = self
            .store
            .open_attempt(entry.id, onto.as_str(), head.as_str(), &branch)
            .await?;
        let draft = self
            .github
            .open_draft(
                tending.who,
                &tending.name,
                &format!(
                    "Merge queue: #{} {}",
                    entry.pull_request, looked.pull_request.title
                ),
                &branch,
                &tending.queue.branch,
                &format!(
                    "Verifying #{} on top of `{}`. This pull request only exists to run the \
                     repository's checks and is closed automatically.",
                    entry.pull_request, tending.queue.branch
                ),
            )
            .await?;
        self.store
            .attach_candidate(attempt.id, Some(draft.number), &candidate)
            .await?;
        self.store
            .write_down(
                US,
                "verification started",
                Some(tending.repository.id),
                Some(entry.pull_request),
                &format!("candidate {candidate} on {branch}"),
            )
            .await?;
        Ok(())
    }

    async fn merge_it(
        &self,
        tending: &Tending,
        entry: &Entry,
        head: &str,
        attempt: Option<&Attempt>,
        method: MergeMethod,
    ) -> Result<(), EngineError> {
        self.show(
            tending.who,
            &tending.name,
            &WhatTheCheckIsAbout {
                head,
                pull_request: entry.pull_request,
                branch: &tending.queue.branch,
            },
            &Status::Merging,
            Some("success"),
        )
        .await?;

        match self
            .github
            .merge_pull_request(
                tending.who,
                &tending.name,
                entry.pull_request,
                &method.to_string(),
                head,
            )
            .await
        {
            Ok(sha) => {
                self.store
                    .settle(
                        entry.id,
                        Status::MERGED,
                        &format!("merged as {sha}"),
                        Some(&sha),
                    )
                    .await?;
                self.store
                    .write_down(
                        US,
                        "merged",
                        Some(tending.repository.id),
                        Some(entry.pull_request),
                        &format!("{method} as {sha}"),
                    )
                    .await?;
                if let Some(attempt) = attempt {
                    self.tidy_away(tending, attempt, "the pull request merged")
                        .await?;
                }
                self.github
                    .remove_label(
                        tending.who,
                        &tending.name,
                        entry.pull_request,
                        config::LABEL,
                    )
                    .await?;
                self.store
                    .note_where_the_base_is(tending.queue.id, &sha)
                    .await?;
                self.store
                    .ask_for(super::TEND, &super::subject(tending.queue.id))
                    .await?;
            }
            Err(problem) => {
                let status = Status::Failed(WhyFailed::MergeRejected {
                    message: problem.to_string(),
                });
                self.store
                    .record_status(entry.id, status.headline(), &status.to_string())
                    .await?;
                self.show(
                    tending.who,
                    &tending.name,
                    &WhatTheCheckIsAbout {
                        head,
                        pull_request: entry.pull_request,
                        branch: &tending.queue.branch,
                    },
                    &status,
                    None,
                )
                .await?;
                if problem.is_worth_another_try() {
                    return Err(problem.into());
                }
                self.store
                    .settle(entry.id, status.headline(), &status.to_string(), None)
                    .await?;
                if let Some(attempt) = attempt {
                    self.tidy_away(tending, attempt, "GitHub refused the merge")
                        .await?;
                }
            }
        }
        Ok(())
    }

    async fn give_up(
        &self,
        tending: &Tending,
        entry: &Entry,
        head: &str,
        attempt: Option<&Attempt>,
        status: &Status,
    ) -> Result<(), EngineError> {
        self.store
            .settle(entry.id, status.headline(), &status.to_string(), None)
            .await?;
        self.show(
            tending.who,
            &tending.name,
            &WhatTheCheckIsAbout {
                head,
                pull_request: entry.pull_request,
                branch: &tending.queue.branch,
            },
            status,
            None,
        )
        .await?;
        self.github
            .say_something(
                tending.who,
                &tending.name,
                entry.pull_request,
                &format!(
                    "**Merge Queue** — removed from the queue for `{}`. {status}\n\nFix it and \
                     add the `{}` label again.",
                    tending.queue.branch,
                    config::LABEL
                ),
            )
            .await?;
        self.github
            .remove_label(
                tending.who,
                &tending.name,
                entry.pull_request,
                config::LABEL,
            )
            .await?;
        if let Some(attempt) = attempt {
            self.tidy_away(tending, attempt, "the pull request left the queue")
                .await?;
        }
        self.store
            .write_down(
                US,
                "left the queue",
                Some(tending.repository.id),
                Some(entry.pull_request),
                &status.to_string(),
            )
            .await?;
        Ok(())
    }

    async fn tidy_away(
        &self,
        tending: &Tending,
        attempt: &Attempt,
        because: &str,
    ) -> Result<(), EngineError> {
        if let Some(number) = attempt.candidate_pull_request {
            self.github
                .close_pull_request(tending.who, &tending.name, number)
                .await?;
        }
        self.github
            .delete_branch(tending.who, &tending.name, &attempt.candidate_branch)
            .await?;
        self.store.discard_attempt(attempt.id, because).await?;
        Ok(())
    }

    pub(crate) async fn show(
        &self,
        who: As,
        name: &str,
        about: &WhatTheCheckIsAbout<'_>,
        status: &Status,
        conclusion: Option<&str>,
    ) -> Result<(), EngineError> {
        let conclusion = conclusion.or(match status {
            Status::Merged => Some("success"),
            Status::Blocked(_) | Status::Failed(_) => Some("failure"),
            Status::Cancelled { .. } => Some("cancelled"),
            _ => None,
        });
        self.github
            .publish_check(
                who,
                name,
                &WhatTheCheckSays {
                    name: CHECK_NAME,
                    head: about.head,
                    conclusion,
                    title: status.headline(),
                    summary: &status.to_string(),
                    details_url: &self
                        .settings
                        .queue_url(name, about.branch, about.pull_request),
                },
            )
            .await?;
        Ok(())
    }
}
