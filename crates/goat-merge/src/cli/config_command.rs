use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use goat_merge_core::config::{self, Config, Enqueue};

pub fn validate(path: Option<PathBuf>) -> ExitCode {
    let path = path.unwrap_or_else(|| PathBuf::from(config::FILE));
    let source = match std::fs::read_to_string(&path) {
        Ok(source) => source,
        Err(problem) => {
            eprintln!("{} could not be read: {problem}", path.display());
            return ExitCode::FAILURE;
        }
    };
    match Config::parse(&source) {
        Ok(config) => {
            describe(&path, &config);
            ExitCode::SUCCESS
        }
        Err(problem) => {
            eprintln!("{} {problem}", path.display());
            ExitCode::FAILURE
        }
    }
}

fn describe(path: &Path, config: &Config) {
    if config.queues.is_empty() {
        println!(
            "{} is valid, but lists no queues, so no branch is managed",
            path.display()
        );
        return;
    }
    println!("{} is valid", path.display());
    for queue in &config.queues {
        let entry = match queue.enqueue {
            Enqueue::Manual => format!("on the {} label", config::LABEL),
            Enqueue::Automatic => "as soon as it is ready".to_owned(),
        };
        let method = queue.merge_method.map_or_else(
            || "whatever the repository allows".to_owned(),
            |method| method.to_string(),
        );
        println!(
            "  {} enters {entry}, merges by {method}, and waits up to {} for its checks",
            queue.branch,
            spelled_out(queue.check_timeout)
        );
    }
    if config.retry.ci_failures > 0 {
        println!(
            "  retry.ci_failures is read but not acted on yet, so a failed check still takes the \
             pull request out of the queue. Press Retry, or add the label again"
        );
    }
}

fn spelled_out(span: Duration) -> String {
    let seconds = span.as_secs();
    if seconds.is_multiple_of(3600) {
        return plural(seconds / 3600, "hour");
    }
    if seconds.is_multiple_of(60) {
        return plural(seconds / 60, "minute");
    }
    plural(seconds, "second")
}

fn plural(count: u64, unit: &str) -> String {
    if count == 1 {
        format!("1 {unit}")
    } else {
        format!("{count} {unit}s")
    }
}

pub fn read_or_default(path: Option<PathBuf>) -> Option<String> {
    let path = path.unwrap_or_else(|| PathBuf::from(config::FILE));
    std::fs::read_to_string(path).ok()
}
