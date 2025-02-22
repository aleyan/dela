use crate::task_shadowing::check_path_executable;
use crate::types::{Task, TaskRunner};
use std::path::Path;

#[allow(dead_code)]
pub fn parse(path: &Path) -> Result<Vec<Task>, String> {
    let _content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read pyproject.toml: {}", e))?;

    let tasks = Vec::new();
    Ok(tasks)
}

/// Detect which Python package manager to use based on lock files and available commands
#[allow(dead_code)]
pub fn detect_package_manager(dir: &Path) -> Option<TaskRunner> {
    // First check for available package managers
    let has_poetry = check_path_executable("poetry").is_some();
    let has_uv = check_path_executable("uv").is_some();
    let has_poe = check_path_executable("poe").is_some();

    // If only one package manager is available, use it
    let available_count = [has_poetry, has_uv, has_poe].iter().filter(|&&x| x).count();
    if available_count == 1 {
        if has_poetry {
            return Some(TaskRunner::PythonPoetry);
        }
        if has_uv {
            return Some(TaskRunner::PythonUv);
        }
        if has_poe {
            return Some(TaskRunner::PythonPoe);
        }
    }

    // If multiple package managers are available, use lock files to disambiguate
    if dir.join("poetry.lock").exists() && has_poetry {
        return Some(TaskRunner::PythonPoetry);
    }
    if dir.join(".venv").exists() && has_uv {
        return Some(TaskRunner::PythonUv);
    }

    // If no lock file but multiple package managers, use preferred order
    if has_poetry {
        Some(TaskRunner::PythonPoetry)
    } else if has_uv {
        Some(TaskRunner::PythonUv)
    } else if has_poe {
        Some(TaskRunner::PythonPoe)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task_shadowing::{enable_mock, mock_executable, reset_mock};
    use std::fs::{self, File};
    use tempfile::TempDir;

    fn create_poetry_lock(dir: &Path) {
        File::create(dir.join("poetry.lock")).unwrap();
    }

    fn create_venv(dir: &Path) {
        fs::create_dir_all(dir.join(".venv")).unwrap();
        File::create(dir.join(".venv/pyvenv.cfg")).unwrap();
    }

    #[test]
    fn test_detect_package_manager_with_poetry_lock() {
        let temp_dir = TempDir::new().unwrap();
        create_poetry_lock(temp_dir.path());

        // Enable mocking and mock poetry
        reset_mock();
        enable_mock();
        mock_executable("poetry");

        assert_eq!(
            detect_package_manager(temp_dir.path()),
            Some(TaskRunner::PythonPoetry)
        );

        reset_mock();
    }

    #[test]
    fn test_detect_package_manager_with_venv() {
        let temp_dir = TempDir::new().unwrap();
        create_venv(temp_dir.path());

        // Enable mocking and mock UV
        reset_mock();
        enable_mock();
        mock_executable("uv");

        assert_eq!(
            detect_package_manager(temp_dir.path()),
            Some(TaskRunner::PythonUv)
        );

        reset_mock();
    }

    #[test]
    fn test_detect_package_manager_no_markers() {
        let temp_dir = TempDir::new().unwrap();

        // Enable mocking and mock poetry
        reset_mock();
        enable_mock();
        mock_executable("poetry");

        assert_eq!(
            detect_package_manager(temp_dir.path()),
            Some(TaskRunner::PythonPoetry)
        );

        reset_mock();
    }
}
