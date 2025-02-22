use once_cell::sync::Lazy;
use std::collections::HashSet;
use std::process::Command;
use std::sync::Mutex;
use crate::builtins::check_shell_builtin;
use crate::types::{ShadowType, ENVIRONMENT};

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
    ENVIRONMENT.lock().unwrap()
        .check_executable(name)
        .map(ShadowType::PathExecutable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{TestEnvironment, set_test_environment, reset_to_real_environment};
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_check_path_executable() {
        // Set up test environment with some executables
        let env = TestEnvironment::new()
            .with_executable("test_exe1")
            .with_executable("test_exe2");
        set_test_environment(env);

        // Test existing executables
        assert_eq!(
            check_path_executable("test_exe1"),
            Some(ShadowType::PathExecutable("/mock/bin/test_exe1".to_string()))
        );
        assert_eq!(
            check_path_executable("test_exe2"),
            Some(ShadowType::PathExecutable("/mock/bin/test_exe2".to_string()))
        );

        // Test non-existent executable
        assert!(check_path_executable("nonexistent_executable_123").is_none());

        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_check_shadowing_precedence() {
        // Set up test environment with both shell and executables
        let env = TestEnvironment::new()
            .with_shell("/bin/zsh")
            .with_executable("cd");
        set_test_environment(env);

        // Test that builtin takes precedence
        let result = check_shadowing("cd");
        assert!(matches!(result, Some(ShadowType::ShellBuiltin(shell)) if shell == "zsh"));

        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_check_shadowing_with_invalid_shell() {
        // Set up test environment with unknown shell but with executables
        let env = TestEnvironment::new()
            .with_shell("/bin/invalid_shell")
            .with_executable("test_exe");
        set_test_environment(env);

        // With invalid shell, should still detect PATH executables
        let result = check_shadowing("test_exe");
        assert!(matches!(result, Some(ShadowType::PathExecutable(_))));

        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_nonexistent_command() {
        // Set up test environment with a shell but no executables
        let env = TestEnvironment::new().with_shell("/bin/zsh");
        set_test_environment(env);

        // Test completely nonexistent command
        let result = check_shadowing("nonexistentcommandxyz123");
        assert!(result.is_none());

        reset_to_real_environment();
    }
}
