use chrono::{DateTime,
             Utc};
use serde::{Deserialize,
            Serialize};
use std::{fmt,
          path::PathBuf};

/// Monotonically increasing generation number, in the NixOS sense.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GenerationId(pub u64);

impl fmt::Display for GenerationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}", self.0) }
}

impl GenerationId {
    /// Zero-padded so store directories sort lexicographically.
    pub fn dir_prefix(&self) -> String { format!("{:04}", self.0) }

    pub fn next(&self) -> GenerationId { GenerationId(self.0 + 1) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GenerationStatus {
    /// Built and tested, never activated.
    Built,
    /// Activated at least once and passed its health check.
    Healthy,
    /// Activated and rolled back because verification failed.
    Rejected,
}

/// The immutable record written next to a generation's artifacts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Generation {
    pub id: GenerationId,
    /// Commit the artifacts were built from, if the source was a git checkout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// Worktree the build came from, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<String>,
    /// True when the build directory had uncommitted changes.
    #[serde(default)]
    pub dirty: bool,
    pub built_at: DateTime<Utc>,
    pub status: GenerationStatus,
    /// Artifact paths captured into the store, relative to the generation root.
    #[serde(default)]
    pub artifacts: Vec<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl Generation {
    pub fn describe_source(&self) -> String {
        let commit = self.commit.as_deref().unwrap_or("unknown");
        let short: String = commit.chars().take(8).collect();
        match (&self.worktree, self.dirty) {
            | (Some(w), true) => format!("{short}+dirty ({w})"),
            | (Some(w), false) => format!("{short} ({w})"),
            | (None, true) => format!("{short}+dirty"),
            | (None, false) => short,
        }
    }
}

/// One line of the switch audit log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchEvent {
    pub at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<GenerationId>,
    pub to: GenerationId,
    pub reason: String,
}
