#[cfg(test)]
use crate::environment::{reset_to_real_environment, set_test_environment, TestEnvironment};
use crate::task_shadowing::check_path_executable;
use crate::types::TaskRunner;
#[cfg(test)]
use serial_test::serial;

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
        TaskRunner::Task => check_path_executable("task").is_some(),
        TaskRunner::Maven => check_path_executable("mvn").is_some(),
        TaskRunner::Gradle => {
            check_path_executable("gradle").is_some()
                || check_path_executable("./gradlew").is_some()
        }
        TaskRunner::Act => check_path_executable("act").is_some(),
        TaskRunner::DockerCompose => check_path_executable("docker-compose").is_some(),
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
        let env = TestEnvironment::new()
            .with_executable("npm")
            .with_executable("yarn")
            .with_executable("bun");

        set_test_environment(env.clone());
        assert!(is_runner_available(&TaskRunner::NodeNpm));
        reset_mock();

        set_test_environment(env.clone());
        assert!(is_runner_available(&TaskRunner::NodeYarn));
        reset_mock();

        set_test_environment(env.clone());
        assert!(is_runner_available(&TaskRunner::NodeBun));
        reset_mock();

        reset_to_real_environment();
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

    #[test]
    #[serial]
    fn test_maven_runner() {
        reset_mock();
        enable_mock();

        // Set up test environment without Maven
        let env = TestEnvironment::new();
        set_test_environment(env);

        // Maven should not be available yet
        assert!(!is_runner_available(&TaskRunner::Maven));

        // Now set up environment with Maven
        let env_with_maven = TestEnvironment::new().with_executable("mvn");
        set_test_environment(env_with_maven);

        // Maven should now be available
        assert!(is_runner_available(&TaskRunner::Maven));

        // Clean up
        reset_mock();
        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_gradle_runner() {
        reset_mock();
        enable_mock();

        // Set up test environment without Gradle
        let env = TestEnvironment::new();
        set_test_environment(env);

        // Gradle should not be available yet
        assert!(!is_runner_available(&TaskRunner::Gradle));

        // Now set up environment with Gradle
        let env_with_gradle = TestEnvironment::new().with_executable("gradle");
        set_test_environment(env_with_gradle);

        // Gradle should now be available
        assert!(is_runner_available(&TaskRunner::Gradle));

        // Test with Gradle wrapper
        let env_with_wrapper = TestEnvironment::new().with_executable("./gradlew");
        set_test_environment(env_with_wrapper);

        // Gradle wrapper should also work
        assert!(is_runner_available(&TaskRunner::Gradle));

        // Clean up
        reset_mock();
        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_act_runner() {
        // Set up test environment with act
        reset_mock();
        enable_mock();
        let env = TestEnvironment::new().with_executable("act");
        set_test_environment(env);

        // Act should be available
        assert!(is_runner_available(&TaskRunner::Act));

        // Set up test environment without act
        let env = TestEnvironment::new();
        set_test_environment(env);

        // Act should not be available
        assert!(!is_runner_available(&TaskRunner::Act));

        reset_mock();
        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_docker_compose_runner() {
        // Set up test environment with docker-compose
        reset_mock();
        enable_mock();
        let env = TestEnvironment::new().with_executable("docker-compose");
        set_test_environment(env);

        // Docker Compose should be available
        assert!(is_runner_available(&TaskRunner::DockerCompose));

        // Set up test environment without docker-compose
        let env = TestEnvironment::new();
        set_test_environment(env);

        // Docker Compose should not be available
        assert!(!is_runner_available(&TaskRunner::DockerCompose));

        reset_mock();
        reset_to_real_environment();
    }
}
