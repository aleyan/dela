use crate::task_shadowing::check_path_executable;
use crate::types::TaskRunner;

pub fn is_runner_available(runner: &TaskRunner) -> bool {
    match runner {
        TaskRunner::Make => check_path_executable("make").is_some(),
        TaskRunner::Node(Some(pm)) => check_path_executable(pm.command()).is_some(),
        TaskRunner::Node(None) => check_path_executable("npm").is_some(),
        TaskRunner::PythonUv => check_path_executable("uv").is_some(),
        TaskRunner::PythonPoetry => check_path_executable("poetry").is_some(),
        TaskRunner::ShellScript => true, // Shell scripts don't need a runner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package_manager::PackageManager;

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
        // Test npm (default) availability
        let npm_available = check_path_executable("npm").is_some();
        assert_eq!(is_runner_available(&TaskRunner::Node(None)), npm_available);

        // Test all package managers
        for pm in [PackageManager::Npm, PackageManager::Yarn, PackageManager::Bun] {
            let pm_available = check_path_executable(pm.command()).is_some();
            assert_eq!(
                is_runner_available(&TaskRunner::Node(Some(pm.clone()))),
                pm_available,
                "Availability check failed for package manager: {:?}",
                pm
            );
        }
    }

    #[test]
    fn test_python_runners() {
        // Test uv availability
        let uv_available = check_path_executable("uv").is_some();
        assert_eq!(is_runner_available(&TaskRunner::PythonUv), uv_available);

        // Test poetry availability
        let poetry_available = check_path_executable("poetry").is_some();
        assert_eq!(
            is_runner_available(&TaskRunner::PythonPoetry),
            poetry_available
        );
    }
} 