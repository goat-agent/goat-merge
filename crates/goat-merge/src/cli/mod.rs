pub mod client;
pub mod config_command;
pub mod queue_command;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "goat-merge",
    version,
    about = "A self-hosted merge queue for GitHub"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Run the server: web, webhooks and the queue engine, in one process
    Run,
    /// Remember which goat-merge server to talk to, and sign in to it
    Login {
        /// The address of the server, for example https://merge.example.com
        url: String,
        /// A session token, if you would rather paste one than be prompted
        #[arg(long)]
        token: Option<String>,
    },
    /// Show the queue for the current repository
    Queue {
        #[command(subcommand)]
        command: Option<QueueCommand>,
        #[command(flatten)]
        which: Which,
    },
    /// Ask for a pull request to be merged
    Enqueue {
        pull_request: Option<i32>,
        #[command(flatten)]
        which: Which,
    },
    /// Take a pull request back out of the queue
    Dequeue {
        pull_request: Option<i32>,
        #[command(flatten)]
        which: Which,
    },
    /// Try a pull request again after a failed verification
    Retry {
        pull_request: Option<i32>,
        #[command(flatten)]
        which: Which,
    },
    /// Say why a pull request is where it is
    Explain {
        pull_request: Option<i32>,
        #[command(flatten)]
        which: Which,
    },
    /// Move a pull request to the front, and say why
    Expedite {
        pull_request: i32,
        #[arg(long)]
        reason: String,
        #[command(flatten)]
        which: Which,
    },
    /// Work with .github/merge-queue.yml
    #[command(subcommand)]
    Config(ConfigCommand),
}

#[derive(Subcommand)]
pub enum QueueCommand {
    /// Stop verifying and merging until it is resumed
    Pause,
    /// Start again after a pause
    Resume,
}

#[derive(Subcommand)]
pub enum ConfigCommand {
    /// Read a merge-queue.yml and report what it says, or why it cannot be used
    Validate {
        /// The file to read. Defaults to .github/merge-queue.yml
        path: Option<PathBuf>,
    },
    /// Ask the server what a configuration would do to a pull request right now
    Simulate {
        pull_request: Option<i32>,
        /// The file to try. Defaults to .github/merge-queue.yml
        #[arg(long)]
        path: Option<PathBuf>,
        #[command(flatten)]
        which: Which,
    },
}

#[derive(clap::Args, Clone, Default)]
pub struct Which {
    /// owner/name, when you are not inside the repository
    #[arg(long, global = true)]
    pub repo: Option<String>,
    /// The branch being merged into
    #[arg(long, global = true)]
    pub branch: Option<String>,
    /// Print machine-readable JSON instead of a table
    #[arg(long, global = true)]
    pub json: bool,
}
