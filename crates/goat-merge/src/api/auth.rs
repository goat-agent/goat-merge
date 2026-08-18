use axum::extract::{Query, State};
use axum::http::{HeaderMap, header};
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;
use serde_json::json;

use super::{Answer, COOKIE, Fault, Incident, cookie_in, set_cookie};
use crate::engine::Engine;
use crate::github::As;
use crate::store::sessions::Session;

pub(crate) const STATE_COOKIE: &str = "goat_merge_state";

pub(crate) fn a_state_to_come_back_with(
    settings: &crate::settings::Settings,
    to: &str,
) -> Result<Response, Fault> {
    let state = crate::store::sessions::fresh_token();
    let secure = settings.public_url.starts_with("https");
    let mut response = Redirect::to(&format!("{to}{state}")).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        set_cookie(STATE_COOKIE, &state, secure, 600)
            .parse()
            .map_err(|problem| Fault::Broken {
                incident: Incident::written_down("writing the sign-in cookie", &problem),
            })?,
    );
    Ok(response)
}

#[derive(Debug, Deserialize)]
pub struct CameBack {
    code: Option<String>,
    state: Option<String>,
    installation_id: Option<i64>,
}

pub async fn begin(State(engine): State<Engine>) -> Response {
    match starting_to_sign_in(&engine).await {
        Ok(response) => response,
        Err(fault) => fault.told_to_the_browser(),
    }
}

async fn starting_to_sign_in(engine: &Engine) -> Result<Response, Fault> {
    if let Err(problem) = engine.github.who_we_are().await {
        return Err(if problem.is_missing() {
            Fault::AppIsGone
        } else {
            problem.into()
        });
    }
    let app = engine
        .store
        .app_credentials()
        .await?
        .ok_or(Fault::NotSetUp)?;
    a_state_to_come_back_with(
        &engine.settings,
        &format!(
            "{}/login/oauth/authorize?client_id={}&redirect_uri={}&state=",
            engine.settings.github_web,
            app.client_id,
            engine.settings.callback_url()
        ),
    )
}

pub async fn callback(
    State(engine): State<Engine>,
    headers: HeaderMap,
    Query(came_back): Query<CameBack>,
) -> Response {
    match coming_back_from_github(&engine, &headers, came_back).await {
        Ok(response) => response,
        Err(fault) => fault.told_to_the_browser(),
    }
}

async fn coming_back_from_github(
    engine: &Engine,
    headers: &HeaderMap,
    came_back: CameBack,
) -> Result<Response, Fault> {
    let Some(code) = came_back.code else {
        return Err(Fault::GithubRefused {
            said: "it sent no code back, so the sign-in could not be finished".to_owned(),
        });
    };
    let just_installed = came_back.installation_id;
    let Some(state) = came_back.state else {
        return Err(Fault::DidNotStartHere);
    };
    if cookie_in(headers, STATE_COOKIE).as_deref() != Some(state.as_str()) {
        return Err(Fault::DidNotStartHere);
    }
    let app = engine
        .store
        .app_credentials()
        .await?
        .ok_or(Fault::NotSetUp)?;

    #[derive(Deserialize)]
    struct Granted {
        access_token: Option<String>,
        error_description: Option<String>,
    }
    let granted: Granted = engine
        .github
        .http()
        .post(format!(
            "{}/login/oauth/access_token",
            engine.settings.github_web
        ))
        .header("accept", "application/json")
        .json(&json!({
            "client_id": app.client_id,
            "client_secret": app.client_secret,
            "code": code,
            "redirect_uri": engine.settings.callback_url(),
        }))
        .send()
        .await
        .map_err(|_| Fault::GithubIsUnreachable)?
        .json()
        .await
        .map_err(|problem| Fault::Broken {
            incident: Incident::written_down("reading GitHub's answer to the sign-in", &problem),
        })?;

    let Some(access_token) = granted.access_token else {
        return Err(Fault::GithubRefused {
            said: granted
                .error_description
                .unwrap_or_else(|| "it would not grant a token".to_owned()),
        });
    };

    #[derive(Deserialize)]
    struct Who {
        login: String,
    }
    let who: Who = engine
        .github
        .http()
        .get(format!("{}/user", engine.settings.github_api))
        .bearer_auth(&access_token)
        .header("accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|_| Fault::GithubIsUnreachable)?
        .json()
        .await
        .map_err(|problem| Fault::Broken {
            incident: Incident::written_down("reading who signed in", &problem),
        })?;

    let token = engine
        .store
        .start_session(&who.login, &access_token)
        .await?;
    let secure = engine.settings.public_url.starts_with("https");
    let landing = match just_installed {
        Some(installation) => where_to_after_installing(engine, installation, &who.login).await,
        None => "/".to_owned(),
    };
    let mut response = Redirect::to(&landing).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        set_cookie(COOKIE, &token, secure, 60 * 60 * 24 * 30)
            .parse()
            .map_err(|problem| Fault::Broken {
                incident: Incident::written_down("writing the session cookie", &problem),
            })?,
    );
    Ok(response)
}

async fn where_to_after_installing(engine: &Engine, installation: i64, login: &str) -> String {
    let Ok(found) = engine.github.repositories_of(As(installation)).await else {
        return "/".to_owned();
    };
    let mut remembered = Vec::new();
    for repository in found {
        let Some((owner, name)) = repository.full_name.split_once('/') else {
            continue;
        };
        if engine
            .store
            .remember_installation(installation, owner)
            .await
            .is_err()
        {
            continue;
        }
        if engine
            .store
            .remember_repository(repository.id, installation, owner, name)
            .await
            .is_ok()
        {
            remembered.push(repository.full_name.clone());
        }
    }
    let _ = engine
        .store
        .write_down(
            login,
            "installed the app",
            None,
            None,
            &remembered.join(", "),
        )
        .await;
    match remembered.as_slice() {
        [only] => format!("/settings/{only}"),
        _ => "/".to_owned(),
    }
}

pub async fn logout(State(engine): State<Engine>, headers: HeaderMap) -> Result<Response, Fault> {
    if let Some(token) = cookie_in(&headers, COOKIE) {
        if let Some(session) = engine.store.session(&token).await? {
            engine.standing.forget(&session.login);
        }
        engine.store.end_session(&token).await?;
    }
    let secure = engine.settings.public_url.starts_with("https");
    let mut response = Answer(json!({ "ok": true })).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        set_cookie(COOKIE, "", secure, 0)
            .parse()
            .map_err(|problem| Fault::Broken {
                incident: Incident::written_down("clearing the session cookie", &problem),
            })?,
    );
    Ok(response)
}

pub async fn token(State(engine): State<Engine>, headers: HeaderMap) -> Result<Response, Fault> {
    whoever_is_asking(&engine, &headers).await?;
    let token = cookie_in(&headers, COOKIE).ok_or(Fault::NotSignedIn)?;
    Ok(Answer(json!({ "token": token, "url": engine.settings.public_url })).into_response())
}

pub async fn me(State(engine): State<Engine>, headers: HeaderMap) -> Result<Response, Fault> {
    let session = whoever_is_asking(&engine, &headers).await?;
    Ok(Answer(json!({ "login": session.login })).into_response())
}

pub async fn whoever_is_asking(engine: &Engine, headers: &HeaderMap) -> Result<Session, Fault> {
    let token = cookie_in(headers, COOKIE).ok_or(Fault::NotSignedIn)?;
    engine
        .store
        .session(&token)
        .await?
        .ok_or(Fault::NotSignedIn)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Standing {
    Read,
    Write,
    Admin,
}

impl std::fmt::Display for Standing {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Admin => "admin",
        })
    }
}

pub async fn what_this_person_may_do(
    engine: &Engine,
    who: As,
    repository: &str,
    login: &str,
) -> Result<Option<Standing>, Fault> {
    if let Some(remembered) = engine.standing.recently(login, repository) {
        return Ok(remembered);
    }
    let permission = match engine
        .github
        .what_may_this_person_do(who, repository, login)
        .await
    {
        Ok(permission) => permission,
        Err(problem) if problem.is_missing() => {
            engine.standing.remember(login, repository, None);
            return Ok(None);
        }
        Err(problem) => return Err(problem.into()),
    };
    let standing = match permission.as_str() {
        "admin" | "maintain" => Some(Standing::Admin),
        "write" => Some(Standing::Write),
        "read" | "triage" => Some(Standing::Read),
        _ => None,
    };
    engine.standing.remember(login, repository, standing);
    Ok(standing)
}

pub async fn must_be_at_least(
    engine: &Engine,
    headers: &HeaderMap,
    who: As,
    repository: &str,
    wanted: Standing,
) -> Result<String, Fault> {
    let session = whoever_is_asking(engine, headers).await?;
    let standing = what_this_person_may_do(engine, who, repository, &session.login).await?;
    if standing.is_none_or(|standing| standing < wanted) {
        let (owner, name) = repository.split_once('/').unwrap_or((repository, ""));
        return Err(Fault::NotAllowed {
            login: session.login,
            needs: wanted,
            owner: owner.to_owned(),
            name: name.to_owned(),
        });
    }
    Ok(session.login)
}
