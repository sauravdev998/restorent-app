//! The restaurant operations platform API.
//!
//! Four layers, with dependencies pointing inward only:
//!
//! - [`domain`] holds entities and the rules they enforce. It imports no
//!   framework and no I/O crate.
//! - [`application`] holds use cases. It reaches the outside world only through
//!   traits it declares itself, in [`application::ports`].
//! - [`infrastructure`] talks to Postgres, the environment, and stdout, and
//!   implements those traits.
//! - [`presentation`] is HTTP: handlers, extractors, wire shapes, status codes.
//!
//! The rule worth knowing before touching anything: a handler cannot reach the
//! connection pool. [`infrastructure::db`] is the only module that holds one,
//! the field is private, and no method hands it out. The single way to query
//! tenant data is [`infrastructure::db::Database::begin_scoped`], which returns
//! a transaction that already has `app.restaurant_id` set on it. Row level
//! security reads that setting, so a forgotten `WHERE` clause is caught by the
//! database rather than leaking another restaurant's orders.

#![deny(missing_docs)]

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
