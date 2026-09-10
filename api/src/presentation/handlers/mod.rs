//! HTTP handlers. Thin: they extract, call a use case, and shape a response.

pub mod auth;
pub mod billing;
pub mod dev;
pub mod events;
pub mod health;
pub mod me;
pub mod menu;
pub mod service;
