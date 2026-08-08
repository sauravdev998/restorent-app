//! A transaction that is already scoped to one restaurant.
//!
//! This type is the whole tenant isolation story on the Rust side. A handler
//! cannot obtain a connection any other way, so it cannot run a query that is
//! not scoped. That is the point: a written rule saying "remember the WHERE
//! clause" gets forgotten somewhere around feature 19, and a type does not.

use sqlx::{PgConnection, Postgres, Transaction};

use crate::domain::error::{DomainError, DomainResult};
use crate::domain::ids::RestaurantId;

/// An open transaction with `app.restaurant_id` already set on it.
///
/// Row level security policies read that setting, so every query run through
/// here is filtered by the database itself even if the SQL forgets to filter.
/// The setting is applied with `set_config(..., true)`, which is `SET LOCAL`:
/// it lasts only for this transaction, so a pooled connection can never carry
/// one request's restaurant into the next request.
pub struct ScopedTx<'c> {
    tx: Transaction<'c, Postgres>,
    restaurant_id: RestaurantId,
}

impl<'c> ScopedTx<'c> {
    /// Wraps a transaction that has already had its scope applied.
    ///
    /// Deliberately visible only inside [`super`], so the only way to build one
    /// is [`super::Database::begin_scoped`], which does apply the scope.
    pub(super) const fn new(tx: Transaction<'c, Postgres>, restaurant_id: RestaurantId) -> Self {
        Self { tx, restaurant_id }
    }

    /// Which restaurant this transaction is scoped to.
    #[must_use]
    pub const fn restaurant_id(&self) -> RestaurantId {
        self.restaurant_id
    }

    /// The connection to run queries on, for example
    /// `sqlx::query!(..).fetch_one(tx.connection())`.
    pub fn connection(&mut self) -> &mut PgConnection {
        &mut self.tx
    }

    /// Commits the work.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::Unavailable`] if the database refuses the commit.
    pub async fn commit(self) -> DomainResult<()> {
        self.tx.commit().await.map_err(DomainError::from)
    }

    /// Rolls the work back. Dropping without committing does the same thing;
    /// this is for when you want it to be obvious in the code.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::Unavailable`] if the rollback itself fails.
    pub async fn rollback(self) -> DomainResult<()> {
        self.tx.rollback().await.map_err(DomainError::from)
    }
}
