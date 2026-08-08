//! The outer layer that talks to the world: Postgres, the environment, stdout.
//!
//! Everything here implements something the inner layers declared. Nothing here
//! is imported by [`crate::domain`] or [`crate::application`].

pub mod config;
pub mod db;
pub mod events;
pub mod health;
pub mod telemetry;
