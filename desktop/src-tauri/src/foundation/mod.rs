//! Cross-cutting infrastructure shared by every module: errors, logging, storage, secrets, state.
//! (Named `foundation` rather than `core` to avoid shadowing Rust's built-in `core` crate.)

pub mod apps;
pub mod errors;
pub mod event_bus;
pub mod logging;
pub mod preferences;
pub mod privacy;
pub mod quiet;
pub mod secrets;
pub mod state;
pub mod storage;
pub(crate) mod validate;
