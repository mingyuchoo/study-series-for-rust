use crate::generation::GenerationId;
use chrono::{DateTime,
             Utc};
use serde::{Deserialize,
            Serialize};
use std::fmt;

/// Where the running process came from.
///
/// Recording this is what keeps `gm status` honest once a worktree can be run
/// directly: "something is running" is not useful if you cannot tell a verified
/// generation from an unverified development checkout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RunSource {
    /// The activated generation — built, tested and health-checked.
    Generation { id: GenerationId },
    /// A development worktree, run in place with no verification.
    Worktree { name: String },
}

impl fmt::Display for RunSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            | RunSource::Generation {
                id,
            } => write!(f, "generation {id}"),
            | RunSource::Worktree {
                name,
            } => write!(f, "worktree `{name}` (dev, unverified)"),
        }
    }
}

impl RunSource {
    pub fn is_dev(&self) -> bool { matches!(self, RunSource::Worktree { .. }) }
}

/// The single service slot. One project runs at most one process, so this file
/// replaces a bare pid file: the pid alone cannot answer "running what?".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunState {
    pub source: RunSource,
    pub pid: i32,
    pub started_at: DateTime<Utc>,
    /// False while a foreground `gm dev run` owns the terminal.
    #[serde(default)]
    pub detached: bool,
}
