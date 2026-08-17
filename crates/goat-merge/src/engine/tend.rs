use goat_merge_core::config::{self, Config};
use goat_merge_core::{
    Aboard, CHECK_NAME, Conclusion, Enqueue, InQueue, MergeMethod, Next, NotQueued,
    Queue as QueueRules, Readiness, Sha, Status, Verification, WhyFailed, after_a_batch, assess,
    decide, how_many_to_verify,
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

const AT_A_TIME: usize = 4;

struct Riding {
    entry: Entry,
    head: String,
    title: String,
    status: Status,
    changed: bool,
}

struct TheBatch {
    riding: Vec<Riding>,
    attempt: Option<Attempt>,
    next: Next,
}

impl Riding {
    fn check_is_about<'a>(&'a self, branch: &'a str) -> WhatTheCheckIsAbout<'a> {
        WhatTheCheckIsAbout {
            head: &self.head,
            pull_request: self.entry.pull_request,
            branch,
        }
    }
}

fn what_to_call_the_candidate(aboard: &[Aboard], onto: &Sha) -> String {
    let first = aboard.first().map_or(0, |one| one.number);
    let sha7: String = onto.as_str().chars().take(7).collect();
    format!("merge-queue/candidate-{first}-{sha7}")
}

fn why_it_moved(now: usize, next: usize, aboard: &[Aboard], passed: bool) -> String {
    let size = aboard.len();
    let what_ran = if size == 1 {
        format!("#{}", aboard.first().map_or(0, |one| one.number))
    } else {
        let numbers: Vec<String> = aboard
            .iter()
            .map(|one| format!("#{}", one.number))
            .collect();
        match numbers.split_last() {
            Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
            None => "nothing".to_owned(),
        }
    };
    let how = if size == 1 { "on its own" } else { "together" };
    match (passed, next == now) {
        (true, true) => format!("{what_ran} passed {how}"),
        (true, false) => format!("{what_ran} passed {how}, so the queue is trying one more"),
        (false, true) => format!("{what_ran} failed {how}, and there is nowhere smaller to go"),
        (false, false) => format!("{what_ran} failed {how}, so the queue is trying fewer"),
    }
}

fn what_to_call_the_draft(riding: &[&Riding]) -> String {
    match riding {
        [one] => format!("Merge queue: #{} {}", one.entry.pull_request, one.title),
        many => format!("Merge queue: {} pull requests together", many.len()),
    }
}

fn what_the_draft_explains(riding: &[&Riding], branch: &str) -> String {
    let listed = riding
        .iter()
        .map(|one| format!("- #{} {}", one.entry.pull_request, one.title))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Verifying these on top of `{branch}`, in this order:\n\n{listed}\n\nThis pull request \
         only exists to run the repository's checks and is closed automatically. If it fails, \
         fewer will be verified together next time until the one at fault is found."
    )
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
        if let Some(attempt) = self.store.verification_carrying(entry.id).await? {
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
        if let Some(attempt) = self.store.verification_carrying(entry.id).await? {
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

        if crate::github::look::allowed_by(&settings, &protection).is_empty() {
            tracing::warn!(
                repository = %name,
                branch = %queue.branch,
                "github says this repository allows no merge method at all, which it cannot \
                 mean, so nothing is decided from this read"
            );
            self.store
                .ask_for_later(super::TEND, &super::subject(queue.id), LOOK_AGAIN_IN)
                .await?;
            return Ok(());
        }

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

        let running = self.store.running(tending.queue.id).await?;
        let looked_at = self
            .look_at_all_of_them(&tending, &running, &settings, &protection)
            .await?;

        let mut seen = Vec::new();
        for (id, looked) in looked_at {
            let Some(entry) = running.iter().find(|entry| entry.id == id).cloned() else {
                continue;
            };
            self.store
                .remember_title(entry.id, &looked.pull_request.title)
                .await?;
            if self
                .caught_up_with_github(&tending, &entry, &looked)
                .await?
            {
                continue;
            }
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

        let live = match self.store.live_attempt_on(tending.queue.id).await? {
            Some(attempt) => Some(self.catch_up_on(&tending, attempt).await?),
            None => None,
        };
        let travelling = self.who_travels_together(&tending, &live, &ready).await?;
        let Some(travelling) = travelling else {
            return Ok(());
        };
        let aboard: Vec<Aboard> = travelling
            .iter()
            .filter_map(|id| seen.iter().find(|(seen_id, _)| seen_id == id))
            .map(|(_, looked)| Aboard {
                number: looked.snapshot.number,
                head: looked.snapshot.head.clone(),
            })
            .collect();

        let mut anything_in_flight = false;
        let mut riding = Vec::new();
        let mut looking_on = Vec::new();
        let mut next = Next::Nothing;
        for entry in entries {
            let Some((_, looked)) = seen.iter().find(|(id, _)| *id == entry.id) else {
                continue;
            };
            let decision = decide(
                &looked.snapshot,
                &tending.rules,
                &InQueue {
                    ahead: ready.iter().position(|id| *id == entry.id).unwrap_or(0),
                    aboard: &aboard,
                    paused: tending.queue.paused,
                    verification: live.as_ref().map(|(_, seen)| seen),
                },
            );
            anything_in_flight |= matches!(
                decision.status,
                Status::Preparing { .. } | Status::Validating { .. } | Status::Merging
            );
            let changed = self
                .store
                .record_status(
                    entry.id,
                    decision.status.headline(),
                    &decision.status.to_string(),
                )
                .await?;
            let standing = Riding {
                entry,
                head: looked.snapshot.head.to_string(),
                title: looked.pull_request.title.clone(),
                status: decision.status,
                changed,
            };
            if !travelling.contains(&standing.entry.id) {
                looking_on.push(standing);
                continue;
            }
            if riding.is_empty() {
                next = decision.next;
            }
            riding.push(standing);
        }

        self.say_where_they_all_stand(&tending, &looking_on).await?;

        if !riding.is_empty() {
            let batch = TheBatch {
                riding,
                attempt: live.map(|(attempt, _)| attempt),
                next,
            };
            self.act_on_the_batch(&tending, batch).await?;
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

    async fn look_at_all_of_them(
        &self,
        tending: &Tending,
        running: &[Entry],
        settings: &crate::github::calls::RepositorySettings,
        protection: &crate::github::calls::Protection,
    ) -> Result<Vec<(i64, Seen)>, EngineError> {
        let mut looked = Vec::with_capacity(running.len());
        for some in running.chunks(AT_A_TIME) {
            let together = some.iter().map(|entry| {
                self.github.look_at(
                    tending.who,
                    &tending.name,
                    entry.pull_request,
                    &tending.base,
                    settings,
                    protection,
                )
            });
            for (entry, one) in some
                .iter()
                .zip(futures_util::future::join_all(together).await)
            {
                match one {
                    Ok(seen) => looked.push((entry.id, seen)),
                    Err(problem) if problem.is_missing() => {
                        tracing::warn!(
                            pull_request = entry.pull_request,
                            "github would not show us this pull request, so the rest of the \
                             queue goes on without it this time"
                        );
                    }
                    Err(problem) => return Err(problem.into()),
                }
            }
        }
        Ok(looked)
    }

    async fn who_travels_together(
        &self,
        tending: &Tending,
        live: &Option<(Attempt, Verification)>,
        ready: &[i64],
    ) -> Result<Option<Vec<i64>>, EngineError> {
        let Some((attempt, _)) = live else {
            let how_many = how_many_to_verify(
                &tending.rules,
                usize::try_from(tending.queue.verify_at_once).unwrap_or(1),
                ready.len(),
            );
            return Ok(Some(ready.iter().take(how_many).copied().collect()));
        };
        let still_here: Vec<i64> = self
            .store
            .everyone_aboard(attempt.id)
            .await?
            .into_iter()
            .map(|member| member.entry_id)
            .filter(|id| ready.contains(id))
            .collect();
        if still_here.is_empty() {
            self.tidy_away(tending, attempt, "everyone it was verifying left the queue")
                .await?;
            self.store
                .ask_for(super::TEND, &super::subject(tending.queue.id))
                .await?;
            return Ok(None);
        }
        Ok(Some(still_here))
    }

    async fn say_where_they_all_stand(
        &self,
        tending: &Tending,
        looking_on: &[Riding],
    ) -> Result<(), EngineError> {
        let news: Vec<&Riding> = looking_on.iter().filter(|one| one.changed).collect();
        if news.is_empty() {
            return Ok(());
        }
        for some in news.chunks(AT_A_TIME) {
            let together = some
                .iter()
                .map(|one| self.say_where_it_stands(tending, one));
            futures_util::future::try_join_all(together).await?;
        }
        self.announce(&tending.name);
        Ok(())
    }

    async fn say_where_it_stands(
        &self,
        tending: &Tending,
        riding: &Riding,
    ) -> Result<(), EngineError> {
        self.show(
            tending.who,
            &tending.name,
            &riding.check_is_about(&tending.queue.branch),
            &riding.status,
            None,
        )
        .await?;
        if matches!(riding.status, Status::Blocked(_)) {
            self.github
                .say_something(
                    tending.who,
                    &tending.name,
                    riding.entry.pull_request,
                    &format!("**Merge Queue** — blocked. {}", riding.status),
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
        if let Some(attempt) = self.store.verification_carrying(entry.id).await? {
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
        let aboard: Vec<Aboard> = self
            .store
            .everyone_aboard(attempt.id)
            .await?
            .into_iter()
            .map(|member| Aboard {
                number: u64::try_from(member.pull_request).unwrap_or_default(),
                head: Sha::from(member.head),
            })
            .collect();
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
            if attempt.conclusion != "pending" {
                let passed = attempt.conclusion == "success";
                let now = usize::try_from(tending.queue.verify_at_once).unwrap_or(1);
                let next = after_a_batch(now, aboard.len(), &tending.rules, passed);
                self.store
                    .verify_this_many_next_time(
                        tending.queue.id,
                        next,
                        &why_it_moved(now, next, &aboard, passed),
                    )
                    .await?;
            }
        }
        let verification = Verification {
            base: Sha::from(attempt.base.clone()),
            aboard,
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

    async fn act_on_the_batch(
        &self,
        tending: &Tending,
        batch: TheBatch,
    ) -> Result<(), EngineError> {
        match &batch.next {
            Next::Nothing => {
                for riding in &batch.riding {
                    if matches!(riding.status, Status::Failed(_)) {
                        self.give_up(tending, riding, batch.attempt.as_ref())
                            .await?;
                    } else {
                        self.tell_them_where_they_stand(tending, riding).await?;
                    }
                }
            }
            Next::BuildCandidate { onto, aboard } => {
                for riding in &batch.riding {
                    self.tell_them_where_they_stand(tending, riding).await?;
                }
                let onto = onto.clone();
                let aboard = aboard.clone();
                self.build_a_candidate(tending, &batch, &onto, &aboard)
                    .await?;
            }
            Next::DiscardVerification => {
                if let Some(attempt) = &batch.attempt {
                    let narrowing = matches!(attempt.conclusion.as_str(), "failure" | "timed_out");
                    let because = if narrowing {
                        "the checks failed, so fewer will be verified together next time"
                    } else {
                        "the base or a head moved"
                    };
                    self.tidy_away(tending, attempt, because).await?;
                    if narrowing {
                        self.say_the_batch_is_being_narrowed(tending, &batch)
                            .await?;
                    }
                }
                for riding in &batch.riding {
                    self.tell_them_where_they_stand(tending, riding).await?;
                }
                self.store
                    .ask_for(super::TEND, &super::subject(tending.queue.id))
                    .await?;
            }
            Next::Merge { method } => {
                self.merge_the_batch(tending, &batch, *method).await?;
            }
        }

        if batch.riding.iter().any(|riding| riding.changed) {
            self.announce(&tending.name);
        }
        Ok(())
    }

    async fn say_the_batch_is_being_narrowed(
        &self,
        tending: &Tending,
        batch: &TheBatch,
    ) -> Result<(), EngineError> {
        if batch.riding.len() < 2 {
            return Ok(());
        }
        let together = batch
            .riding
            .iter()
            .map(|riding| format!("#{}", riding.entry.pull_request))
            .collect::<Vec<_>>()
            .join(", ");
        let now = self
            .store
            .queue_by_id(tending.queue.id)
            .await?
            .map_or(1, |queue| queue.verify_at_once);
        for riding in &batch.riding {
            self.store
                .write_down(
                    US,
                    "verified together and failed",
                    Some(tending.repository.id),
                    Some(riding.entry.pull_request),
                    &format!(
                        "{together} failed on one candidate, so none of them is the one at \
                         fault yet. The queue will verify {now} at a time until it finds out."
                    ),
                )
                .await?;
        }
        Ok(())
    }

    async fn tell_them_where_they_stand(
        &self,
        tending: &Tending,
        riding: &Riding,
    ) -> Result<(), EngineError> {
        if !riding.changed {
            return Ok(());
        }
        self.show(
            tending.who,
            &tending.name,
            &riding.check_is_about(&tending.queue.branch),
            &riding.status,
            None,
        )
        .await?;
        if matches!(riding.status, Status::Blocked(_)) {
            self.github
                .say_something(
                    tending.who,
                    &tending.name,
                    riding.entry.pull_request,
                    &format!("**Merge Queue** — blocked. {}", riding.status),
                )
                .await?;
        }
        Ok(())
    }

    async fn build_a_candidate(
        &self,
        tending: &Tending,
        batch: &TheBatch,
        onto: &Sha,
        aboard: &[Aboard],
    ) -> Result<(), EngineError> {
        let branch = what_to_call_the_candidate(aboard, onto);
        self.github
            .make_branch(tending.who, &tending.name, &branch, onto.as_str())
            .await?;

        let mut carried = Vec::new();
        let mut candidate = onto.as_str().to_owned();
        for riding in &batch.riding {
            match self
                .github
                .merge_into_branch(
                    tending.who,
                    &tending.name,
                    &branch,
                    &riding.head,
                    &format!(
                        "Merge queue candidate for #{} onto {}",
                        riding.entry.pull_request, tending.queue.branch
                    ),
                )
                .await
            {
                Ok(merged) => {
                    if let Some(result) = merged {
                        candidate = result.sha;
                    }
                    carried.push((riding.entry.id, riding.head.clone()));
                }
                Err(problem) if problem.is_conflict() => {
                    self.let_go_of_what_no_longer_merges(tending, riding)
                        .await?;
                }
                Err(problem) => return Err(problem.into()),
            }
        }
        if carried.is_empty() {
            self.github
                .delete_branch(tending.who, &tending.name, &branch)
                .await?;
            return Ok(());
        }

        let narrowed_from = self.what_this_narrows(tending, carried.len()).await?;
        let attempt = self
            .store
            .open_attempt(
                tending.queue.id,
                onto.as_str(),
                &branch,
                &carried,
                narrowed_from,
            )
            .await?;
        let riding_on_it: Vec<&Riding> = batch
            .riding
            .iter()
            .filter(|riding| carried.iter().any(|(id, _)| *id == riding.entry.id))
            .collect();
        let draft = self
            .github
            .open_draft(
                tending.who,
                &tending.name,
                &what_to_call_the_draft(&riding_on_it),
                &branch,
                &tending.queue.branch,
                &what_the_draft_explains(&riding_on_it, &tending.queue.branch),
            )
            .await?;
        self.store
            .attach_candidate(attempt.id, Some(draft.number), &candidate)
            .await?;
        for riding in &riding_on_it {
            self.store
                .write_down(
                    US,
                    "verification started",
                    Some(tending.repository.id),
                    Some(riding.entry.pull_request),
                    &format!("candidate {candidate} on {branch}"),
                )
                .await?;
        }
        Ok(())
    }

    async fn what_this_narrows(
        &self,
        tending: &Tending,
        size: usize,
    ) -> Result<Option<i64>, EngineError> {
        let Some(before) = self
            .store
            .the_last_verification_on(tending.queue.id)
            .await?
        else {
            return Ok(None);
        };
        if !matches!(before.conclusion.as_str(), "failure" | "timed_out") {
            return Ok(None);
        }
        let was = self.store.everyone_aboard(before.id).await?.len();
        Ok((size < was).then_some(before.id))
    }

    async fn let_go_of_what_no_longer_merges(
        &self,
        tending: &Tending,
        riding: &Riding,
    ) -> Result<(), EngineError> {
        let status = Status::Failed(WhyFailed::ConflictWhilePreparing);
        self.store
            .settle(
                riding.entry.id,
                status.headline(),
                &status.to_string(),
                None,
            )
            .await?;
        self.show(
            tending.who,
            &tending.name,
            &riding.check_is_about(&tending.queue.branch),
            &status,
            None,
        )
        .await?;
        Ok(())
    }

    async fn merge_the_batch(
        &self,
        tending: &Tending,
        batch: &TheBatch,
        method: MergeMethod,
    ) -> Result<(), EngineError> {
        let mut refused = None;
        for riding in &batch.riding {
            if !self.merge_it(tending, riding, method).await? {
                refused = Some(riding);
                break;
            }
        }
        if let Some(attempt) = &batch.attempt {
            let because = if refused.is_some() {
                "GitHub refused a merge"
            } else {
                "the pull requests merged"
            };
            self.tidy_away(tending, attempt, because).await?;
        }
        self.store
            .ask_for(super::TEND, &super::subject(tending.queue.id))
            .await?;
        Ok(())
    }

    async fn merge_it(
        &self,
        tending: &Tending,
        riding: &Riding,
        method: MergeMethod,
    ) -> Result<bool, EngineError> {
        self.show(
            tending.who,
            &tending.name,
            &riding.check_is_about(&tending.queue.branch),
            &Status::Merging,
            Some("success"),
        )
        .await?;

        match self
            .github
            .merge_pull_request(
                tending.who,
                &tending.name,
                riding.entry.pull_request,
                &method.to_string(),
                &riding.head,
            )
            .await
        {
            Ok(sha) => {
                self.store
                    .settle(
                        riding.entry.id,
                        Status::MERGED,
                        &format!("merged as {}", sha.chars().take(7).collect::<String>()),
                        Some(&sha),
                    )
                    .await?;
                self.store
                    .write_down(
                        US,
                        "merged",
                        Some(tending.repository.id),
                        Some(riding.entry.pull_request),
                        &format!("{method} as {sha}"),
                    )
                    .await?;
                self.github
                    .remove_label(
                        tending.who,
                        &tending.name,
                        riding.entry.pull_request,
                        config::LABEL,
                    )
                    .await?;
                self.store
                    .note_where_the_base_is(tending.queue.id, &sha)
                    .await?;
                Ok(true)
            }
            Err(problem) => {
                let status = Status::Failed(WhyFailed::MergeRejected {
                    message: problem.to_string(),
                });
                self.store
                    .record_status(riding.entry.id, status.headline(), &status.to_string())
                    .await?;
                self.show(
                    tending.who,
                    &tending.name,
                    &riding.check_is_about(&tending.queue.branch),
                    &status,
                    None,
                )
                .await?;
                if problem.is_worth_another_try() {
                    return Err(problem.into());
                }
                self.store
                    .settle(
                        riding.entry.id,
                        status.headline(),
                        &status.to_string(),
                        None,
                    )
                    .await?;
                Ok(false)
            }
        }
    }

    async fn give_up(
        &self,
        tending: &Tending,
        riding: &Riding,
        attempt: Option<&Attempt>,
    ) -> Result<(), EngineError> {
        let status = &riding.status;
        self.store
            .settle(
                riding.entry.id,
                status.headline(),
                &status.to_string(),
                None,
            )
            .await?;
        self.show(
            tending.who,
            &tending.name,
            &riding.check_is_about(&tending.queue.branch),
            status,
            None,
        )
        .await?;
        self.github
            .say_something(
                tending.who,
                &tending.name,
                riding.entry.pull_request,
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
                riding.entry.pull_request,
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
                Some(riding.entry.pull_request),
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
