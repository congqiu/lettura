use std::io::Write;
use std::path::PathBuf;

use crate::error::CliError;
use crate::output;
use crate::skill_asset::SkillAssets;

const SKILL_NAME: &str = "lettura.md";

pub fn run_print() -> Result<i32, CliError> {
    let bytes = SkillAssets::get(SKILL_NAME)
        .ok_or_else(|| CliError::ServerError("embedded skill asset missing".into()))?;
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    lock.write_all(&bytes.data).map_err(CliError::from)?;
    Ok(0)
}

pub fn run_install(path: Option<&str>) -> Result<i32, CliError> {
    let target: PathBuf = match path {
        Some(p) => PathBuf::from(p),
        None => {
            let base = directories::BaseDirs::new()
                .ok_or_else(|| CliError::BadArgs("no home directory available".into()))?;
            base.home_dir()
                .join(".claude")
                .join("skills")
                .join(SKILL_NAME)
        }
    };
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(CliError::from)?;
    }
    let bytes = SkillAssets::get(SKILL_NAME)
        .ok_or_else(|| CliError::ServerError("embedded skill asset missing".into()))?;
    std::fs::write(&target, &bytes.data).map_err(CliError::from)?;
    output::info(&format!("Skill installed to {}", target.display()));
    Ok(0)
}
