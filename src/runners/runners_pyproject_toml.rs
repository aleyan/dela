use crate::task_shadowing::check_path_executable;
use crate::types::{Task, TaskDefinitionFile, TaskDefinitionType, TaskFileStatus, TaskRunner};
use std::fs;
use std::path::Path;

/// Parse a pyproject.toml file at the given path and extract tasks
pub fn parse(path: &Path) -> Result<Vec<Task>, String> {
    let _content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read pyproject.toml: {}", e))?;

    let tasks = Vec::new();
    Ok(tasks)
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
