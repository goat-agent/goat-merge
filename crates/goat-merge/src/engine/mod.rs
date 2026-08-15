pub mod react;
pub mod tend;

use std::sync::Arc;

use time::Duration;
use tokio::sync::broadcast;

use crate::github::{Github, GithubError};
use crate::settings::Settings;
use crate::store::{Store, StoreError};

pub const TEND: &str = "tend";
pub const IDLE: std::time::Duration = std::time::Duration::from_millis(500);
pub const HELD_TOO_LONG: Duration = Duration::minutes(10);
pub const LOOK_AGAIN_IN: Duration = Duration::seconds(45);

#[derive(Clone)]
pub struct Engine {
    pub store: Store,
    pub github: Github,
    pub settings: Arc<Settings>,
    pub news: broadcast::Sender<String>,
    pub standing: crate::api::standing::WhoMaySeeWhat,
}

impl Engine {
    pub fn new(store: Store, github: Github, settings: Arc<Settings>) -> Self {
        let (news, _) = broadcast::channel(256);
        Self {
            store,
            github,
            settings,
            news,
            standing: crate::api::standing::WhoMaySeeWhat::default(),
        }
    }

    async fn write_off(&self, queue_id: i64, problem: &EngineError) {
        let Ok(Some(queue)) = self.store.queue_by_id(queue_id).await else {
            return;
        };
        let _ = self
            .store
            .write_down(
                crate::store::audit::US,
                "stopped trying for now",
                Some(queue.repository_id),
                None,
                &problem.to_string(),
            )
            .await;
    }

    pub async fn catch_the_app_up_with_our_address(&self) {
        if !self.github.is_set_up() {
            return;
        }
        let ours = self.settings.webhook_url();
        let registered = match self.github.where_our_webhook_points().await {
            Ok(registered) => registered,
            Err(problem) if problem.is_missing() => {
                tracing::error!(
                    setup = %format!("{}/setup", self.settings.public_url),
                    "the GitHub App this server was set up with no longer exists on GitHub"
                );
                return;
            }
            Err(problem) => {
                tracing::warn!(problem = %problem, "could not read the App's webhook address");
                return;
            }
        };
        if registered.as_deref() == Some(ours.as_str()) {
            return;
        }
        if let Err(problem) = self.github.point_our_webhook_at(&ours).await {
            tracing::error!(problem = %problem, webhook = %ours, "could not move the App's webhook");
            return;
        }
        let settings_page = match self.github.who_we_are().await {
            Ok(app) => app.where_its_settings_are(&self.settings.github_web),
            Err(_) => format!("{}/settings/apps", self.settings.github_web),
        };
        tracing::warn!(
            was = registered.unwrap_or_default(),
            webhook = %ours,
            callback = %self.settings.callback_url(),
            app = %settings_page,
            "this server has moved, so the App's webhook was moved with it. GitHub does not let a \
             server change its own sign-in address: add the callback URL to the App by hand or \
             nobody will be able to sign in"
        );
    }

    pub fn announce(&self, what: &str) {
        let _ = self.news.send(what.to_owned());
    }

    pub async fn tend_queue_soon(&self, queue_id: i64) -> Result<(), EngineError> {
        self.store.ask_for(TEND, &subject(queue_id)).await?;
        Ok(())
    }

    pub async fn work_until_stopped(self) {
        let worker = format!("worker-{}", std::process::id());
        loop {
            if let Err(problem) = self
                .store
                .release_jobs_held_since_before(HELD_TOO_LONG)
                .await
            {
                tracing::warn!(%problem, "could not release jobs from a worker that went away");
            }
            match self.store.claim_a_job(&worker).await {
                Ok(Some(job)) => {
                    let outcome = match job.kind.as_str() {
                        TEND => self.tend(queue_of(&job.subject)).await,
                        other => {
                            tracing::warn!(kind = other, "a job of an unknown kind was dropped");
                            Ok(())
                        }
                    };
                    match outcome {
                        Ok(()) => {
                            if let Err(problem) = self.store.job_is_done(job.id).await {
                                tracing::error!(%problem, "a finished job could not be cleared");
                            }
                        }
                        Err(problem) if !problem.is_worth_another_try() => {
                            tracing::warn!(%problem, subject = %job.subject, "a job was given up on");
                            self.write_off(queue_of(&job.subject), &problem).await;
                            if let Err(problem) = self.store.job_is_done(job.id).await {
                                tracing::error!(%problem, "a hopeless job could not be cleared");
                            }
                        }
                        Err(problem) => {
                            tracing::warn!(%problem, subject = %job.subject, "a job did not finish");
                            let wait = Duration::seconds(5 * i64::from(job.attempts.min(12)));
                            if let Err(problem) = self
                                .store
                                .job_went_wrong(job.id, &problem.to_string(), wait)
                                .await
                            {
                                tracing::error!(%problem, "a failed job could not be rescheduled");
                            }
                        }
                    }
                }
                Ok(None) => tokio::time::sleep(IDLE).await,
                Err(problem) => {
                    tracing::error!(%problem, "the job table could not be read");
                    tokio::time::sleep(IDLE).await;
                }
            }
        }
    }
}

pub fn subject(queue_id: i64) -> String {
    format!("queue:{queue_id}")
}

fn queue_of(subject: &str) -> i64 {
    subject
        .strip_prefix("queue:")
        .and_then(|id| id.parse().ok())
        .unwrap_or(-1)
}

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Github(#[from] GithubError),
    #[error("queue {0} is not there any more")]
    NoQueue(i64),
}

impl EngineError {
    pub fn is_worth_another_try(&self) -> bool {
        match self {
            Self::Github(problem) => problem.is_worth_another_try(),
            Self::Store(_) => true,
            Self::NoQueue(_) => false,
        }
    }
}
