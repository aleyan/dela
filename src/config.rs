use crate::environment::get_current_home;
use std::path::PathBuf;

pub fn dela_config_dir() -> Result<PathBuf, String> {
    let home = get_current_home().ok_or("HOME environment variable not set".to_string())?;
    Ok(PathBuf::from(home).join(".config").join("dela"))
}

pub fn legacy_dela_config_dir() -> Result<PathBuf, String> {
    let home = get_current_home().ok_or("HOME environment variable not set".to_string())?;
    Ok(PathBuf::from(home).join(".dela"))
}

pub fn active_dela_config_dir() -> Result<PathBuf, String> {
    let dela_dir = dela_config_dir()?;
    if dela_dir.exists() {
        return Ok(dela_dir);
    }

    let legacy_dir = legacy_dela_config_dir()?;
    if legacy_dir.exists() {
        return Ok(legacy_dir);
    }

    Ok(dela_dir)
}

pub fn preferred_allowlist_path() -> Result<PathBuf, String> {
    Ok(dela_config_dir()?.join("allowlist.toml"))
}

pub fn active_allowlist_path() -> Result<PathBuf, String> {
    Ok(active_dela_config_dir()?.join("allowlist.toml"))
}
