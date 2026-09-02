//! The innermost layer: entities, value objects, and the rules they enforce.
//!
//! Nothing here may import a framework or an I/O crate. No `axum`, no `sqlx`.
//! Utility value types (`uuid`, `chrono`, `rust_decimal`, `serde_json`) are
//! allowed, because they describe values rather than machinery.
//!
//! The enums in [`enums`] mirror Postgres enum types, and the mapping that
//! carries them across is in `infrastructure::db::pg_enum` rather than here,
//! precisely so that this layer keeps that rule.

pub mod audit;
pub mod billing;
pub mod catalog;
pub mod enums;
pub mod error;
pub mod event;
pub mod ids;
pub mod money;
pub mod people;
pub mod service;
