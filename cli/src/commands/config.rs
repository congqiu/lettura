use crate::cli::ConfigCmd;
use crate::config::Config;
use crate::error::CliError;

pub fn run(cmd: &ConfigCmd) -> Result<i32, CliError> {
    let path = Config::default_path().map_err(|e| CliError::BadArgs(e.to_string()))?;
    let mut cfg = Config::load_from(&path).unwrap_or_default();
    match cmd {
        ConfigCmd::List => {
            println!("{}", toml::to_string_pretty(&cfg).unwrap());
        }
        ConfigCmd::Get { key } => {
            let v = get_by_key(&cfg, key)
                .ok_or_else(|| CliError::NotFound(format!("config key {key}")))?;
            println!("{v}");
        }
        ConfigCmd::Set { key, value } => {
            set_by_key(&mut cfg, key, value)?;
            cfg.save_to(&path).map_err(|e| CliError::ServerError(e.to_string()))?;
        }
    }
    Ok(0)
}

/// Navigate a Config by a dot-separated path and return the corresponding string value.
fn get_by_key(cfg: &Config, key: &str) -> Option<String> {
    let mut value = toml::Value::try_from(cfg).ok()?;
    for segment in key.split('.') {
        match value {
            toml::Value::Table(mut t) => {
                value = t.remove(segment)?;
            }
            _ => return None,
        }
    }
    match value {
        toml::Value::String(s) => Some(s),
        toml::Value::Integer(i) => Some(i.to_string()),
        toml::Value::Boolean(b) => Some(b.to_string()),
        toml::Value::Float(f) => Some(f.to_string()),
        _ => Some(value.to_string()),
    }
}

/// Set a value in Config by dot-separated path. Only supports existing paths
/// or profiles.<name>.url / profiles.<name>.token / default_profile for now.
fn set_by_key(cfg: &mut Config, key: &str, value: &str) -> Result<(), CliError> {
    if key == "default_profile" {
        cfg.default_profile = Some(value.to_string());
        return Ok(());
    }
    let mut doc = toml::Value::try_from(&*cfg)
        .map_err(|e| CliError::ServerError(format!("config serialize: {e}")))?;
    let segments: Vec<&str> = key.split('.').collect();
    if segments.len() < 2 {
        return Err(CliError::BadArgs(format!("unsupported config key: {key}")));
    }
    let mut current = &mut doc;
    for (i, seg) in segments.iter().enumerate() {
        let is_last = i == segments.len() - 1;
        match current {
            toml::Value::Table(t) => {
                if is_last {
                    t.insert(seg.to_string(), toml::Value::String(value.to_string()));
                    break;
                }
                current = t.entry(seg.to_string()).or_insert_with(|| toml::Value::Table(toml::Table::new()));
            }
            _ => return Err(CliError::BadArgs(format!("invalid path at segment '{seg}'"))),
        }
    }
    *cfg = doc.try_into()
        .map_err(|e| CliError::BadArgs(format!("invalid value for {key}: {e}")))?;
    Ok(())
}
