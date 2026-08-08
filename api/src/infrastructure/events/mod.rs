//! Live delivery: one Postgres listen connection per instance, fanning out to
//! the streams that instance is holding open.

pub mod listener;
pub mod registry;

pub use listener::{ListenerHandle, spawn};
pub use registry::EventRegistry;
