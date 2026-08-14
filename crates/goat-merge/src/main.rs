use std::process::ExitCode;

use clap::Parser;
use goat_merge::cli::{Cli, Command, ConfigCommand, QueueCommand, config_command, queue_command};
use goat_merge::settings::Settings;
use tracing_subscriber::EnvFilter;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Run => run(),
        Command::Config(ConfigCommand::Validate { path }) => config_command::validate(path),
        other => talking_to_a_server(other),
    }
}

fn run() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| "goat_merge=info".into()),
        )
        .init();

    let settings = match Settings::from_the_environment() {
        Ok(settings) => settings,
        Err(problem) => {
            eprintln!("{problem}");
            return ExitCode::FAILURE;
        }
    };

    let engine = match tokio::runtime::Runtime::new() {
        Ok(engine) => engine,
        Err(problem) => {
            eprintln!("the async runtime could not be started: {problem}");
            return ExitCode::FAILURE;
        }
    };
    match engine.block_on(goat_merge::serve::run(settings)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(problem) => {
            eprintln!("{problem}");
            ExitCode::FAILURE
        }
    }
}

fn talking_to_a_server(command: Command) -> ExitCode {
    let engine = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(engine) => engine,
        Err(problem) => {
            eprintln!("the async runtime could not be started: {problem}");
            return ExitCode::FAILURE;
        }
    };
    let outcome = engine.block_on(async {
        match command {
            Command::Run | Command::Config(ConfigCommand::Validate { .. }) => Ok(()),
            Command::Login { url, token } => queue_command::login(&url, token).await,
            Command::Queue { command, which } => match command {
                None => queue_command::show_queue(&which).await,
                Some(QueueCommand::Pause) => queue_command::hold(&which, true).await,
                Some(QueueCommand::Resume) => queue_command::hold(&which, false).await,
            },
            Command::Enqueue {
                pull_request,
                which,
            } => queue_command::act_on(&which, pull_request, "enqueue").await,
            Command::Dequeue {
                pull_request,
                which,
            } => queue_command::act_on(&which, pull_request, "dequeue").await,
            Command::Retry {
                pull_request,
                which,
            } => queue_command::act_on(&which, pull_request, "retry").await,
            Command::Explain {
                pull_request,
                which,
            } => queue_command::explain(&which, pull_request).await,
            Command::Expedite {
                pull_request,
                reason,
                which,
            } => queue_command::expedite(&which, pull_request, &reason).await,
            Command::Config(ConfigCommand::Simulate {
                pull_request,
                path,
                which,
            }) => {
                let written = config_command::read_or_default(path);
                queue_command::simulate(&which, pull_request, written).await
            }
        }
    });
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(problem) => {
            eprintln!("{problem}");
            ExitCode::FAILURE
        }
    }
}
