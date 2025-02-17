use std::env;
use std::path::Path;
use std::process::Command;

/// Information about what shadows a task name
#[derive(Debug, Clone, PartialEq)]
pub enum ShadowType {
    /// Task is shadowed by a shell builtin
    ShellBuiltin(String), // shell name
    /// Task is shadowed by an executable in PATH
    PathExecutable(String), // full path
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

/// Check if a name is a shell builtin
fn check_shell_builtin(name: &str) -> Option<ShadowType> {
    // Get current shell
    let shell = env::var("SHELL").ok()?;
    let shell_path = Path::new(&shell);
    let shell_name = shell_path.file_name()?.to_str()?;

    match shell_name {
        "zsh" => check_zsh_builtin(name),
        "bash" => check_bash_builtin(name),
        "fish" => check_fish_builtin(name),
        "pwsh" => check_pwsh_builtin(name),
        _ => None,
    }
}

/// Check if a name is a zsh builtin
fn check_zsh_builtin(name: &str) -> Option<ShadowType> {
    const ZSH_BUILTINS: &[&str] = &[
        "cd",
        "echo",
        "pwd",
        "export",
        "alias",
        "bg",
        "bindkey",
        "builtin",
        "command",
        "declare",
        "dirs",
        "disable",
        "disown",
        "enable",
        "eval",
        "exec",
        "exit",
        "fg",
        "getopts",
        "hash",
        "jobs",
        "kill",
        "let",
        "local",
        "popd",
        "print",
        "pushd",
        "read",
        "readonly",
        "return",
        "set",
        "setopt",
        "shift",
        "source",
        "suspend",
        "test",
        "times",
        "trap",
        "type",
        "typeset",
        "ulimit",
        "umask",
        "unalias",
        "unfunction",
        "unhash",
        "unset",
        "unsetopt",
        "wait",
        "whence",
        "where",
        "which",
        ".",
        ":",
        "[",
        "ls",
        "test",
    ];

    if ZSH_BUILTINS.contains(&name) {
        Some(ShadowType::ShellBuiltin("zsh".to_string()))
    } else {
        None
    }
}

/// Check if a name is a bash builtin
fn check_bash_builtin(name: &str) -> Option<ShadowType> {
    const BASH_BUILTINS: &[&str] = &[
        "cd",
        "echo",
        "pwd",
        "export",
        "alias",
        "bg",
        "bind",
        "break",
        "builtin",
        "caller",
        "command",
        "compgen",
        "complete",
        "continue",
        "declare",
        "dirs",
        "disown",
        "enable",
        "eval",
        "exec",
        "exit",
        "fc",
        "fg",
        "getopts",
        "hash",
        "help",
        "history",
        "jobs",
        "kill",
        "let",
        "local",
        "logout",
        "mapfile",
        "popd",
        "printf",
        "pushd",
        "pwd",
        "read",
        "readarray",
        "readonly",
        "return",
        "set",
        "shift",
        "shopt",
        "source",
        "suspend",
        "test",
        "times",
        "trap",
        "type",
        "typeset",
        "ulimit",
        "umask",
        "unalias",
        "unset",
        "wait",
        ".",
        ":",
        "[",
        "ls",
        "test",
    ];

    if BASH_BUILTINS.contains(&name) {
        Some(ShadowType::ShellBuiltin("bash".to_string()))
    } else {
        None
    }
}

/// Check if a name is a fish builtin
fn check_fish_builtin(name: &str) -> Option<ShadowType> {
    const FISH_BUILTINS: &[&str] = &[
        "cd",
        "echo",
        "pwd",
        "export",
        "alias",
        "bg",
        "bind",
        "block",
        "breakpoint",
        "builtin",
        "case",
        "command",
        "commandline",
        "complete",
        "contains",
        "count",
        "dirh",
        "dirs",
        "disown",
        "emit",
        "eval",
        "exec",
        "exit",
        "fg",
        "fish_config",
        "fish_update_completions",
        "funced",
        "funcsave",
        "functions",
        "help",
        "history",
        "isatty",
        "jobs",
        "math",
        "nextd",
        "open",
        "popd",
        "prevd",
        "printf",
        "pushd",
        "pwd",
        "random",
        "read",
        "realpath",
        "set",
        "set_color",
        "source",
        "status",
        "string",
        "test",
        "time",
        "trap",
        "type",
        "ulimit",
        "umask",
        "vared",
        ".",
        ":",
        "[",
        "ls",
        "test",
    ];

    if FISH_BUILTINS.contains(&name) {
        Some(ShadowType::ShellBuiltin("fish".to_string()))
    } else {
        None
    }
}

/// Check if a name is a PowerShell builtin
fn check_pwsh_builtin(name: &str) -> Option<ShadowType> {
    #[rustfmt::skip]
    const PWSH_BUILTINS: &[&str] = &[
        "cd",
        "echo", 
        "pwd",
        "export",
        "alias",
        "clear",
        "copy",
        "del",
        "dir",
        "exit",
        "get",
        "help",
        "history",
        "kill",
        "mkdir",
        "move",
        "popd",
        "pushd",
        "pwd",
        "read",
        "remove",
        "rename",
        "set",
        "start",
        "test",
        "type",
        "wait",
        "where",
        "write",
        "ls",
        "rm",
        "cp",
        "mv",
        "cat",
        "clear",
        "sleep",
        "sort",
        "tee",
        "write",
    ];

    if PWSH_BUILTINS.contains(&name) {
        Some(ShadowType::ShellBuiltin("pwsh".to_string()))
    } else {
        None
    }
}

/// Check if a command exists in PATH
pub fn check_path_executable(name: &str) -> Option<ShadowType> {
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
    use std::fs::File;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    fn setup_test_env(shell: &str) -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        env::set_var("SHELL", shell);
        temp_dir
    }

    fn create_fake_executable(dir: &Path, name: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        File::create(&path).unwrap();
        #[cfg(unix)]
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    #[test]
    #[serial]
    fn test_check_zsh_builtin() {
        let _temp_dir = setup_test_env("/bin/zsh");

        // Test common zsh builtins
        for builtin in ["cd", "echo", "pwd", "export"] {
            let result = check_zsh_builtin(builtin);
            assert!(result.is_some(), "Expected {} to be a zsh builtin", builtin);
            assert_eq!(result.unwrap(), ShadowType::ShellBuiltin("zsh".to_string()));
        }

        // Test non-builtin
        assert!(check_zsh_builtin("definitely_not_a_builtin_123").is_none());
    }

    #[test]
    #[serial]
    fn test_check_bash_builtin() {
        let _temp_dir = setup_test_env("/bin/bash");

        // Test common bash builtins
        for builtin in ["cd", "echo", "pwd", "export"] {
            let result = check_bash_builtin(builtin);
            assert!(
                result.is_some(),
                "Expected {} to be a bash builtin",
                builtin
            );
            assert_eq!(
                result.unwrap(),
                ShadowType::ShellBuiltin("bash".to_string())
            );
        }

        // Test non-builtin
        assert!(check_bash_builtin("definitely_not_a_builtin_123").is_none());
    }

    #[test]
    #[serial]
    fn test_check_fish_builtin() {
        let _temp_dir = setup_test_env("/usr/bin/fish");

        // Test common fish builtins
        for builtin in ["cd", "echo", "pwd", "set"] {
            let result = check_fish_builtin(builtin);
            assert!(
                result.is_some(),
                "Expected {} to be a fish builtin",
                builtin
            );
            assert_eq!(
                result.unwrap(),
                ShadowType::ShellBuiltin("fish".to_string())
            );
        }

        // Test non-builtin
        assert!(check_fish_builtin("definitely_not_a_builtin_123").is_none());
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

        // Create fake executables
        let path1 = create_fake_executable(temp_dir.path(), "test_exe1");
        let path2 = create_fake_executable(temp_dir.path(), "test_exe2");

        // Test executables we created
        let result1 = check_path_executable("test_exe1");
        assert!(result1.is_some());
        assert_eq!(
            result1.unwrap(),
            ShadowType::PathExecutable(path1.to_str().unwrap().to_string())
        );

        let result2 = check_path_executable("test_exe2");
        assert!(result2.is_some());
        assert_eq!(
            result2.unwrap(),
            ShadowType::PathExecutable(path2.to_str().unwrap().to_string())
        );

        // Test non-existent executable
        assert!(check_path_executable("nonexistent_executable_123").is_none());

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

        // Create a fake executable
        let temp_dir = TempDir::new().unwrap();
        let old_path = env::var("PATH").unwrap_or_default();
        env::set_var(
            "PATH",
            format!("{}:{}", temp_dir.path().display(), old_path),
        );

        let _path = create_fake_executable(temp_dir.path(), "test_exe");

        // With invalid shell, should still detect PATH executables
        let result = check_shadowing("test_exe");
        assert!(matches!(result, Some(ShadowType::PathExecutable(_))));

        // Restore PATH
        env::set_var("PATH", old_path);
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
