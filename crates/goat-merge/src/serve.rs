use std::sync::Arc;

use axum::Router;
use tokio::signal;

use crate::api;
use crate::engine::Engine;
use crate::github::{AppAuth, Github};
use crate::settings::Settings;
use crate::store::{Seal, Store};
use crate::web;

pub async fn run(settings: Settings) -> Result<(), Box<dyn std::error::Error>> {
    let settings = Arc::new(settings);
    let store = Store::open(&settings.database_url, Seal::holding(settings.master_key)).await?;
    let github = Github::new(&settings.github_api)?;

    match store.app_credentials().await {
        Ok(Some(app)) => {
            github.adopt(AppAuth::holding(app.app_id, &app.private_key)?);
            tracing::info!(app = %app.slug, "the GitHub App is ready");
        }
        Ok(None) => tracing::warn!(
            setup = %format!("{}/setup", settings.public_url),
            "no GitHub App yet"
        ),
        Err(problem) => return Err(problem.into()),
    }

    let engine = Engine::new(store, github, Arc::clone(&settings));
    engine.catch_the_app_up_with_our_address().await;

    let working = engine.clone();
    tokio::spawn(async move { working.work_until_stopped().await });

    let sweeping = engine.clone();
    tokio::spawn(async move {
        loop {
            match sweeping.store.every_queue().await {
                Ok(queues) => {
                    for queue in queues {
                        if let Err(problem) = sweeping.tend_queue_soon(queue.id).await {
                            tracing::warn!(%problem, "a queue could not be scheduled");
                        }
                    }
                }
                Err(problem) => tracing::warn!(%problem, "the queues could not be listed"),
            }
            match sweeping
                .store
                .forget_deliveries_older_than(time::Duration::days(7))
                .await
            {
                Ok(0) => {}
                Ok(gone) => tracing::debug!(gone, "old webhook deliveries were forgotten"),
                Err(problem) => tracing::warn!(%problem, "old deliveries could not be forgotten"),
            }
            tokio::time::sleep(std::time::Duration::from_secs(120)).await;
        }
    });

    let app = Router::new()
        .merge(api::router())
        .fallback(web::serve)
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .with_state(engine);

    let listener = tokio::net::TcpListener::bind(settings.listen).await?;
    tracing::info!(
        listening = %settings.listen,
        public = %settings.public_url,
        "goat-merge is up"
    );
    axum::serve(listener, app)
        .with_graceful_shutdown(stopped())
        .await?;
    Ok(())
}

async fn stopped() {
    let interrupt = async {
        let _ = signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) = signal::unix::signal(signal::unix::SignalKind::terminate()) {
            signal.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = interrupt => {},
        () = terminate => {},
    }
    tracing::info!("goat-merge is stopping");
}
