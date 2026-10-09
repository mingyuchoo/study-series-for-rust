//! The immutable generation store and its atomic activation pointer.
//!
//! The filesystem is the source of truth: there is no database. A generation is
//! a directory under `store/`, a numbered symlink under `generations/`, and the
//! `current` symlink that selects one of them. Switching is a `rename(2)` over
//! `current`, which is atomic, so a crash mid-switch can never leave the
//! project pointing at nothing.
//! Also owns project discovery, serialization and locked run-state cleanup;
//! process control is supplied by a separate application adapter.

mod error;
pub mod layout;
pub mod lock;
mod project;
mod run_state;
mod store;

pub use error::{Error,
                Result};
pub use layout::{Layout,
                 MANIFEST};
pub use lock::ProjectLock;
pub use project::{Discovery,
                  detect_preset,
                  discover_project,
                  load_config,
                  parse_config,
                  render_config,
                  save_config};
pub use run_state::FileRunState;
pub use store::{GenerationEntry,
                StagedGeneration,
                Store,
                StoreRepository,
                write_atomic};
