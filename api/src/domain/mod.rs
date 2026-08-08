//! The innermost layer: entities, value objects, and the rules they enforce.
//!
//! Nothing here may import a framework or an I/O crate. No `axum`, no `sqlx`.
//! Utility value types (`uuid`, `chrono`, and later `rust_decimal`) are allowed,
//! because they describe values rather than machinery.

pub mod error;
pub mod event;
pub mod ids;
