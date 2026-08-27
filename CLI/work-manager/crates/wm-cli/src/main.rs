//! `wm` — drive a change from an isolated worktree, through build and test, into
//! a numbered generation you can switch to and roll back from.

mod cmd;
mod ui;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "wm",
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
    /// Create work-manager.toml in the current directory.
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

    /// Show the active generation and service state.
    Status,

    /// Manage development worktrees (stage 1).
    #[command(subcommand)]
    Dev(DevCommand),

    /// Build and test a checkout into a new generation (stage 2).
    Build {
        /// Build from this worktree instead of the project root.
        #[arg(long, value_name = "NAME")]
        from: Option<String>,
        /// Free-form note stored with the generation.
        #[arg(long)]
        note: Option<String>,
        /// Activate the generation immediately after a successful build.
        #[arg(long)]
        switch: bool,
    },

    /// Activate a generation and verify it (stage 2).
    Switch {
        /// Generation number (defaults to the newest).
        #[arg(long = "gen", value_name = "N")]
        generation: Option<u64>,
    },

    /// Return to an older generation (stage 3).
    Rollback {
        /// Target generation (defaults to the one before the active).
        #[arg(long, value_name = "N")]
        to: Option<u64>,
    },

    /// List generations.
    #[command(visible_alias = "gens")]
    Generations,

    /// Show the switch log.
    History,

    /// Delete old generations.
    Gc {
        /// Number of recent generations to keep.
        #[arg(long, default_value_t = 5)]
        keep: usize,
    },

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

#[derive(Subcommand)]
enum DevCommand {
    /// Create a worktree and a branch for it.
    New {
        name: String,
        /// Base commit or branch for the new branch.
        #[arg(long)]
        base: Option<String>,
    },
    /// List worktrees managed by this project.
    List,
    /// Run this worktree's code directly, without creating a generation.
    Run {
        /// Worktree to run (defaults to the one you are standing in).
        #[arg(long, value_name = "NAME")]
        from: Option<String>,
        /// Skip the build stage before running.
        #[arg(long)]
        no_build: bool,
        /// Run in the background instead of attaching to the terminal.
        #[arg(long)]
        detach: bool,
    },
    /// Remove a worktree.
    Rm {
        name: String,
        /// Remove even with uncommitted changes.
        #[arg(long)]
        force: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    if let Some(dir) = &cli.directory
        && let Err(err) = std::env::set_current_dir(dir) {
            eprintln!("{} cannot enter {}: {err}", ui::ERR, dir.display());
            return ExitCode::FAILURE;
        }

    match cmd::dispatch(cli.command) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("{} {err:#}", ui::ERR);
            ExitCode::FAILURE
        }
    }
}
