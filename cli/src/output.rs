use std::io::Write;
use serde::Serialize;

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

pub fn emit_ids<I: IntoIterator<Item = uuid::Uuid>>(ids: I) -> anyhow::Result<()> {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    for id in ids {
        writeln!(lock, "{id}")?;
    }
    Ok(())
}
