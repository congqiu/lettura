//! Structured audit logging for security-sensitive operations.
//!
//! Provides a dedicated audit log target that outputs structured JSON
//! for easy ingestion by log aggregation systems.

use serde_json::Value;
use tracing::{info, instrument};
use uuid::Uuid;

/// Log an audit event for security-sensitive operations.
///
/// This function emits structured logs to the "audit" target, which can
/// be filtered and routed separately from general application logs.
///
/// # Arguments
///
/// * `user_id` - The user performing the action
/// * `action` - The action being performed (e.g., "entry.save", "auth.login")
/// * `resource_type` - The type of resource (e.g., "entry", "user", "pat")
/// * `resource_id` - The ID of the affected resource
/// * `details` - Optional additional context as JSON
///
/// # Example
///
/// ```ignore
/// use crate::audit::log_audit_event;
///
/// log_audit_event(
///     user.id,
///     "entry.save",
///     "entry",
///     &entry.id.to_string(),
///     Some(json!({ "url": "https://example.com" })),
/// );
/// ```
#[instrument(skip_all, fields(
    user_id = %user_id,
    action = %action,
    resource_type = %resource_type,
    resource_id = %resource_id,
))]
pub fn log_audit_event(
    user_id: Uuid,
    action: &str,
    resource_type: &str,
    resource_id: &str,
    details: Option<Value>,
) {
    info!(
        target: "audit",
        action = %action,
        resource_type = %resource_type,
        resource_id = %resource_id,
        details = ?details,
        "audit event"
    );
}

/// Convenience function for authentication events.
pub fn log_auth_event(user_id: Uuid, action: &str, details: Option<Value>) {
    log_audit_event(user_id, action, "auth", &user_id.to_string(), details);
}

/// Convenience function for entry events.
pub fn log_entry_event(user_id: Uuid, action: &str, entry_id: Uuid, details: Option<Value>) {
    log_audit_event(user_id, action, "entry", &entry_id.to_string(), details);
}

/// Convenience function for PAT events.
pub fn log_pat_event(user_id: Uuid, action: &str, pat_id: Uuid, details: Option<Value>) {
    log_audit_event(user_id, action, "pat", &pat_id.to_string(), details);
}

/// Convenience function for tag events.
pub fn log_tag_event(user_id: Uuid, action: &str, tag_id: Uuid, details: Option<Value>) {
    log_audit_event(user_id, action, "tag", &tag_id.to_string(), details);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_audit_event() {
        // This test verifies the function compiles and runs without panic.
        // Actual log output verification would require a tracing subscriber.
        let user_id = Uuid::new_v4();
        let resource_id = Uuid::new_v4();

        log_audit_event(
            user_id,
            "test.action",
            "test_resource",
            &resource_id.to_string(),
            Some(serde_json::json!({ "key": "value" })),
        );
    }
}
