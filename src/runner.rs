use crate::task_shadowing::check_path_executable;
use crate::types::TaskRunner;

pub fn is_runner_available(runner: &TaskRunner) -> bool {
    match runner {
        TaskRunner::Make => check_path_executable("make").is_some(),
        TaskRunner::NodeNpm => check_path_executable("npm").is_some(),
        TaskRunner::NodeYarn => check_path_executable("yarn").is_some(),
        TaskRunner::NodePnpm => check_path_executable("pnpm").is_some(),
        TaskRunner::NodeBun => check_path_executable("bun").is_some(),
        TaskRunner::PythonUv => check_path_executable("uv").is_some(),
        TaskRunner::PythonPoetry => check_path_executable("poetry").is_some(),
        TaskRunner::PythonPoe => check_path_executable("poe").is_some(),
        TaskRunner::ShellScript => true, // Shell scripts don't need a runner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_script_always_available() {
        // Shell scripts should always be available as they don't need a runner
        assert!(is_runner_available(&TaskRunner::ShellScript));
    }

    #[test]
    fn test_make_availability() {
        // Test make availability (depends on system)
        let make_available = check_path_executable("make").is_some();
        assert_eq!(is_runner_available(&TaskRunner::Make), make_available);
    }

    #[test]
    fn test_node_package_managers() {
        // Test all package managers
        let npm_available = check_path_executable("npm").is_some();
        assert_eq!(is_runner_available(&TaskRunner::NodeNpm), npm_available);

        let yarn_available = check_path_executable("yarn").is_some();
        assert_eq!(is_runner_available(&TaskRunner::NodeYarn), yarn_available);

        let bun_available = check_path_executable("bun").is_some();
        assert_eq!(is_runner_available(&TaskRunner::NodeBun), bun_available);
    }

    #[test]
    fn test_python_runners() {
        // Test individual runners
        let uv_available = check_path_executable("uv").is_some();
        assert_eq!(is_runner_available(&TaskRunner::PythonUv), uv_available);

        let poetry_available = check_path_executable("poetry").is_some();
        assert_eq!(
            is_runner_available(&TaskRunner::PythonPoetry),
            poetry_available
        );
    }
}
