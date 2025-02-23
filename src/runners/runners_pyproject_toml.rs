use crate::task_shadowing::check_path_executable;
use crate::types::{ShadowType, Task, TaskRunner};
use std::path::Path;

#[cfg(test)]
use crate::task_shadowing::{enable_mock, mock_executable, reset_mock};
#[cfg(test)]
use serial_test::serial;

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

    #[cfg(test)]
    eprintln!(
        "detect_package_manager debug: poetry={}, uv={}, poe={}",
        has_poetry, has_uv, has_poe
    );

    // If no package managers are available, return None
    if !has_poetry && !has_uv && !has_poe {
        #[cfg(test)]
        eprintln!("detect_package_manager debug: no package managers available");
        return None;
    }

    // Check for lock files first
    let poetry_lock_exists = dir.join("poetry.lock").exists();
    let uv_lock_exists = dir.join("uv.lock").exists();

    #[cfg(test)]
    eprintln!(
        "detect_package_manager debug: poetry_lock={}, uv_lock={}",
        poetry_lock_exists, uv_lock_exists
    );

    if poetry_lock_exists && has_poetry {
        #[cfg(test)]
        eprintln!("detect_package_manager debug: selecting poetry due to lock file");
        return Some(TaskRunner::PythonPoetry);
    }
    if uv_lock_exists && has_uv {
        #[cfg(test)]
        eprintln!("detect_package_manager debug: selecting uv due to lock file");
        return Some(TaskRunner::PythonUv);
    }

    // If no lock files, use preferred order
    #[cfg(test)]
    eprintln!("detect_package_manager debug: no lock files found, using preferred order");

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
    use std::fs::File;
    use tempfile::TempDir;

    fn create_poetry_lock(dir: &Path) {
        File::create(dir.join("poetry.lock")).unwrap();
    }

    fn create_uv_lock(dir: &Path) {
        File::create(dir.join("uv.lock")).unwrap();
    }

    #[test]
    #[serial]
    fn test_detect_package_manager_with_poetry_lock() {
        let temp_dir = TempDir::new().unwrap();
        create_poetry_lock(temp_dir.path());
        assert!(
            temp_dir.path().join("poetry.lock").exists(),
            "poetry.lock file should exist"
        );

        // Set up test environment with poetry only
        let env = TestEnvironment::new().with_executable("poetry");
        set_test_environment(env);

        // Debug checks
        let has_poetry = check_path_executable("poetry").is_some();
        assert!(
            has_poetry,
            "Poetry should be available via check_path_executable"
        );

        let result = detect_package_manager(temp_dir.path());
        assert_eq!(
            result,
            Some(TaskRunner::PythonPoetry),
            "Should detect Poetry as package manager"
        );

        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_detect_package_manager_with_venv() {
        let temp_dir = TempDir::new().unwrap();

        // Create uv.lock file
        create_uv_lock(temp_dir.path());
        assert!(
            temp_dir.path().join("uv.lock").exists(),
            "uv.lock file should exist"
        );

        // Reset and enable mock system first
        reset_mock();
        enable_mock();

        // Set up test environment with UV only
        let env = TestEnvironment::new().with_executable("uv");
        set_test_environment(env);

        // Mock UV being available
        mock_executable("uv");

        // Debug checks
        let has_poetry = check_path_executable("poetry").is_some();
        let has_uv = check_path_executable("uv").is_some();
        let has_poe = check_path_executable("poe").is_some();

        assert!(has_uv, "UV should be available via check_path_executable");
        assert!(!has_poetry, "Poetry should not be available");
        assert!(!has_poe, "Poe should not be available");

        // Verify lock file exists right before detection
        assert!(
            temp_dir.path().join("uv.lock").exists(),
            "uv.lock should exist before detection"
        );

        // Test package manager detection
        let result = detect_package_manager(temp_dir.path());
        assert_eq!(
            result,
            Some(TaskRunner::PythonUv),
            "Should detect UV as package manager"
        );

        // Clean up
        reset_mock();
        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_detect_package_manager_no_markers() {
        let temp_dir = TempDir::new().unwrap();

        // Set up test environment with poetry only
        let env = TestEnvironment::new().with_executable("poetry");
        set_test_environment(env.clone());

        // Debug assertions to help diagnose issues
        let poetry_path = check_path_executable("poetry");
        assert!(
            poetry_path.is_some(),
            "poetry executable should be available"
        );
        assert_eq!(
            poetry_path,
            Some(ShadowType::PathExecutable("/mock/bin/poetry".to_string()))
        );

        let result = detect_package_manager(temp_dir.path());
        assert_eq!(result, Some(TaskRunner::PythonPoetry));

        reset_to_real_environment();
    }
}
