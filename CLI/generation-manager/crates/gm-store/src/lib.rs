//! The immutable generation store and its atomic activation pointer.
//!
//! The filesystem is the source of truth: there is no database. A generation is
//! a directory under `store/`, a numbered symlink under `generations/`, and the
//! `current` symlink that selects one of them. Switching is a `rename(2)` over
//! `current`, which is atomic, so a crash mid-switch can never leave the
//! project pointing at nothing.

pub mod layout;
pub mod lock;
mod store;

pub use layout::Layout;
pub use lock::ProjectLock;
pub use store::{GenerationEntry,
                StagedGeneration,
                Store};
