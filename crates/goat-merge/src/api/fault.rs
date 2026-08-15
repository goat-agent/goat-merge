use std::fmt::Display;

use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::extract::{FromRequest, FromRequestParts, Request};
use axum::http::request::Parts;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::de::DeserializeOwned;
use serde_json::json;

use super::auth::Standing;
use crate::engine::EngineError;
use crate::github::GithubError;
use crate::store::StoreError;

pub struct Incident(String);

impl Incident {
    pub fn written_down(doing: &'static str, detail: &dyn Display) -> Self {
        let number: String = crate::store::sessions::fresh_token()
            .chars()
            .take(8)
            .collect();
        tracing::error!(incident = %number, doing, detail = %detail, "a request could not be answered");
        Self(number)
    }
}

impl std::fmt::Debug for Incident {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(&self.0)
    }
}

#[derive(Debug)]
pub enum Fault {
    NotSignedIn,
    NotSetUp,
    AppIsGone,
    NotInstalledHere {
        owner: String,
        name: String,
    },
    NotAllowed {
        login: String,
        needs: Standing,
        owner: String,
        name: String,
    },
    NoQueueHere {
        owner: String,
        name: String,
        branch: String,
    },
    NeverQueued {
        owner: String,
        name: String,
        number: i32,
    },
    QueueIsOff {
        owner: String,
        name: String,
    },
    GithubRefused {
        said: String,
    },
    GithubIsRateLimiting {
        wait: Option<u64>,
    },
    GithubIsUnreachable,
    ConfigurationIsInvalid {
        owner: String,
        name: String,
        branch: String,
        problem: String,
    },
    ReasonRequired,
    NoSuchEndpoint {
        incident: Incident,
    },
    MalformedRequest {
        incident: Incident,
    },
    Broken {
        incident: Incident,
    },
}

impl Fault {
    fn kind(&self) -> &'static str {
        match self {
            Self::NotSignedIn => "not_signed_in",
            Self::NotSetUp => "not_set_up",
            Self::AppIsGone => "app_is_gone",
            Self::NotInstalledHere { .. } => "not_installed_here",
            Self::NotAllowed { .. } => "not_allowed",
            Self::NoQueueHere { .. } => "no_queue_here",
            Self::NeverQueued { .. } => "never_queued",
            Self::QueueIsOff { .. } => "queue_is_off",
            Self::GithubRefused { .. } => "github_refused",
            Self::GithubIsRateLimiting { .. } => "github_is_rate_limiting",
            Self::GithubIsUnreachable => "github_is_unreachable",
            Self::ConfigurationIsInvalid { .. } => "configuration_is_invalid",
            Self::ReasonRequired => "reason_required",
            Self::NoSuchEndpoint { .. } => "no_such_endpoint",
            Self::MalformedRequest { .. } => "malformed_request",
            Self::Broken { .. } => "broken",
        }
    }

    fn status(&self) -> StatusCode {
        match self {
            Self::NotSignedIn => StatusCode::UNAUTHORIZED,
            Self::NotAllowed { .. } => StatusCode::FORBIDDEN,
            Self::NotInstalledHere { .. }
            | Self::NoQueueHere { .. }
            | Self::NeverQueued { .. }
            | Self::NoSuchEndpoint { .. } => StatusCode::NOT_FOUND,
            Self::QueueIsOff { .. } | Self::GithubRefused { .. } => StatusCode::CONFLICT,
            Self::ConfigurationIsInvalid { .. } | Self::ReasonRequired => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            Self::MalformedRequest { .. } => StatusCode::BAD_REQUEST,
            Self::NotSetUp | Self::AppIsGone | Self::GithubIsRateLimiting { .. } => {
                StatusCode::SERVICE_UNAVAILABLE
            }
            Self::GithubIsUnreachable => StatusCode::BAD_GATEWAY,
            Self::Broken { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn sentence(&self) -> String {
        match self {
            Self::NotSignedIn => "Your session has ended. Sign in with GitHub again and you will \
                                  come straight back here."
                .to_owned(),
            Self::NotSetUp => "This server has no GitHub App yet, so there is nothing to show. \
                               Create one and install it on a repository to get started."
                .to_owned(),
            Self::AppIsGone => "The GitHub App this server was set up with no longer exists on \
                                GitHub. Nobody can sign in and nothing will merge until a new one \
                                is created here."
                .to_owned(),
            Self::NotInstalledHere { owner, name } => format!(
                "The Merge Queue App is not installed on {owner}/{name}, so this server cannot \
                 see it. Install it on GitHub and this page will fill in."
            ),
            Self::NotAllowed {
                login,
                needs,
                owner,
                name,
            } => format!(
                "{login} needs {needs} access to {owner}/{name} to do that. Ask somebody who \
                 administers the repository."
            ),
            Self::NoQueueHere {
                owner,
                name,
                branch,
            } => format!(
                "There is no queue for {branch} in {owner}/{name}. Turn one on in Settings, or \
                 pick a branch that already has one."
            ),
            Self::NeverQueued {
                owner,
                name,
                number,
            } => format!(
                "#{number} has never been in a queue in {owner}/{name}. Add the {} label to it \
                 on GitHub and it will appear here.",
                goat_merge_core::config::LABEL
            ),
            Self::QueueIsOff { owner, name } => format!(
                "The queue is switched off for {owner}/{name}, so nothing can go through it. \
                 Turn it back on in Settings."
            ),
            Self::GithubRefused { said } => format!("GitHub would not do that. It said: {said}"),
            Self::GithubIsRateLimiting { wait } => {
                let when = match wait {
                    Some(seconds) if *seconds > 90 => {
                        format!("It clears in about {} minutes", seconds / 60)
                    }
                    Some(_) => "It clears in under a minute".to_owned(),
                    None => "It usually clears within a few minutes".to_owned(),
                };
                format!(
                    "GitHub is rate limiting this server, so it cannot read from GitHub at the \
                     moment. {when}, and nothing in the queue has been lost."
                )
            }
            Self::GithubIsUnreachable => "GitHub could not be reached from this server. Nothing \
                                          in the queue has been lost — it carries on from where \
                                          it stopped as soon as GitHub answers."
                .to_owned(),
            Self::ConfigurationIsInvalid {
                branch, problem, ..
            } => format!(
                "{} {problem}. Fix it on {branch} and the queue will pick the change up on its \
                 next look.",
                goat_merge_core::config::FILE
            ),
            Self::ReasonRequired => "Expediting moves other people's work down the queue, so it \
                                     needs a reason. It is written into the log next to your name."
                .to_owned(),
            Self::NoSuchEndpoint { incident } => format!(
                "This server has no such endpoint. The console and the server are probably \
                 different versions — reload the page, and quote {incident:?} if it keeps \
                 happening."
            ),
            Self::MalformedRequest { incident } => format!(
                "The console asked this server for something it could not read. That is a bug in \
                 goat-merge, not something you did. Quote {incident:?} if you report it."
            ),
            Self::Broken { incident } => format!(
                "Something went wrong inside this server. Nothing was changed. Quote \
                 {incident:?} if you report it."
            ),
        }
    }

    fn somewhere_to_go(&self) -> Option<String> {
        match self {
            Self::NotSignedIn => Some("/auth/github".to_owned()),
            Self::NotSetUp | Self::AppIsGone => Some("/setup".to_owned()),
            Self::NotInstalledHere { owner, name } => Some(format!(
                "https://github.com/{owner}/{name}/settings/installations"
            )),
            Self::NoQueueHere { owner, name, .. } | Self::QueueIsOff { owner, name } => {
                Some(format!("/settings/{owner}/{name}"))
            }
            Self::NeverQueued {
                owner,
                name,
                number,
            } => Some(format!("https://github.com/{owner}/{name}/pull/{number}")),
            Self::ConfigurationIsInvalid {
                owner,
                name,
                branch,
                ..
            } => Some(format!(
                "https://github.com/{owner}/{name}/blob/{branch}/{}",
                goat_merge_core::config::FILE
            )),
            _ => None,
        }
    }

    pub fn told_to_the_browser(self) -> Response {
        let to = format!(
            "/?trouble={}&said={}",
            self.kind(),
            escaped(&self.sentence())
        );
        axum::response::Redirect::to(&to).into_response()
    }

    fn incident(&self) -> Option<&str> {
        match self {
            Self::NoSuchEndpoint { incident }
            | Self::MalformedRequest { incident }
            | Self::Broken { incident } => Some(&incident.0),
            _ => None,
        }
    }
}

impl IntoResponse for Fault {
    fn into_response(self) -> Response {
        let body = json!({
            "error": self.sentence(),
            "kind": self.kind(),
            "where": self.somewhere_to_go(),
            "incident": self.incident(),
        });
        let mut response = (self.status(), axum::Json(body)).into_response();
        if let Self::GithubIsRateLimiting { wait: Some(wait) } = self
            && let Ok(seconds) = HeaderValue::from_str(&wait.to_string())
        {
            response.headers_mut().insert(header::RETRY_AFTER, seconds);
        }
        response
    }
}

impl From<StoreError> for Fault {
    fn from(problem: StoreError) -> Self {
        Self::Broken {
            incident: Incident::written_down("reading or writing the database", &problem),
        }
    }
}

impl From<GithubError> for Fault {
    fn from(problem: GithubError) -> Self {
        match problem {
            GithubError::NoAppYet => Self::NotSetUp,
            GithubError::Unreachable { .. } => Self::GithubIsUnreachable,
            GithubError::RateLimited { wait } => Self::GithubIsRateLimiting { wait },
            GithubError::Refused {
                said: Some(said), ..
            } => Self::GithubRefused { said },
            other => Self::Broken {
                incident: Incident::written_down("talking to GitHub", &other),
            },
        }
    }
}

impl From<EngineError> for Fault {
    fn from(problem: EngineError) -> Self {
        match problem {
            EngineError::Store(problem) => problem.into(),
            EngineError::Github(problem) => problem.into(),
            other => Self::Broken {
                incident: Incident::written_down("keeping the queue moving", &other),
            },
        }
    }
}

fn escaped(sentence: &str) -> String {
    let mut out = String::with_capacity(sentence.len());
    for byte in sentence.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

pub struct Sent<T>(pub T);

impl<S, T> FromRequest<S> for Sent<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Fault;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        axum::Json::<T>::from_request(request, state)
            .await
            .map(|axum::Json(given)| Self(given))
            .map_err(|refused: JsonRejection| Fault::MalformedRequest {
                incident: Incident::written_down("reading the request body", &refused),
            })
    }
}

pub struct Named<T>(pub T);

impl<S, T> FromRequestParts<S> for Named<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = Fault;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        axum::extract::Path::<T>::from_request_parts(parts, state)
            .await
            .map(|axum::extract::Path(given)| Self(given))
            .map_err(|refused: PathRejection| Fault::MalformedRequest {
                incident: Incident::written_down("reading the address", &refused),
            })
    }
}

pub struct Wanting<T>(pub T);

impl<S, T> FromRequestParts<S> for Wanting<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = Fault;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        axum::extract::Query::<T>::from_request_parts(parts, state)
            .await
            .map(|axum::extract::Query(given)| Self(given))
            .map_err(|refused: QueryRejection| Fault::MalformedRequest {
                incident: Incident::written_down("reading the query", &refused),
            })
    }
}
