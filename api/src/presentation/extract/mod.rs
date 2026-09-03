//! Request extractors: the typed things handlers ask for.

pub mod actor;
pub mod client_address;
pub mod json;

pub use actor::{Actor, Admin, AnyRole, Chef, RoleRequirement, Waiter};
pub use client_address::ClientAddress;
pub use json::JsonBody;
