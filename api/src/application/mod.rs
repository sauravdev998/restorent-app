//! Use cases: thin orchestrators that call domain logic and the ports below.
//!
//! Same import rule as the domain layer. No `axum`, no `sqlx`. A use case
//! reaches the outside world only through a trait declared in [`ports`].

pub mod health;
pub mod ports;
