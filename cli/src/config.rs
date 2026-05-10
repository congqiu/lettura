use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Config {
    pub default_profile: Option<String>,
    #[serde(default)]
    pub profiles: BTreeMap<String, Profile>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Profile {
    pub url: String,
    pub token: String,
}

#[derive(Debug, Default)]
pub struct Override {
    pub profile: Option<String>,
    pub url: Option<String>,
    pub token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Resolved {
    pub url: String,
    pub token: String,
}

impl Config {
    pub fn default_path() -> anyhow::Result<std::path::PathBuf> {
        let dirs = directories::ProjectDirs::from("dev", "lettura", "lettura")
            .ok_or_else(|| anyhow::anyhow!("no config dir"))?;
        Ok(dirs.config_dir().join("config.toml"))
    }

    pub fn load_from(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let s = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&s)?)
    }

    pub fn save_to(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p)?;
        }
        let s = toml::to_string_pretty(self)?;
        std::fs::write(path, s)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }
}

pub fn resolve(cfg: &Config, over: &Override) -> anyhow::Result<Resolved> {
    let profile_name = over.profile.clone().or_else(|| cfg.default_profile.clone());
    let profile = profile_name.as_ref().and_then(|n| cfg.profiles.get(n));

    let url = over
        .url
        .clone()
        .or_else(|| profile.map(|p| p.url.clone()))
        .ok_or_else(|| anyhow::anyhow!("no server URL configured. Run `lettura-cli login`."))?;
    let token = over
        .token
        .clone()
        .or_else(|| profile.map(|p| p.token.clone()))
        .ok_or_else(|| anyhow::anyhow!("no token configured. Run `lettura-cli login`."))?;
    Ok(Resolved { url, token })
}
