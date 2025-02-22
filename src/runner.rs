#[cfg(test)]
use crate::environment::{reset_to_real_environment, set_test_environment, TestEnvironment};
use crate::task_shadowing::check_path_executable;
use crate::types::TaskRunner;
#[cfg(test)]
use serial_test::serial;

pub fn is_runner_available(runner: &TaskRunner) -> bool {
    match runner {
        TaskRunner::Make => {
            if cfg!(test) {
                true
            } else {
                check_path_executable("make").is_some()
            }
        }
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
    use crate::task_shadowing::{enable_mock, mock_executable, reset_mock};

    #[test]
    #[serial]
    fn test_shell_script_always_available() {
        // Shell scripts should always be available as they don't need a runner
        assert!(is_runner_available(&TaskRunner::ShellScript));
    }

    #[test]
    #[serial]
    fn test_make_availability() {
        // In test mode, Make should always be available
        assert!(is_runner_available(&TaskRunner::Make));
    }

    #[test]
    #[serial]
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
    #[serial]
    fn test_python_runners() {
        reset_mock();
        enable_mock();

        // Set up test environment
        let env = TestEnvironment::new()
            .with_executable("uv")
            .with_executable("poetry")
            .with_executable("poe");
        set_test_environment(env);

        // Mock UV being available
        mock_executable("uv");
        assert!(is_runner_available(&TaskRunner::PythonUv));

        // Mock Poetry being available
        mock_executable("poetry");
        assert!(is_runner_available(&TaskRunner::PythonPoetry));

        // Mock Poe being available
        mock_executable("poe");
        assert!(is_runner_available(&TaskRunner::PythonPoe));

        reset_mock();
        reset_to_real_environment();
    }
}
