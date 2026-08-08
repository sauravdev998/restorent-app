//! The outermost layer: HTTP. Handlers, extractors, wire shapes, status codes.
//!
//! Nothing here holds business rules, and nothing inner imports it.

pub mod error;
pub mod extract;
pub mod handlers;
pub mod openapi;
pub mod router;
pub mod state;
