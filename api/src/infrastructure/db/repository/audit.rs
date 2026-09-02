//! Writing the record of consequential changes.

use serde_json::Value;
use uuid::Uuid;

use crate::domain::audit::AuditAction;
use crate::domain::error::DomainResult;
use crate::domain::ids::{AuditEntryId, StaffId};

use super::super::ScopedTx;

/// Writes one entry, inside the transaction that made the change.
///
/// Being in the same transaction is the point: if the change rolls back so does
/// its record, and if the change commits the record cannot be missing. An audit
/// log written afterwards, from another connection, is a log that disagrees with
/// the data whenever anything goes wrong, which is exactly when it is read.
///
/// # Errors
///
/// Returns [`DomainError::Unavailable`](crate::domain::error::DomainError) if
/// the insert fails.
pub async fn record(
    tx: &mut ScopedTx<'_>,
    actor_staff_id: Option<StaffId>,
    action: AuditAction,
    entity_type: &str,
    entity_id: Uuid,
    before: Option<Value>,
    after: Option<Value>,
) -> DomainResult<()> {
    let restaurant_id = tx.restaurant_id().as_uuid();

    sqlx::query!(
        r#"
        INSERT INTO audit_log
            (id, restaurant_id, actor_staff_id, action, entity_type, entity_id,
             before, after, occurred_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, now(), now())
        "#,
        AuditEntryId::new().as_uuid(),
        restaurant_id,
        actor_staff_id.map(StaffId::as_uuid),
        action.as_label(),
        entity_type,
        entity_id,
        before,
        after,
    )
    .execute(tx.connection())
    .await?;

    Ok(())
}
