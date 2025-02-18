use crate::task_shadowing::check_path_executable;
use crate::types::TaskRunner;
use std::fs;
use std::path::Path;

/// Detect which Python package manager to use based on project configuration and available commands
pub fn detect_package_manager(dir: &Path) -> Option<TaskRunner> {
    // Check for poetry.lock first
    if dir.join("poetry.lock").exists() && check_path_executable("poetry").is_some() {
        return Some(TaskRunner::PythonPoetry);
    }

    // Check pyproject.toml for Poetry or uv scripts
    if let Ok(content) = fs::read_to_string(dir.join("pyproject.toml")) {
        if content.contains("[tool.poetry.scripts]") && check_path_executable("poetry").is_some() {
            return Some(TaskRunner::PythonPoetry);
        }
        if content.contains("[project.scripts]")
            && !content.contains("[tool.poetry.scripts]")
            && check_path_executable("uv").is_some()
        {
            return Some(TaskRunner::PythonUv);
        }
    }

    // Check for .venv/pyvenv.cfg for uv
    if dir.join(".venv/pyvenv.cfg").exists() && check_path_executable("uv").is_some() {
        return Some(TaskRunner::PythonUv);
    }

    // If no specific markers found, check for available package managers in preferred order
    if check_path_executable("poetry").is_some() {
        Some(TaskRunner::PythonPoetry)
    } else if check_path_executable("uv").is_some() {
        Some(TaskRunner::PythonUv)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

        if check_path_executable("poetry").is_some() {
            assert_eq!(
                detect_package_manager(temp_dir.path()),
                Some(TaskRunner::PythonPoetry)
            );
        }
    }

    #[test]
    fn test_detect_package_manager_with_venv() {
        let temp_dir = TempDir::new().unwrap();
        create_venv(temp_dir.path());

        if check_path_executable("uv").is_some() {
            assert_eq!(
                detect_package_manager(temp_dir.path()),
                Some(TaskRunner::PythonUv)
            );
        }
    }

    #[test]
    fn test_detect_package_manager_no_markers() {
        let temp_dir = TempDir::new().unwrap();

        let result = detect_package_manager(temp_dir.path());
        // Result depends on which package managers are installed
        if let Some(runner) = result {
            assert!(matches!(
                runner,
                TaskRunner::PythonUv | TaskRunner::PythonPoetry
            ));
        }
    }
}
