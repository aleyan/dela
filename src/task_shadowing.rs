use once_cell::sync::Lazy;
use std::collections::HashSet;
use std::process::Command;
use std::sync::Mutex;
use crate::builtins::{check_shell_builtin, ShadowType};

// Global mock state for tests
static MOCK_EXECUTABLES: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));
static USE_MOCK: Lazy<Mutex<bool>> = Lazy::new(|| Mutex::new(false));

#[cfg(test)]
pub fn mock_executable(name: &str) {
    MOCK_EXECUTABLES.lock().unwrap().insert(name.to_string());
}

#[allow(dead_code)]
pub fn unmock_executable(name: &str) {
    MOCK_EXECUTABLES.lock().unwrap().remove(name);
}

#[cfg(test)]
pub fn enable_mock() {
    *USE_MOCK.lock().unwrap() = true;
}

#[allow(dead_code)]
pub fn disable_mock() {
    *USE_MOCK.lock().unwrap() = false;
}

#[cfg(test)]
pub fn reset_mock() {
    MOCK_EXECUTABLES.lock().unwrap().clear();
    *USE_MOCK.lock().unwrap() = false;
}

/// Check if a task name is shadowed by a shell builtin or PATH executable
pub fn check_shadowing(task_name: &str) -> Option<ShadowType> {
    // First check shell builtins
    if let Some(shadow) = check_shell_builtin(task_name) {
        return Some(shadow);
    }

    // Then check PATH executables
    check_path_executable(task_name)
}

/// Check if a command exists in PATH
pub fn check_path_executable(name: &str) -> Option<ShadowType> {
    // If mocking is enabled in tests, use mock data
    if cfg!(test) && *USE_MOCK.lock().unwrap() {
        if MOCK_EXECUTABLES.lock().unwrap().contains(name) {
            return Some(ShadowType::PathExecutable(format!("/mock/bin/{}", name)));
        }
        return None;
    }

    // Use 'which' command to find executable in PATH
    let output = Command::new("which").arg(name).output().ok()?;

    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Some(ShadowType::PathExecutable(path));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;
    use std::fs::File;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    fn setup_test_env(shell: &str) -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        env::set_var("SHELL", shell);
        temp_dir
    }

    fn create_fake_executable(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        File::create(&path).unwrap();
        #[cfg(unix)]
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    #[test]
    #[serial]
    fn test_check_path_executable() {
        let temp_dir = TempDir::new().unwrap();
        let old_path = env::var("PATH").unwrap_or_default();
        env::set_var(
            "PATH",
            format!("{}:{}", temp_dir.path().display(), old_path),
        );

        // Enable mocking
        reset_mock();
        enable_mock();

        // Test with mocked executables
        mock_executable("test_exe1");
        assert!(check_path_executable("test_exe1").is_some());
        assert_eq!(
            check_path_executable("test_exe1").unwrap(),
            ShadowType::PathExecutable("/mock/bin/test_exe1".to_string())
        );

        mock_executable("test_exe2");
        assert!(check_path_executable("test_exe2").is_some());
        assert_eq!(
            check_path_executable("test_exe2").unwrap(),
            ShadowType::PathExecutable("/mock/bin/test_exe2".to_string())
        );

        // Test non-existent executable
        assert!(check_path_executable("nonexistent_executable_123").is_none());

        reset_mock();

        // Restore PATH
        env::set_var("PATH", old_path);
    }

    #[test]
    #[serial]
    fn test_check_shadowing_precedence() {
        // Save original environment
        let original_shell = env::var("SHELL").ok();
        let original_path = env::var("PATH").unwrap_or_default();

        // Set up test environment with a known shell
        env::set_var("SHELL", "/bin/zsh");

        // Create a temporary directory and add it to PATH
        let temp_dir = TempDir::new().unwrap();
        let cd_path = create_fake_executable(temp_dir.path(), "cd");
        env::set_var(
            "PATH",
            format!("{}:{}", temp_dir.path().display(), original_path),
        );

        // Verify the fake executable exists and is in PATH
        assert!(cd_path.exists());

        // Test that builtin takes precedence
        let result = check_shadowing("cd");
        assert!(matches!(result, Some(ShadowType::ShellBuiltin(shell)) if shell == "zsh"));

        // Clean up environment
        match original_shell {
            Some(shell) => env::set_var("SHELL", shell),
            None => env::remove_var("SHELL"),
        }
        env::set_var("PATH", original_path);
    }

    #[test]
    #[serial]
    fn test_check_shadowing_with_invalid_shell() {
        let _temp_dir = setup_test_env("/bin/invalid_shell");

        // Enable mocking
        reset_mock();
        enable_mock();

        // Create a fake executable
        mock_executable("test_exe");

        // With invalid shell, should still detect PATH executables
        let result = check_shadowing("test_exe");
        assert!(matches!(result, Some(ShadowType::PathExecutable(_))));

        reset_mock();
    }

    #[test]
    #[serial]
    fn test_nonexistent_command() {
        let _temp_dir = setup_test_env("/bin/zsh");

        // Test completely nonexistent command
        let result = check_shadowing("nonexistentcommandxyz123");
        assert!(result.is_none());
    }
}
