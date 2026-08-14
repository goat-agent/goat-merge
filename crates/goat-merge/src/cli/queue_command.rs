use serde_json::{Value, json};

use super::Which;
use super::client::{Server, Trouble, branch_here, repository_here};

pub async fn login(url: &str, token: Option<String>) -> Result<(), Trouble> {
    let url = url.trim_end_matches('/').to_owned();
    let token = match token {
        Some(token) => token,
        None => {
            println!("Open this in a browser and sign in with GitHub:\n");
            println!("    {url}/auth/github\n");
            println!("Then open {url}/token, copy the token it shows, and paste it here.\n");
            print!("Token: ");
            use std::io::Write;
            let _ = std::io::stdout().flush();
            let mut typed = String::new();
            std::io::stdin()
                .read_line(&mut typed)
                .map_err(|problem| Trouble::CannotRead {
                    path: "the terminal".to_owned(),
                    problem: problem.to_string(),
                })?;
            typed.trim().to_owned()
        }
    };
    let remembered = super::client::Remembered { url, token };
    super::client::remember(&remembered)?;
    let server = Server::we_know()?;
    let me = server.get("/api/me").await?;
    println!(
        "Signed in to {} as {}",
        server.url(),
        me.get("login").and_then(Value::as_str).unwrap_or("someone")
    );
    Ok(())
}

pub async fn show_queue(which: &Which) -> Result<(), Trouble> {
    let server = Server::we_know()?;
    let repository = repository_here(which.repo.as_deref())?;
    let branch = which_branch(&server, &repository, which.branch.clone()).await?;
    let queue = server
        .get(&format!("/api/queue/{repository}/{branch}"))
        .await?;
    if which.json {
        println!("{queue:#}");
        return Ok(());
    }

    let paused = queue
        .get("paused")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    println!(
        "{repository} · {branch} · {}",
        if paused { "paused" } else { "active" }
    );
    let rows = queue
        .get("entries")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if rows.is_empty() {
        println!("  the queue is empty");
        return Ok(());
    }
    for (at, row) in rows.iter().enumerate() {
        println!(
            "  {:>2}. #{:<6} {:<10} {}",
            at + 1,
            row.get("pull_request")
                .and_then(Value::as_i64)
                .unwrap_or_default(),
            row.get("status").and_then(Value::as_str).unwrap_or(""),
            row.get("title").and_then(Value::as_str).unwrap_or("")
        );
        if let Some(detail) = row.get("detail").and_then(Value::as_str)
            && !detail.is_empty()
        {
            println!("      {detail}");
        }
    }
    Ok(())
}

pub async fn hold(which: &Which, paused: bool) -> Result<(), Trouble> {
    let server = Server::we_know()?;
    let repository = repository_here(which.repo.as_deref())?;
    let branch = which_branch(&server, &repository, which.branch.clone()).await?;
    let what = if paused { "pause" } else { "resume" };
    server
        .post(
            &format!("/api/queue/{repository}/{branch}/{what}"),
            json!({}),
        )
        .await?;
    println!(
        "{repository} · {branch} is now {}",
        if paused { "paused" } else { "active" }
    );
    Ok(())
}

pub async fn act_on(which: &Which, number: Option<i32>, what: &str) -> Result<(), Trouble> {
    let server = Server::we_know()?;
    let repository = repository_here(which.repo.as_deref())?;
    let number = which_pull_request(&server, &repository, number).await?;
    server
        .post(
            &format!("/api/pull/{repository}/{number}/{what}"),
            json!({}),
        )
        .await?;
    println!("#{number} · {what}");
    Ok(())
}

pub async fn expedite(which: &Which, number: i32, reason: &str) -> Result<(), Trouble> {
    let server = Server::we_know()?;
    let repository = repository_here(which.repo.as_deref())?;
    server
        .post(
            &format!("/api/pull/{repository}/{number}/expedite"),
            json!({ "reason": reason }),
        )
        .await?;
    println!("#{number} moved to the front · {reason}");
    Ok(())
}

pub async fn explain(which: &Which, number: Option<i32>) -> Result<(), Trouble> {
    let server = Server::we_know()?;
    let repository = repository_here(which.repo.as_deref())?;
    let number = which_pull_request(&server, &repository, number).await?;
    let said = server
        .get(&format!("/api/pull/{repository}/{number}"))
        .await?;
    if which.json {
        println!("{said:#}");
        return Ok(());
    }
    let entry = said.get("entry").cloned().unwrap_or(Value::Null);
    println!(
        "#{number} {}",
        entry.get("title").and_then(Value::as_str).unwrap_or("")
    );
    println!(
        "  {} — {}",
        entry.get("status").and_then(Value::as_str).unwrap_or(""),
        entry.get("detail").and_then(Value::as_str).unwrap_or("")
    );
    if let Some(attempt) = entry.get("attempt").filter(|value| !value.is_null()) {
        println!(
            "  verifying {} on {} · {}",
            attempt.get("head").and_then(Value::as_str).unwrap_or(""),
            attempt.get("base").and_then(Value::as_str).unwrap_or(""),
            attempt
                .get("conclusion")
                .and_then(Value::as_str)
                .unwrap_or("")
        );
        let failed = attempt
            .get("failed_checks")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for check in failed {
            println!("    failed: {}", check.as_str().unwrap_or(""));
        }
    }
    for note in said
        .get("timeline")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
    {
        println!(
            "  {} {} {}",
            note.get("at").and_then(Value::as_str).unwrap_or(""),
            note.get("actor").and_then(Value::as_str).unwrap_or(""),
            note.get("action").and_then(Value::as_str).unwrap_or("")
        );
    }
    Ok(())
}

pub async fn simulate(
    which: &Which,
    number: Option<i32>,
    written: Option<String>,
) -> Result<(), Trouble> {
    let server = Server::we_know()?;
    let repository = repository_here(which.repo.as_deref())?;
    let number = which_pull_request(&server, &repository, number).await?;
    let said = server
        .post(
            &format!("/api/simulate/{repository}/{number}"),
            json!({ "config": written }),
        )
        .await?;
    if which.json {
        println!("{said:#}");
        return Ok(());
    }
    println!(
        "#{number} on {} would be {} — {}",
        said.get("branch").and_then(Value::as_str).unwrap_or(""),
        said.get("status").and_then(Value::as_str).unwrap_or(""),
        said.get("detail").and_then(Value::as_str).unwrap_or("")
    );
    println!(
        "  next: {}",
        said.get("next").and_then(Value::as_str).unwrap_or("")
    );
    Ok(())
}

async fn which_branch(
    server: &Server,
    repository: &str,
    given: Option<String>,
) -> Result<String, Trouble> {
    if let Some(given) = given {
        return Ok(given);
    }
    let (owner, name) = repository.split_once('/').ok_or(Trouble::WhichBranch)?;
    let listed = server.get("/api/repositories").await?;
    let branches: Vec<String> = listed
        .as_array()
        .into_iter()
        .flatten()
        .find(|found| {
            found.get("owner").and_then(Value::as_str) == Some(owner)
                && found.get("name").and_then(Value::as_str) == Some(name)
        })
        .and_then(|found| found.get("queues"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|queue| queue.get("branch").and_then(Value::as_str))
        .map(str::to_owned)
        .collect();

    match branches.as_slice() {
        [only] => Ok(only.clone()),
        [] => Err(Trouble::WhichBranch),
        many => Err(Trouble::WhichOfThese {
            branches: many.join(", "),
        }),
    }
}

async fn which_pull_request(
    server: &Server,
    repository: &str,
    given: Option<i32>,
) -> Result<i32, Trouble> {
    if let Some(given) = given {
        return Ok(given);
    }
    let branch = branch_here().ok_or(Trouble::WhichPullRequest)?;
    let found = server
        .get(&format!("/api/find/{repository}?branch={branch}"))
        .await?;
    found
        .get("pull_requests")
        .and_then(Value::as_array)
        .and_then(|listed| listed.first())
        .and_then(|first| first.get("number"))
        .and_then(Value::as_i64)
        .and_then(|number| i32::try_from(number).ok())
        .ok_or(Trouble::WhichPullRequest)
}
