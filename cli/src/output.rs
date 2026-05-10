use serde::Serialize;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};

static QUIET: AtomicBool = AtomicBool::new(false);

pub fn set_quiet(v: bool) {
    QUIET.store(v, Ordering::Relaxed);
}

pub fn is_quiet() -> bool {
    QUIET.load(Ordering::Relaxed)
}

/// Emit JSON to stdout. Core command output — not affected by --quiet.
pub fn emit_json<T: Serialize>(v: &T, pretty: bool) -> anyhow::Result<()> {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    if pretty {
        serde_json::to_writer_pretty(&mut lock, v)?;
    } else {
        serde_json::to_writer(&mut lock, v)?;
    }
    writeln!(lock)?;
    Ok(())
}

/// Emit IDs to stdout. Core command output — not affected by --quiet.
pub fn emit_ids<I: IntoIterator<Item: std::fmt::Display>>(ids: I) -> anyhow::Result<()> {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    for id in ids {
        writeln!(lock, "{id}")?;
    }
    Ok(())
}

/// Human-readable list of entries.
pub fn emit_human_entries(entries: &[crate::api_types::EntrySummary]) -> anyhow::Result<()> {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    for e in entries {
        let title = e.title.as_deref().unwrap_or("(no title)");
        writeln!(lock, "{} | {} | {}", e.id, title, e.url)?;
    }
    Ok(())
}

/// Human-readable list of tags.
pub fn emit_human_tags(tags: &[crate::api_types::Tag]) -> anyhow::Result<()> {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    for t in tags {
        writeln!(lock, "{} | {}", t.id, t.label)?;
    }
    Ok(())
}

pub fn emit_human_audit_logs(
    logs: &[crate::api_types::AuditLog],
    total: i64,
    limit: i64,
    offset: i64,
) -> anyhow::Result<()> {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    writeln!(
        lock,
        "Showing {}-{} of {} audit logs",
        offset + 1,
        (offset + limit).min(total),
        total
    )?;
    for log in logs {
        let ts = log.created_at.as_deref().unwrap_or("-");
        let action = &log.action;
        let resource = log.resource_type.as_deref().unwrap_or("-");
        let status = &log.status;
        writeln!(
            lock,
            "{} | {} | {} | {} | {}",
            log.id, ts, action, resource, status
        )?;
    }
    Ok(())
}

/// Print an informational message, suppressed by --quiet.
pub fn info(msg: &str) {
    if !is_quiet() {
        let _ = writeln!(std::io::stdout().lock(), "{msg}");
    }
}
