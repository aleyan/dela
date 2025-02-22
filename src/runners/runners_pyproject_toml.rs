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
    // Check for available package managers
    let has_poetry = check_path_executable("poetry").is_some();
    let has_uv = check_path_executable("uv").is_some();
    let has_poe = check_path_executable("poe").is_some();

    // If no package managers are available, return None
    if !has_poetry && !has_uv && !has_poe {
        return None;
    }

    // Check for lock files first
    if dir.join("poetry.lock").exists() && has_poetry {
        return Some(TaskRunner::PythonPoetry);
    }
    if dir.join("uv.lock").exists() && has_uv {
        return Some(TaskRunner::PythonUv);
    }

    // If no lock files, use preferred order
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
    use crate::environment::{reset_to_real_environment, set_test_environment, TestEnvironment};
    use crate::task_shadowing::{enable_mock, mock_executable, reset_mock};
    use std::fs::{self, File};
    use tempfile::TempDir;

    fn create_poetry_lock(dir: &Path) {
        File::create(dir.join("poetry.lock")).unwrap();
    }

    fn create_uv_lock(dir: &Path) {
        File::create(dir.join("uv.lock")).unwrap();
    }

    #[test]
    fn test_detect_package_manager_with_poetry_lock() {
        let temp_dir = TempDir::new().unwrap();
        create_poetry_lock(temp_dir.path());

        // Enable mocking and mock poetry
        reset_mock();
        enable_mock();
        mock_executable("poetry");

        // Set up test environment
        let env = TestEnvironment::new().with_executable("poetry");
        set_test_environment(env);

        assert_eq!(
            detect_package_manager(temp_dir.path()),
            Some(TaskRunner::PythonPoetry)
        );

        reset_mock();
        reset_to_real_environment();
    }

    #[test]
    fn test_detect_package_manager_with_venv() {
        let temp_dir = TempDir::new().unwrap();
        create_uv_lock(temp_dir.path());

        // Enable mocking and mock UV
        reset_mock();
        enable_mock();
        mock_executable("uv");

        // Set up test environment
        let env = TestEnvironment::new().with_executable("uv");
        set_test_environment(env);

        assert_eq!(
            detect_package_manager(temp_dir.path()),
            Some(TaskRunner::PythonUv)
        );

        reset_mock();
        reset_to_real_environment();
    }

    #[test]
    fn test_detect_package_manager_no_markers() {
        let temp_dir = TempDir::new().unwrap();

        // Enable mocking and set up test environment
        reset_mock();
        enable_mock();

        // Set up test environment with poetry
        let env = TestEnvironment::new().with_executable("poetry");
        set_test_environment(env);

        assert_eq!(
            detect_package_manager(temp_dir.path()),
            Some(TaskRunner::PythonPoetry)
        );

        reset_mock();
        reset_to_real_environment();
    }
}
