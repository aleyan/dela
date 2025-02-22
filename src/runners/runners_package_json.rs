use crate::task_shadowing::check_path_executable;
use crate::types::TaskRunner;
use std::path::Path;

/// Detect which Node.js package manager to use based on lock files and available commands
pub fn detect_package_manager(dir: &Path) -> Option<TaskRunner> {
    let has_npm = check_path_executable("npm").is_some();
    let has_bun = check_path_executable("bun").is_some();
    let has_pnpm = check_path_executable("pnpm").is_some();
    let has_yarn = check_path_executable("yarn").is_some();

    if std::fs::metadata(dir.join("yarn.lock")).is_ok() {
        return Some(TaskRunner::NodeYarn);
    } else if std::fs::metadata(dir.join("package-lock.json")).is_ok() {
        return Some(TaskRunner::NodeNpm);
    } else if std::fs::metadata(dir.join("pnpm-lock.yaml")).is_ok() {
        return Some(TaskRunner::NodePnpm);
    } else if std::fs::metadata(dir.join("bun.lockb")).is_ok() {
        return Some(TaskRunner::NodeBun);
    }

    if has_bun {
        Some(TaskRunner::NodeBun)
    } else if has_npm {
        Some(TaskRunner::NodeNpm)
    } else if has_pnpm {
        Some(TaskRunner::NodePnpm)
    } else if has_yarn {
        Some(TaskRunner::NodeYarn)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{reset_to_real_environment, set_test_environment, TestEnvironment};
    use crate::task_shadowing::{disable_mock, enable_mock, mock_executable, reset_mock};
    use serial_test::serial;
    use std::fs::File;
    use tempfile::TempDir;

    fn create_lock_file(dir: &Path, filename: &str) {
        File::create(dir.join(filename)).unwrap();
    }

    #[test]
    fn test_detect_package_manager_with_lock_files() {
        let temp_dir = TempDir::new().unwrap();

        // Helper function to remove all lock files
        fn remove_all_lock_files(dir: &std::path::Path) {
            let _ = std::fs::remove_file(dir.join("package-lock.json"));
            let _ = std::fs::remove_file(dir.join("yarn.lock"));
            let _ = std::fs::remove_file(dir.join("pnpm-lock.yaml"));
            let _ = std::fs::remove_file(dir.join("bun.lockb"));
        }

        // Enable mocking
        reset_mock();
        enable_mock();

        // Test package-lock.json with npm available
        remove_all_lock_files(temp_dir.path());
        create_lock_file(temp_dir.path(), "package-lock.json");
        mock_executable("npm");
        assert_eq!(
            detect_package_manager(temp_dir.path()),
            Some(TaskRunner::NodeNpm)
        );

        // Test yarn.lock with yarn available
        remove_all_lock_files(temp_dir.path());
        create_lock_file(temp_dir.path(), "yarn.lock");
        mock_executable("yarn");
        assert_eq!(
            detect_package_manager(temp_dir.path()),
            Some(TaskRunner::NodeYarn)
        );

        // Test pnpm-lock.yaml with pnpm available
        remove_all_lock_files(temp_dir.path());
        create_lock_file(temp_dir.path(), "pnpm-lock.yaml");
        mock_executable("pnpm");
        assert_eq!(
            detect_package_manager(temp_dir.path()),
            Some(TaskRunner::NodePnpm)
        );

        // Test bun.lockb with bun available
        remove_all_lock_files(temp_dir.path());
        create_lock_file(temp_dir.path(), "bun.lockb");
        mock_executable("bun");
        assert_eq!(
            detect_package_manager(temp_dir.path()),
            Some(TaskRunner::NodeBun)
        );

        reset_mock();
    }

    #[test]
    #[serial]
    fn test_detect_package_manager_no_lock_files() {
        let temp_dir = TempDir::new().unwrap();

        // Test with only bun available
        let env = TestEnvironment::new().with_executable("bun");
        set_test_environment(env);
        assert_eq!(
            detect_package_manager(temp_dir.path()),
            Some(TaskRunner::NodeBun)
        );
        reset_to_real_environment();

        // Test with only npm available - should return NodeNpm since bun is not available
        let env = TestEnvironment::new().with_executable("npm");
        set_test_environment(env);
        assert_eq!(
            detect_package_manager(temp_dir.path()),
            Some(TaskRunner::NodeNpm)
        );
        reset_to_real_environment();

        // Test with both bun and npm available - should prefer bun
        let env = TestEnvironment::new()
            .with_executable("bun")
            .with_executable("npm");
        set_test_environment(env);
        assert_eq!(
            detect_package_manager(temp_dir.path()),
            Some(TaskRunner::NodeBun)
        );
        reset_to_real_environment();

        // Test with no package managers
        let env = TestEnvironment::new();
        set_test_environment(env);
        assert_eq!(detect_package_manager(temp_dir.path()), None);
        reset_to_real_environment();
    }

    #[test]
    fn test_detect_package_manager_multiple_available() {
        let temp_dir = TempDir::new().unwrap();

        // Enable mocking
        reset_mock();
        enable_mock();

        // Mock multiple package managers being available
        mock_executable("npm");
        mock_executable("bun");
        mock_executable("pnpm");
        mock_executable("yarn");

        // Test preference order with no lock files
        assert_eq!(
            detect_package_manager(temp_dir.path()),
            Some(TaskRunner::NodeBun)
        );

        // Test that lock files take precedence
        create_lock_file(temp_dir.path(), "package-lock.json");
        assert_eq!(
            detect_package_manager(temp_dir.path()),
            Some(TaskRunner::NodeNpm)
        );

        reset_mock();
    }
}
