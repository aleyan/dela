use crate::environment::ENVIRONMENT;
use crate::types::ShadowType;
use std::path::Path;

/// Check if a name is a shell builtin
pub fn check_shell_builtin(name: &str) -> Option<ShadowType> {
    // Get current shell
    let shell = ENVIRONMENT.lock().unwrap().get_shell()?;
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
        "cd", "echo", "pwd", "export", "alias", "clear", "copy", "del",
        "dir", "exit", "get", "help", "history", "kill", "mkdir", "move",
        "popd", "pushd", "pwd", "read", "remove", "rename", "set", "start",
        "test", "type", "wait", "where", "write", "ls", "rm", "cp", "mv",
        "cat", "clear", "sleep", "sort", "tee", "write",
    ];

    if PWSH_BUILTINS.contains(&name) {
        Some(ShadowType::ShellBuiltin("pwsh".to_string()))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{reset_to_real_environment, set_test_environment, TestEnvironment};
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_check_zsh_builtin() {
        set_test_environment(TestEnvironment::new().with_shell("/bin/zsh"));

        // Test common zsh builtins
        for builtin in ["cd", "echo", "pwd", "export"] {
            let result = check_zsh_builtin(builtin);
            assert!(result.is_some(), "Expected {} to be a zsh builtin", builtin);
            assert_eq!(result.unwrap(), ShadowType::ShellBuiltin("zsh".to_string()));
        }

        // Test non-builtin
        assert!(check_zsh_builtin("definitely_not_a_builtin_123").is_none());

        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_check_bash_builtin() {
        set_test_environment(TestEnvironment::new().with_shell("/bin/bash"));

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

        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_check_fish_builtin() {
        set_test_environment(TestEnvironment::new().with_shell("/usr/bin/fish"));

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

        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_check_pwsh_builtin() {
        set_test_environment(TestEnvironment::new().with_shell("/usr/bin/pwsh"));

        // Test common PowerShell builtins
        for builtin in ["cd", "echo", "pwd", "get"] {
            let result = check_pwsh_builtin(builtin);
            assert!(
                result.is_some(),
                "Expected {} to be a PowerShell builtin",
                builtin
            );
            assert_eq!(
                result.unwrap(),
                ShadowType::ShellBuiltin("pwsh".to_string())
            );
        }

        // Test non-builtin
        assert!(check_pwsh_builtin("definitely_not_a_builtin_123").is_none());

        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_check_shell_builtin() {
        // Test with zsh
        set_test_environment(TestEnvironment::new().with_shell("/bin/zsh"));
        assert!(matches!(
            check_shell_builtin("cd"),
            Some(ShadowType::ShellBuiltin(shell)) if shell == "zsh"
        ));
        reset_to_real_environment();

        // Test with bash
        set_test_environment(TestEnvironment::new().with_shell("/bin/bash"));
        assert!(matches!(
            check_shell_builtin("cd"),
            Some(ShadowType::ShellBuiltin(shell)) if shell == "bash"
        ));
        reset_to_real_environment();

        // Test with fish
        set_test_environment(TestEnvironment::new().with_shell("/usr/bin/fish"));
        assert!(matches!(
            check_shell_builtin("cd"),
            Some(ShadowType::ShellBuiltin(shell)) if shell == "fish"
        ));
        reset_to_real_environment();

        // Test with pwsh
        set_test_environment(TestEnvironment::new().with_shell("/usr/bin/pwsh"));
        assert!(matches!(
            check_shell_builtin("cd"),
            Some(ShadowType::ShellBuiltin(shell)) if shell == "pwsh"
        ));
        reset_to_real_environment();

        // Test with unknown shell
        set_test_environment(TestEnvironment::new().with_shell("/bin/unknown_shell"));
        assert!(check_shell_builtin("cd").is_none());
        reset_to_real_environment();
    }
}
