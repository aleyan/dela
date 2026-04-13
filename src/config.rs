use crate::environment::get_current_home;
use std::path::{Path, PathBuf};

pub fn preferred_config_dir_path_for(home: impl AsRef<Path>) -> PathBuf {
    home.as_ref().join(".config").join("dela")
}

pub fn preferred_allowlist_path_for(home: impl AsRef<Path>) -> PathBuf {
    preferred_config_dir_path_for(home).join("allowlist.toml")
}

pub fn legacy_allowlist_path_for(home: impl AsRef<Path>) -> PathBuf {
    home.as_ref().join(".dela").join("allowlist.toml")
}

pub fn preferred_config_dir_path() -> Result<PathBuf, String> {
    let home = get_current_home().ok_or("HOME environment variable not set".to_string())?;
    Ok(preferred_config_dir_path_for(PathBuf::from(home)))
}

pub fn legacy_dela_config_dir() -> Result<PathBuf, String> {
    let home = get_current_home().ok_or("HOME environment variable not set".to_string())?;
    Ok(PathBuf::from(home).join(".dela"))
}

pub fn legacy_allowlist_path() -> Result<PathBuf, String> {
    let home = get_current_home().ok_or("HOME environment variable not set".to_string())?;
    Ok(legacy_allowlist_path_for(PathBuf::from(home)))
}

pub fn active_dela_config_dir() -> Result<PathBuf, String> {
    let dela_dir = preferred_config_dir_path()?;
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
    Ok(preferred_config_dir_path()?.join("allowlist.toml"))
}

pub fn active_allowlist_path() -> Result<PathBuf, String> {
    let preferred_path = preferred_allowlist_path()?;
    if preferred_path.exists() {
        return Ok(preferred_path);
    }

    let legacy_path = legacy_allowlist_path()?;
    if legacy_path.exists() {
        return Ok(legacy_path);
    }

    Ok(preferred_path)
}
