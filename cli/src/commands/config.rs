use crate::cli::ConfigCmd;
use crate::config::{Config, Profile};
use crate::error::CliError;

pub fn run(cmd: &ConfigCmd) -> Result<i32, CliError> {
    let path = Config::default_path().map_err(|e| CliError::BadArgs(e.to_string()))?;
    let mut cfg = Config::load_from(&path).unwrap_or_default();
    match cmd {
        ConfigCmd::List => {
            // avoid surprise: print the whole config as TOML
            println!("{}", toml::to_string_pretty(&cfg).unwrap());
        }
        ConfigCmd::Get { key } => {
            let v = read_key(&cfg, key)
                .ok_or_else(|| CliError::NotFound(format!("config key {key}")))?;
            println!("{v}");
        }
        ConfigCmd::Set { key, value } => {
            write_key(&mut cfg, key, value)?;
            cfg.save_to(&path).map_err(|e| CliError::ServerError(e.to_string()))?;
        }
    }
    Ok(0)
}

fn read_key(cfg: &Config, key: &str) -> Option<String> {
    if key == "default_profile" { return cfg.default_profile.clone(); }
    let rest = key.strip_prefix("profiles.")?;
    let (name, field) = rest.split_once('.')?;
    let p = cfg.profiles.get(name)?;
    match field {
        "url" => Some(p.url.clone()),
        "token" => Some(p.token.clone()),
        _ => None,
    }
}

fn write_key(cfg: &mut Config, key: &str, value: &str) -> Result<(), CliError> {
    if key == "default_profile" {
        cfg.default_profile = Some(value.to_string());
        return Ok(());
    }
    let rest = key.strip_prefix("profiles.")
        .ok_or_else(|| CliError::BadArgs(format!("unknown key: {key}")))?;
    let (name, field) = rest.split_once('.')
        .ok_or_else(|| CliError::BadArgs(format!("config key must be 'profiles.<name>.<field>' or 'default_profile', got: {key}")))?;
    let p = cfg.profiles.entry(name.to_string())
        .or_insert_with(|| Profile { url: String::new(), token: String::new() });
    match field {
        "url" => p.url = value.to_string(),
        "token" => p.token = value.to_string(),
        _ => return Err(CliError::BadArgs(format!("unknown field: {field}"))),
    }
    Ok(())
}
