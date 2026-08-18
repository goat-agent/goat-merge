pub mod auth;
pub mod calls;
pub mod look;
pub mod webhook;

use std::sync::{Arc, RwLock};

use reqwest::{Method, RequestBuilder, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::Value;
use time::OffsetDateTime;

pub use auth::AppAuth;

pub const USER_AGENT: &str = concat!("goat-merge/", env!("CARGO_PKG_VERSION"));

#[derive(Clone)]
pub struct Github {
    http: reqwest::Client,
    api: String,
    auth: Arc<RwLock<Option<Arc<AppAuth>>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct As(pub i64);

impl Github {
    pub fn new(api: impl Into<String>) -> Result<Self, GithubError> {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .map_err(|problem| GithubError::Unreachable {
                problem: problem.to_string(),
            })?;
        Ok(Self {
            http,
            api: api.into().trim_end_matches('/').to_owned(),
            auth: Arc::new(RwLock::new(None)),
        })
    }

    pub fn adopt(&self, auth: AppAuth) {
        if let Ok(mut held) = self.auth.write() {
            *held = Some(Arc::new(auth));
        }
    }

    pub fn auth(&self) -> Result<Arc<AppAuth>, GithubError> {
        self.auth
            .read()
            .ok()
            .and_then(|held| held.clone())
            .ok_or(GithubError::NoAppYet)
    }

    pub fn is_set_up(&self) -> bool {
        self.auth().is_ok()
    }

    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }

    async fn token_for(&self, installation: i64) -> Result<String, GithubError> {
        let auth = self.auth()?;
        if let Some(token) = auth.borrowed(installation) {
            return Ok(token);
        }
        let minting = format!("/app/installations/{installation}/access_tokens");
        let minted: calls::MintedToken = self
            .send(
                &minting,
                self.http
                    .request(
                        Method::POST,
                        format!(
                            "{}/app/installations/{installation}/access_tokens",
                            self.api
                        ),
                    )
                    .bearer_auth(auth.as_the_app()?),
            )
            .await?;
        auth.keep(installation, minted.token.clone(), minted.expires_at);
        Ok(minted.token)
    }

    pub async fn get<T: DeserializeOwned>(&self, who: As, path: &str) -> Result<T, GithubError> {
        self.call(who, Method::GET, path, None).await
    }

    pub async fn get_if_there<T: DeserializeOwned>(
        &self,
        who: As,
        path: &str,
    ) -> Result<Option<T>, GithubError> {
        match self.call(who, Method::GET, path, None).await {
            Ok(found) => Ok(Some(found)),
            Err(GithubError::Refused { status, .. }) if status == StatusCode::NOT_FOUND => Ok(None),
            Err(problem) => Err(problem),
        }
    }

    pub async fn post<T: DeserializeOwned>(
        &self,
        who: As,
        path: &str,
        body: Value,
    ) -> Result<T, GithubError> {
        self.call(who, Method::POST, path, Some(body)).await
    }

    pub async fn patch<T: DeserializeOwned>(
        &self,
        who: As,
        path: &str,
        body: Value,
    ) -> Result<T, GithubError> {
        self.call(who, Method::PATCH, path, Some(body)).await
    }

    pub async fn put<T: DeserializeOwned>(
        &self,
        who: As,
        path: &str,
        body: Value,
    ) -> Result<T, GithubError> {
        self.call(who, Method::PUT, path, Some(body)).await
    }

    pub async fn delete(&self, who: As, path: &str) -> Result<(), GithubError> {
        let _: Ignored = self.call(who, Method::DELETE, path, None).await?;
        Ok(())
    }

    async fn call<T: DeserializeOwned>(
        &self,
        who: As,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<T, GithubError> {
        let token = self.token_for(who.0).await?;
        self.send(path, self.asking(&token, method, path, body))
            .await
    }

    pub(crate) async fn as_ourselves<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<T, GithubError> {
        let token = self.auth()?.as_the_app()?;
        self.send(path, self.asking(&token, method, path, body))
            .await
    }

    fn asking(
        &self,
        token: &str,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> RequestBuilder {
        let mut request = self
            .http
            .request(method, format!("{}{path}", self.api))
            .bearer_auth(token)
            .header("accept", "application/vnd.github+json")
            .header("x-github-api-version", "2022-11-28");
        if let Some(body) = body {
            request = request.json(&body);
        }
        request
    }

    async fn send<T: DeserializeOwned>(
        &self,
        asking: &str,
        request: RequestBuilder,
    ) -> Result<T, GithubError> {
        let mut wait_before_trying_again = ONE_MORE_TIME;
        loop {
            let Some(again) = request.try_clone() else {
                return self.send_once(asking, request).await;
            };
            match self.send_once(asking, again).await {
                Err(GithubError::Refused { status, .. })
                    if status.is_server_error() && !wait_before_trying_again.is_empty() =>
                {
                    let (pause, rest) = wait_before_trying_again.split_at(1);
                    wait_before_trying_again = rest;
                    tokio::time::sleep(std::time::Duration::from_millis(pause[0])).await;
                }
                answer => return answer,
            }
        }
    }

    async fn send_once<T: DeserializeOwned>(
        &self,
        asking: &str,
        request: RequestBuilder,
    ) -> Result<T, GithubError> {
        let response = request
            .send()
            .await
            .map_err(|problem| GithubError::Unreachable {
                problem: problem.to_string(),
            })?;
        let status = response.status();
        let held_off = out_of_requests(&response);
        let wait = how_long_to_wait(&response);
        let body = response.text().await.unwrap_or_default();
        if held_off {
            return Err(GithubError::RateLimited { wait });
        }
        if !status.is_success() {
            return Err(GithubError::Refused {
                asking: asking.to_owned(),
                said: what_github_said(&body),
                body,
                status,
            });
        }
        if body.trim().is_empty() {
            return serde_json::from_str("null").map_err(|problem| GithubError::Unreadable {
                problem: problem.to_string(),
                body: String::new(),
            });
        }
        serde_json::from_str(&body).map_err(|problem| GithubError::Unreadable {
            problem: problem.to_string(),
            body: body.chars().take(400).collect(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ignored;

impl<'de> serde::Deserialize<'de> for Ignored {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        serde::de::IgnoredAny::deserialize(deserializer).map(|_| Self)
    }
}

fn out_of_requests(response: &reqwest::Response) -> bool {
    if response.status() == StatusCode::TOO_MANY_REQUESTS {
        return true;
    }
    response.status() == StatusCode::FORBIDDEN
        && response
            .headers()
            .get("x-ratelimit-remaining")
            .and_then(|left| left.to_str().ok())
            == Some("0")
}

fn how_long_to_wait(response: &reqwest::Response) -> Option<u64> {
    let headers = response.headers();
    if let Some(after) = headers
        .get("retry-after")
        .and_then(|after| after.to_str().ok())
        .and_then(|after| after.parse().ok())
    {
        return Some(after);
    }
    let resets_at: i64 = headers
        .get("x-ratelimit-reset")
        .and_then(|at| at.to_str().ok())
        .and_then(|at| at.parse().ok())?;
    let now = OffsetDateTime::now_utc().unix_timestamp();
    u64::try_from(resets_at - now).ok()
}

fn what_github_said(body: &str) -> Option<String> {
    let parsed = serde_json::from_str::<Value>(body).ok()?;
    let headline = parsed.get("message").and_then(Value::as_str)?;
    let reasons: Vec<String> = parsed
        .get("errors")
        .and_then(Value::as_array)
        .map(|errors| errors.iter().filter_map(why).collect())
        .unwrap_or_default();
    Some(if reasons.is_empty() {
        headline.to_owned()
    } else {
        format!("{headline} — {}", reasons.join("; "))
    })
}

fn why(error: &Value) -> Option<String> {
    if let Some(said) = error.get("message").and_then(Value::as_str) {
        return Some(said.to_owned());
    }
    let field = error.get("field").and_then(Value::as_str)?;
    let code = error.get("code").and_then(Value::as_str)?;
    Some(format!("{field} {code}"))
}

#[derive(Debug, thiserror::Error)]
pub enum GithubError {
    #[error("GitHub could not be reached: {problem}")]
    Unreachable { problem: String },
    #[error("GitHub is rate limiting this server")]
    RateLimited { wait: Option<u64> },
    #[error("GitHub answered {status} to {asking}: {body}")]
    Refused {
        status: StatusCode,
        asking: String,
        said: Option<String>,
        body: String,
    },
    #[error("GitHub sent something this version cannot read ({problem}): {body}")]
    Unreadable { problem: String, body: String },
    #[error("the GitHub App private key could not be used to sign a request: {problem}")]
    PrivateKeyUnusable { problem: String },
    #[error("goat-merge has no GitHub App yet. Open /setup and create one")]
    NoAppYet,
}

const ONE_MORE_TIME: &[u64] = &[250, 1_000];

impl GithubError {
    pub fn is_conflict(&self) -> bool {
        matches!(self, Self::Refused { status, .. } if *status == StatusCode::CONFLICT)
    }

    pub fn says_it_is_already_there(&self) -> bool {
        match self {
            Self::Refused {
                status, said, body, ..
            } => {
                *status == StatusCode::CONFLICT
                    || (*status == StatusCode::UNPROCESSABLE_ENTITY && {
                        let words = said.as_deref().unwrap_or(body).to_lowercase();
                        words.contains("already exist") || words.contains("already_exist")
                    })
            }
            _ => false,
        }
    }

    pub fn is_missing(&self) -> bool {
        matches!(self, Self::Refused { status, .. } if *status == StatusCode::NOT_FOUND)
    }

    pub fn is_worth_another_try(&self) -> bool {
        match self {
            Self::Unreachable { .. } | Self::RateLimited { .. } => true,
            Self::Refused { status, .. } => {
                status.is_server_error() || *status == StatusCode::REQUEST_TIMEOUT
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{GithubError, StatusCode, what_github_said};

    fn refusing(body: &str) -> GithubError {
        GithubError::Refused {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            asking: "/repos/acme/api/pulls".to_owned(),
            said: what_github_said(body),
            body: body.to_owned(),
        }
    }

    #[test]
    fn a_refusal_carries_the_reason_github_gave_not_just_validation_failed() {
        let said = what_github_said(
            r#"{"message":"Validation Failed","errors":[{"resource":"PullRequest","code":"custom","message":"No commits between main and merge-queue/setup"}]}"#,
        );

        assert!(
            said.is_some_and(|said| said.contains("No commits between")),
            "the headline alone tells the reader nothing they can act on, and this one is the \
             whole answer"
        );
    }

    #[test]
    fn an_answer_github_did_not_write_is_not_repeated_to_anybody() {
        assert_eq!(
            what_github_said("<html><body>502 Bad Gateway</body></html>"),
            None
        );
        assert_eq!(what_github_said(r#"{"unexpected":"shape"}"#), None);
    }

    #[test]
    fn github_says_a_label_is_already_there_with_a_422_not_a_409() {
        let refused = refusing(
            r#"{"message":"Validation Failed","errors":[{"resource":"Label","code":"already_exists","field":"name"}]}"#,
        );

        assert!(
            refused.says_it_is_already_there(),
            "setting a repository up twice must be as harmless as doing it once, and the label \
             and the setup branch are both already there the second time"
        );
    }

    #[test]
    fn a_branch_that_is_already_there_is_not_a_failure_either() {
        assert!(refusing(r#"{"message":"Reference already exists"}"#).says_it_is_already_there());
    }

    #[test]
    fn a_real_rejection_is_still_a_failure() {
        assert!(
            !refusing(
                r#"{"message":"Validation Failed","errors":[{"message":"No commits between main and merge-queue/setup"}]}"#,
            )
            .says_it_is_already_there()
        );
    }

    #[test]
    fn a_refusal_with_no_details_reads_as_the_headline_alone() {
        assert_eq!(
            what_github_said(r#"{"message":"Not Found"}"#),
            Some("Not Found".to_owned())
        );
    }
}
