use goat_merge_core::config::LABEL;
use serde_json::Value;

use super::{Engine, EngineError};
use crate::store::audit::US;
use crate::store::repositories::Repository;

impl Engine {
    pub async fn react_to(&self, event: &str, payload: &Value) -> Result<(), EngineError> {
        match event {
            "installation" | "installation_repositories" => {
                self.installation_changed(payload).await
            }
            "pull_request" => self.pull_request_changed(payload).await,
            "pull_request_review" | "check_run" | "check_suite" | "status" => {
                self.something_about_the_code_changed(payload).await
            }
            "push" => self.a_branch_moved(payload).await,
            _ => Ok(()),
        }
    }

    async fn installation_changed(&self, payload: &Value) -> Result<(), EngineError> {
        let Some(installation) = payload.get("installation") else {
            return Ok(());
        };
        let Some(id) = installation.get("id").and_then(Value::as_i64) else {
            return Ok(());
        };
        let account = installation
            .get("account")
            .and_then(|account| account.get("login"))
            .and_then(Value::as_str)
            .unwrap_or("unknown");

        if matches!(text(payload, "action"), Some("deleted" | "suspend")) {
            self.store.forget_installation(id).await?;
            if let Ok(auth) = self.github.auth() {
                auth.forget(id);
            }
            return Ok(());
        }

        if !self.store.installation_started_here(id).await? {
            tracing::info!(
                installation = id,
                account,
                "ignoring an installation that was not started on this server"
            );
            return Ok(());
        }
        self.store.remember_installation(id, account).await?;
        for listed in ["repositories", "repositories_added"] {
            let Some(repositories) = payload.get(listed).and_then(Value::as_array) else {
                continue;
            };
            for repository in repositories {
                let (Some(repository_id), Some(full_name)) = (
                    repository.get("id").and_then(Value::as_i64),
                    repository.get("full_name").and_then(Value::as_str),
                ) else {
                    continue;
                };
                if let Some((owner, name)) = full_name.split_once('/') {
                    self.store
                        .remember_repository(repository_id, id, owner, name)
                        .await?;
                }
            }
        }
        for repository in payload
            .get("repositories_removed")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(repository_id) = repository.get("id").and_then(Value::as_i64) else {
                continue;
            };
            self.store.repository_is_out_of_reach(repository_id).await?;
        }
        Ok(())
    }

    async fn pull_request_changed(&self, payload: &Value) -> Result<(), EngineError> {
        let Some(repository) = self.repository_in(payload).await? else {
            return Ok(());
        };
        let Some(pull_request) = payload.get("pull_request") else {
            return Ok(());
        };
        let Some(number) = pull_request.get("number").and_then(Value::as_i64) else {
            return Ok(());
        };
        let number = i32::try_from(number).unwrap_or_default();
        let branch = pull_request
            .get("base")
            .and_then(|base| base.get("ref"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        if branch.is_empty() {
            return Ok(());
        }
        let queue = self.store.queue_for(repository.id, &branch).await?;
        let action = text(payload, "action").unwrap_or_default();
        let who = actor(payload);

        match action {
            "labeled" if label_in(payload) == Some(LABEL) => {
                let waiting_already = self
                    .store
                    .entry(queue.id, number)
                    .await?
                    .is_some_and(|entry| entry.settled_at.is_none());
                if !waiting_already {
                    let title = pull_request
                        .get("title")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    self.store.enter(queue.id, number, &who, title).await?;
                    self.store
                        .write_down(
                            &who,
                            "asked to merge",
                            Some(repository.id),
                            Some(number),
                            &branch,
                        )
                        .await?;
                }
            }
            "unlabeled" if label_in(payload) == Some(LABEL) => {
                self.cancel(&repository, number, &who).await?;
            }
            _ => {}
        }

        self.tend_queue_soon(queue.id).await?;
        Ok(())
    }

    async fn cancel(
        &self,
        repository: &Repository,
        number: i32,
        who: &str,
    ) -> Result<(), EngineError> {
        self.take_it_out_of_the_queue(repository, number, who)
            .await?;
        Ok(())
    }

    async fn something_about_the_code_changed(&self, payload: &Value) -> Result<(), EngineError> {
        let Some(repository) = self.repository_in(payload).await? else {
            return Ok(());
        };
        for queue in self.store.queues_of(repository.id).await? {
            self.tend_queue_soon(queue.id).await?;
        }
        Ok(())
    }

    async fn a_branch_moved(&self, payload: &Value) -> Result<(), EngineError> {
        let Some(repository) = self.repository_in(payload).await? else {
            return Ok(());
        };
        let Some(branch) = text(payload, "ref").and_then(|full| full.strip_prefix("refs/heads/"))
        else {
            return Ok(());
        };
        let Some(queue) = self
            .store
            .queues_of(repository.id)
            .await?
            .into_iter()
            .find(|queue| queue.branch == branch)
        else {
            return Ok(());
        };
        if let Some(after) = text(payload, "after") {
            self.store.note_where_the_base_is(queue.id, after).await?;
        }
        self.tend_queue_soon(queue.id).await?;
        Ok(())
    }

    async fn repository_in(&self, payload: &Value) -> Result<Option<Repository>, EngineError> {
        let Some(repository) = payload.get("repository") else {
            return Ok(None);
        };
        let (Some(id), Some(full_name)) = (
            repository.get("id").and_then(Value::as_i64),
            repository.get("full_name").and_then(Value::as_str),
        ) else {
            return Ok(None);
        };
        let Some((owner, name)) = full_name.split_once('/') else {
            return Ok(None);
        };
        let known = self.store.repository_by_id(id).await?;
        let known = match known {
            Some(known) => known,
            None => {
                let Some(installation) = payload
                    .get("installation")
                    .and_then(|installation| installation.get("id"))
                    .and_then(Value::as_i64)
                else {
                    return Ok(None);
                };
                self.store
                    .remember_installation(installation, owner)
                    .await?;
                self.store
                    .remember_repository(id, installation, owner, name)
                    .await?
            }
        };
        Ok((known.active && known.we_can_still_see_it()).then_some(known))
    }
}

fn text<'a>(payload: &'a Value, key: &str) -> Option<&'a str> {
    payload.get(key).and_then(Value::as_str)
}

fn label_in(payload: &Value) -> Option<&str> {
    payload
        .get("label")
        .and_then(|label| label.get("name"))
        .and_then(Value::as_str)
}

fn actor(payload: &Value) -> String {
    payload
        .get("sender")
        .and_then(|sender| sender.get("login"))
        .and_then(Value::as_str)
        .unwrap_or(US)
        .to_owned()
}
