use crate::task_shadowing::check_path_executable;
use crate::types::{TaskRunner, TaskDefinitionType};
use crate::package_manager::{PackageManager, detect_package_manager};

/// Get all available runners for a given task definition type
pub fn get_available_runners(definition_type: &TaskDefinitionType) -> Vec<TaskRunner> {
    match definition_type {
        TaskDefinitionType::Makefile => {
            if is_runner_available(&TaskRunner::Make) {
                vec![TaskRunner::Make]
            } else {
                vec![]
            }
        }
        TaskDefinitionType::PackageJson => {
            // Check all Node.js package managers
            let mut available = vec![];
            if let Some(pm) = detect_package_manager() {
                available.push(TaskRunner::Node(pm));
            }
            available
        }
        TaskDefinitionType::PyprojectToml => {
            let mut available = vec![];
            if is_runner_available(&TaskRunner::PythonUv) {
                available.push(TaskRunner::PythonUv);
            }
            if is_runner_available(&TaskRunner::PythonPoetry) {
                available.push(TaskRunner::PythonPoetry);
            }
            available
        }
        TaskDefinitionType::ShellScript => {
            // Shell scripts are always available
            vec![TaskRunner::ShellScript]
        }
    }
}

pub fn is_runner_available(runner: &TaskRunner) -> bool {
    match runner {
        TaskRunner::Make => check_path_executable("make").is_some(),
        TaskRunner::Node(pm) => check_path_executable(pm.command()).is_some(),
        TaskRunner::PythonUv => check_path_executable("uv").is_some(),
        TaskRunner::PythonPoetry => check_path_executable("poetry").is_some(),
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
        let available = get_available_runners(&TaskDefinitionType::ShellScript);
        assert_eq!(available, vec![TaskRunner::ShellScript]);
    }

    #[test]
    fn test_make_availability() {
        // Test make availability (depends on system)
        let make_available = check_path_executable("make").is_some();
        assert_eq!(is_runner_available(&TaskRunner::Make), make_available);
        
        let available = get_available_runners(&TaskDefinitionType::Makefile);
        if make_available {
            assert_eq!(available, vec![TaskRunner::Make]);
        } else {
            assert!(available.is_empty());
        }
    }

    #[test]
    fn test_node_package_managers() {
        // Test all package managers
        for pm in [PackageManager::Npm, PackageManager::Yarn, PackageManager::Bun] {
            let pm_available = check_path_executable(pm.command()).is_some();
            assert_eq!(
                is_runner_available(&TaskRunner::Node(pm.clone())),
                pm_available,
                "Availability check failed for package manager: {:?}",
                pm
            );
        }

        // Test available runners for package.json
        let available = get_available_runners(&TaskDefinitionType::PackageJson);
        if let Some(pm) = detect_package_manager() {
            assert_eq!(available, vec![TaskRunner::Node(pm)]);
        } else {
            assert!(available.is_empty());
        }
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

        // Test available runners for pyproject.toml
        let available = get_available_runners(&TaskDefinitionType::PyprojectToml);
        let mut expected = vec![];
        if uv_available {
            expected.push(TaskRunner::PythonUv);
        }
        if poetry_available {
            expected.push(TaskRunner::PythonPoetry);
        }
        assert_eq!(available, expected);
    }
} 