//! `gm` — drive a change from an isolated worktree, through build and test,
//! into a numbered generation you can switch to and roll back from.

mod cmd;
mod ui;

use clap::{Parser,
           Subcommand};
use std::{path::PathBuf,
          process::ExitCode};

#[derive(Parser)]
#[command(
    name = "gm",
    version,
    about = "Worktree-to-generation development lifecycle manager",
    long_about = "Develop in an isolated git worktree, freeze a build into an immutable \
                  generation, activate it with a health check, and roll back when it \
                  misbehaves."
)]
struct Cli {
    /// Run as if started in this directory.
    #[arg(short = 'C', long, global = true, value_name = "DIR")]
    directory: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Manage project configuration and inspect the project overview.
    #[command(subcommand)]
    Project(ProjectCommand),

    /// Manage development worktrees.
    #[command(subcommand)]
    Worktree(WorktreeCommand),

    /// Build, activate, inspect, and prune generations.
    #[command(subcommand)]
    Generation(GenerationCommand),

    /// Inspect and control the service process.
    #[command(subcommand)]
    Service(ServiceCommand),
}

#[derive(Subcommand)]
enum ProjectCommand {
    /// Create generation-manager.toml in the current directory.
    Init {
        /// Language preset: rust, node, python, go, generic.
        #[arg(long)]
        preset: Option<String>,
        /// Project name (defaults to the directory name).
        #[arg(long)]
        name: Option<String>,
        /// Overwrite an existing manifest.
        #[arg(long)]
        force: bool,
    },

    /// Show the project, active generation, and service state.
    Status,
}

#[derive(Subcommand)]
enum WorktreeCommand {
    /// Create a worktree and a branch for it.
    Create {
        name: String,
        /// Base commit or branch for the new branch.
        #[arg(long)]
        base: Option<String>,
    },
    /// List worktrees managed by this project.
    List,
    /// Run a worktree's code directly, without creating a generation.
    Run {
        /// Worktree to run (defaults to the one you are standing in).
        #[arg(value_name = "WORKTREE")]
        worktree: Option<String>,
        /// Skip the build stage before running.
        #[arg(long)]
        no_build: bool,
        /// Run in the background instead of attaching to the terminal.
        #[arg(long)]
        detach: bool,
    },
    /// Remove a worktree.
    Remove {
        name: String,
        /// Remove even with uncommitted changes.
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum GenerationCommand {
    /// Build and test a checkout into a new generation.
    Build {
        /// Build from this worktree (defaults to the current worktree or
        /// project root).
        #[arg(value_name = "WORKTREE")]
        worktree: Option<String>,
        /// Free-form note stored with the generation.
        #[arg(long)]
        note: Option<String>,
        /// Activate the generation immediately after a successful build.
        #[arg(long)]
        activate: bool,
    },

    /// List generations.
    List,

    /// Activate a generation and verify it.
    Activate {
        /// Generation number (defaults to the newest).
        #[arg(value_name = "GENERATION")]
        generation: Option<u64>,
    },

    /// Return to an older generation.
    Rollback {
        /// Target generation (defaults to the one before the active).
        #[arg(value_name = "GENERATION")]
        generation: Option<u64>,
    },

    /// Show the activation log.
    History,

    /// Delete old generations.
    Prune {
        /// Number of recent generations to keep.
        #[arg(long, default_value_t = 5)]
        keep: usize,
    },
}

#[derive(Subcommand)]
enum ServiceCommand {
    /// Show the service process and its source.
    Status,
    /// Start the service from the active generation.
    Start,
    /// Stop the running service.
    Stop,
    /// Stop then start the active generation.
    Restart,
    /// Show the tail of the service log.
    Logs {
        #[arg(short = 'n', long, default_value_t = 40)]
        lines: usize,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    if let Some(dir) = &cli.directory
        && let Err(err) = std::env::set_current_dir(dir)
    {
        eprintln!("{} cannot enter {}: {err}", ui::ERR, dir.display());
        return ExitCode::FAILURE;
    }

    match cmd::dispatch(cli.command) {
        | Ok(code) => code,
        | Err(err) => {
            eprintln!("{} {err:#}", ui::ERR);
            ExitCode::FAILURE
        },
    }
}
