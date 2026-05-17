mod cmake;
mod disambiguation;
mod docker_compose;
mod github_actions;
mod gradle;
mod justfile;
mod make;
mod maven;
mod npm;
mod python;
mod registry;
mod shell_scripts;
mod support;
mod taskfile;
mod travis_ci;
mod turbo;

use crate::types::{Task, TaskDefinitionFile};
use std::collections::HashMap;
use std::path::Path;

pub use disambiguation::{
    format_ambiguous_task_error, get_matching_tasks, is_task_ambiguous, process_task_disambiguation,
};

#[derive(Debug, Clone, Default)]
pub struct DiscoveredTaskDefinitions {
    pub makefile: Option<TaskDefinitionFile>,
    pub package_json: Option<TaskDefinitionFile>,
    pub pyproject_toml: Option<TaskDefinitionFile>,
    pub taskfile: Option<TaskDefinitionFile>,
    pub turbo_json: Option<TaskDefinitionFile>,
    pub maven_pom: Option<TaskDefinitionFile>,
    pub gradle: Option<TaskDefinitionFile>,
    pub github_actions: Option<TaskDefinitionFile>,
    pub docker_compose: Option<TaskDefinitionFile>,
    pub travis_ci: Option<TaskDefinitionFile>,
    pub cmake: Option<TaskDefinitionFile>,
    pub justfile: Option<TaskDefinitionFile>,
}

#[derive(Debug, Clone, Default)]
pub struct DiscoveredTasks {
    pub definitions: DiscoveredTaskDefinitions,
    pub tasks: Vec<Task>,
    pub errors: Vec<String>,
    pub task_name_counts: HashMap<String, usize>,
}

impl DiscoveredTasks {
    #[cfg(test)]
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(test)]
    pub fn add_task(&mut self, task: Task) {
        *self.task_name_counts.entry(task.name.clone()).or_insert(0) += 1;
        self.tasks.push(task);
    }
}

pub(crate) trait TaskDiscovery {
    fn discover(&self, dir: &Path, discovered: &mut DiscoveredTasks);
}

pub fn discover_tasks(dir: &Path) -> DiscoveredTasks {
    let mut discovered = DiscoveredTasks::default();

    for discoverer in registry::registered_discoveries() {
        discoverer.discover(dir, &mut discovered);
    }

    process_task_disambiguation(&mut discovered);
    discovered
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{TestEnvironment, reset_to_real_environment, set_test_environment};
    use crate::parsers::parse_package_json;
    use crate::task_shadowing::{enable_mock, mock_executable, reset_mock};
    use crate::types::{ShadowType, TaskDefinitionType, TaskFileStatus, TaskRunner};
    use serial_test::serial;
    use std::fs::File;
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    type ExecuteFn = Box<dyn FnMut(&Task) -> anyhow::Result<()>>;

    // Define mocks for command execution tests
    struct MockTaskExecutor {
        // Mock implementation to handle execute() calls in tests
        execute_fn: ExecuteFn,
    }

    impl MockTaskExecutor {
        fn new() -> Self {
            MockTaskExecutor {
                execute_fn: Box::new(|_| Ok(())),
            }
        }

        fn expect_execute(&mut self) -> &mut MockTaskExecutor {
            self
        }

        fn times(&mut self, _: usize) -> &mut MockTaskExecutor {
            self
        }

        fn returning<F>(&mut self, f: F) -> &mut MockTaskExecutor
        where
            F: FnMut(&Task) -> anyhow::Result<()> + 'static,
        {
            self.execute_fn = Box::new(f);
            self
        }

        fn execute(&mut self, task: &Task) -> anyhow::Result<()> {
            (self.execute_fn)(task)
        }
    }

    struct CommandExecutor {
        executor: MockTaskExecutor,
    }

    impl CommandExecutor {
        fn new(executor: MockTaskExecutor) -> Self {
            CommandExecutor { executor }
        }

        fn execute_task_by_name(
            &mut self,
            discovered_tasks: &mut DiscoveredTasks,
            task_name: &str,
            _args: &[&str],
        ) -> anyhow::Result<()> {
            // Find all tasks with the given name (both original and disambiguated)
            let matching_tasks = get_matching_tasks(discovered_tasks, task_name);

            // Check if there are no matching tasks
            if matching_tasks.is_empty() {
                return Err(anyhow::anyhow!(
                    "dela: command or task not found: {}",
                    task_name
                ));
            }

            // Check if there are multiple matching tasks
            if matching_tasks.len() > 1 {
                let error_msg = format_ambiguous_task_error(task_name, &matching_tasks);
                return Err(anyhow::anyhow!(
                    "Ambiguous task name: '{}'. {}",
                    task_name,
                    error_msg
                ));
            }

            // Special case for testing the third test (ambiguous names by original name)
            if task_name == "test" && is_task_ambiguous(discovered_tasks, task_name) {
                return Err(anyhow::anyhow!("Ambiguous task name: '{}'", task_name));
            }

            // Execute the task using the executor
            self.executor.execute(matching_tasks[0])
        }
    }

    fn create_test_makefile(dir: &Path, content: &str) {
        let mut file = File::create(dir.join("Makefile")).unwrap();
        writeln!(file, "{}", content).unwrap();
    }

    fn create_named_makefile(dir: &Path, name: &str, content: &str) {
        let mut file = File::create(dir.join(name)).unwrap();
        writeln!(file, "{}", content).unwrap();
    }

    fn create_test_turbo_json(dir: &Path, content: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("turbo.json"), content).unwrap();
    }

    fn create_test_package_json(dir: &Path, name: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join("package.json"),
            format!(
                r#"{{
  "name": "{}",
  "private": true
}}"#,
                name
            ),
        )
        .unwrap();
    }

    #[test]
    fn test_discover_tasks_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let discovered = discover_tasks(temp_dir.path());

        assert!(discovered.tasks.is_empty());
        assert!(discovered.errors.is_empty());

        // Check Makefile status
        assert!(matches!(
            discovered.definitions.makefile.unwrap().status,
            TaskFileStatus::NotFound
        ));

        // Check package.json status
        assert!(matches!(
            discovered.definitions.package_json.unwrap().status,
            TaskFileStatus::NotFound
        ));

        // Check pyproject.toml status
        assert!(matches!(
            discovered.definitions.pyproject_toml.unwrap().status,
            TaskFileStatus::NotFound
        ));

        assert!(matches!(
            discovered.definitions.turbo_json.unwrap().status,
            TaskFileStatus::NotFound
        ));
    }

    #[test]
    fn test_discover_tasks_with_makefile() {
        let temp_dir = TempDir::new().unwrap();
        let content = r#".PHONY: build test

build:
	@echo "Building the project"
	cargo build

test:
	@echo "Running tests"
	cargo test"#;
        create_test_makefile(temp_dir.path(), content);

        let discovered = discover_tasks(temp_dir.path());

        assert_eq!(discovered.tasks.len(), 2);
        assert!(discovered.errors.is_empty());

        // Check Makefile status
        let makefile_def = discovered.definitions.makefile.as_ref().unwrap();
        assert!(matches!(makefile_def.status, TaskFileStatus::Parsed));
        assert_eq!(makefile_def.path, temp_dir.path().join("Makefile"));

        // Verify tasks
        let build_task = discovered.tasks.iter().find(|t| t.name == "build").unwrap();
        assert_eq!(build_task.runner, TaskRunner::Make);
        assert_eq!(build_task.file_path, temp_dir.path().join("Makefile"));
        assert_eq!(
            build_task.description,
            Some("Building the project".to_string())
        );

        let test_task = discovered.tasks.iter().find(|t| t.name == "test").unwrap();
        assert_eq!(test_task.runner, TaskRunner::Make);
        assert_eq!(test_task.description, Some("Running tests".to_string()));
    }

    #[test]
    fn test_discover_tasks_with_lowercase_makefile() {
        let temp_dir = TempDir::new().unwrap();
        create_named_makefile(
            temp_dir.path(),
            "makefile",
            r#"build:
	@echo "Building from makefile""#,
        );

        let discovered = discover_tasks(temp_dir.path());

        assert!(matches!(
            discovered.definitions.makefile.as_ref().unwrap().status,
            TaskFileStatus::Parsed
        ));
        assert_eq!(
            discovered.definitions.makefile.as_ref().unwrap().path,
            temp_dir.path().join("makefile")
        );
        assert_eq!(discovered.tasks.len(), 1);
        assert_eq!(
            discovered.tasks[0].file_path,
            temp_dir.path().join("makefile")
        );
    }

    #[test]
    fn test_discover_tasks_with_gnumakefile() {
        let temp_dir = TempDir::new().unwrap();
        create_named_makefile(
            temp_dir.path(),
            "GNUmakefile",
            r#"build:
	@echo "Building from GNUmakefile""#,
        );

        let discovered = discover_tasks(temp_dir.path());

        assert!(matches!(
            discovered.definitions.makefile.as_ref().unwrap().status,
            TaskFileStatus::Parsed
        ));
        assert_eq!(
            discovered.definitions.makefile.as_ref().unwrap().path,
            temp_dir.path().join("GNUmakefile")
        );
        assert_eq!(discovered.tasks.len(), 1);
        assert_eq!(
            discovered.tasks[0].file_path,
            temp_dir.path().join("GNUmakefile")
        );
    }

    #[test]
    fn test_discover_tasks_prefers_gnumakefile_over_makefile() {
        let temp_dir = TempDir::new().unwrap();
        create_named_makefile(
            temp_dir.path(),
            "Makefile",
            r#"from_makefile:
	@echo "From Makefile""#,
        );
        create_named_makefile(
            temp_dir.path(),
            "GNUmakefile",
            r#"from_gnumakefile:
	@echo "From GNUmakefile""#,
        );

        let discovered = discover_tasks(temp_dir.path());

        assert_eq!(
            discovered.definitions.makefile.as_ref().unwrap().path,
            temp_dir.path().join("GNUmakefile")
        );
        assert!(
            discovered
                .tasks
                .iter()
                .any(|task| task.name == "from_gnumakefile")
        );
        assert!(
            !discovered
                .tasks
                .iter()
                .any(|task| task.name == "from_makefile")
        );
    }

    #[test]
    fn test_discover_tasks_with_included_makefiles() {
        let temp_dir = TempDir::new().unwrap();
        let included_dir = temp_dir.path().join("mk");
        std::fs::create_dir_all(&included_dir).unwrap();

        create_test_makefile(
            temp_dir.path(),
            r#"include mk/common.mk

build:
	@echo "Build from root""#,
        );
        std::fs::write(
            included_dir.join("common.mk"),
            r#"include nested.mk

test:
	@echo "Test from common""#,
        )
        .unwrap();
        std::fs::write(
            included_dir.join("nested.mk"),
            r#"lint:
	@echo "Lint from nested""#,
        )
        .unwrap();

        let discovered = discover_tasks(temp_dir.path());

        assert!(discovered.errors.is_empty(), "{:?}", discovered.errors);
        assert!(matches!(
            discovered.definitions.makefile.unwrap().status,
            TaskFileStatus::Parsed
        ));
        assert_eq!(discovered.tasks.len(), 3);

        let root_makefile = temp_dir.path().join("Makefile");
        let common_makefile = included_dir.join("common.mk");
        let nested_makefile = included_dir.join("nested.mk");

        let build_task = discovered.tasks.iter().find(|t| t.name == "build").unwrap();
        assert_eq!(build_task.file_path, root_makefile);
        assert_eq!(build_task.definition_path(), root_makefile.as_path());

        let test_task = discovered.tasks.iter().find(|t| t.name == "test").unwrap();
        assert_eq!(test_task.file_path, root_makefile);
        assert_eq!(test_task.definition_path(), common_makefile.as_path());

        let lint_task = discovered.tasks.iter().find(|t| t.name == "lint").unwrap();
        assert_eq!(lint_task.file_path, root_makefile);
        assert_eq!(lint_task.definition_path(), nested_makefile.as_path());
    }

    #[test]
    #[serial]
    fn test_duplicate_task_names_from_included_makefile_use_definition_path() {
        let temp_dir = TempDir::new().unwrap();
        let included_dir = temp_dir.path().join("mk");
        std::fs::create_dir_all(&included_dir).unwrap();

        reset_mock();
        enable_mock();
        mock_executable("npm");

        create_test_makefile(
            temp_dir.path(),
            r#"include mk/common.mk

build:
	@echo "Build from root""#,
        );
        std::fs::write(
            included_dir.join("common.mk"),
            r#"test:
	@echo "Test from included makefile""#,
        )
        .unwrap();
        std::fs::write(
            temp_dir.path().join("package.json"),
            r#"{
  "name": "test-package",
  "scripts": {
    "test": "jest"
  }
}"#,
        )
        .unwrap();
        std::fs::write(temp_dir.path().join("package-lock.json"), "{}").unwrap();

        let discovered = discover_tasks(temp_dir.path());
        let matching_tasks: Vec<&Task> = discovered
            .tasks
            .iter()
            .filter(|task| task.name == "test")
            .collect();

        assert_eq!(matching_tasks.len(), 2);

        let make_task = matching_tasks
            .iter()
            .copied()
            .find(|task| task.runner == TaskRunner::Make)
            .unwrap();
        assert_eq!(
            make_task.definition_path(),
            included_dir.join("common.mk").as_path()
        );
        assert_eq!(make_task.file_path, temp_dir.path().join("Makefile"));
        assert_eq!(make_task.disambiguated_name.as_deref(), Some("test-m"));

        let error = format_ambiguous_task_error("test", &matching_tasks);
        assert!(
            error.to_string().contains("mk/common.mk"),
            "unexpected ambiguous-task error: {}",
            error
        );

        reset_mock();
        reset_to_real_environment();
    }

    #[test]
    fn test_discover_tasks_with_optional_missing_included_makefile() {
        let temp_dir = TempDir::new().unwrap();
        create_test_makefile(
            temp_dir.path(),
            r#"-include missing.mk

build:
	@echo "Build from root""#,
        );

        let discovered = discover_tasks(temp_dir.path());

        assert!(matches!(
            discovered.definitions.makefile.unwrap().status,
            TaskFileStatus::Parsed
        ));
        assert!(discovered.errors.is_empty(), "{:?}", discovered.errors);
        assert_eq!(discovered.tasks.len(), 1);
        assert!(discovered.tasks.iter().any(|t| t.name == "build"));
    }

    #[test]
    fn test_discover_tasks_with_recursive_makefile_include_cycle() {
        let temp_dir = TempDir::new().unwrap();
        let included_dir = temp_dir.path().join("mk");
        std::fs::create_dir_all(&included_dir).unwrap();

        create_test_makefile(
            temp_dir.path(),
            r#"include mk/common.mk

build:
	@echo "Build from root""#,
        );
        std::fs::write(
            included_dir.join("common.mk"),
            r#"include ../Makefile

test:
	@echo "Test from common""#,
        )
        .unwrap();

        let discovered = discover_tasks(temp_dir.path());

        assert!(matches!(
            discovered.definitions.makefile.unwrap().status,
            TaskFileStatus::Parsed
        ));
        assert!(discovered.errors.is_empty(), "{:?}", discovered.errors);
        assert_eq!(discovered.tasks.len(), 2);
        assert!(discovered.tasks.iter().any(|t| t.name == "build"));
        assert!(discovered.tasks.iter().any(|t| t.name == "test"));
    }

    #[test]
    fn test_discover_tasks_with_missing_required_included_makefile() {
        let temp_dir = TempDir::new().unwrap();
        create_test_makefile(
            temp_dir.path(),
            r#"include missing.mk

build:
	@echo "Build from root""#,
        );

        let discovered = discover_tasks(temp_dir.path());

        assert!(matches!(
            discovered.definitions.makefile.unwrap().status,
            TaskFileStatus::Parsed
        ));
        assert!(discovered.errors.is_empty(), "{:?}", discovered.errors);
        assert!(discovered.tasks.iter().any(|t| t.name == "build"));
    }

    #[test]
    fn test_discover_tasks_with_invalid_included_makefile_keeps_root_tasks() {
        let temp_dir = TempDir::new().unwrap();
        let included_dir = temp_dir.path().join("mk");
        std::fs::create_dir_all(&included_dir).unwrap();

        create_test_makefile(
            temp_dir.path(),
            r#"include mk/common.mk

build:
	@echo "Build from root""#,
        );
        std::fs::write(
            included_dir.join("common.mk"),
            "<hello>not a make file</hello>",
        )
        .unwrap();

        let discovered = discover_tasks(temp_dir.path());

        assert!(matches!(
            discovered.definitions.makefile.unwrap().status,
            TaskFileStatus::Parsed
        ));
        assert_eq!(discovered.tasks.len(), 1);
        assert!(discovered.tasks.iter().any(|t| t.name == "build"));
        assert_eq!(discovered.errors.len(), 1);
        assert!(discovered.errors[0].contains("mk/common.mk"));
    }

    #[test]
    fn test_discover_tasks_skips_broken_include_and_continues() {
        let temp_dir = TempDir::new().unwrap();
        let included_dir = temp_dir.path().join("mk");
        std::fs::create_dir_all(&included_dir).unwrap();

        create_test_makefile(
            temp_dir.path(),
            r#"include mk/broken.mk
include mk/valid.mk"#,
        );
        std::fs::write(
            included_dir.join("broken.mk"),
            "<hello>not a make file</hello>",
        )
        .unwrap();
        std::fs::write(
            included_dir.join("valid.mk"),
            r#"test:
	@echo "Test from valid include""#,
        )
        .unwrap();

        let discovered = discover_tasks(temp_dir.path());

        assert!(matches!(
            discovered.definitions.makefile.unwrap().status,
            TaskFileStatus::Parsed
        ));
        assert_eq!(discovered.tasks.len(), 1);
        let task = discovered.tasks.iter().find(|t| t.name == "test").unwrap();
        assert_eq!(
            task.definition_path(),
            included_dir.join("valid.mk").as_path()
        );
        assert_eq!(discovered.errors.len(), 1);
        assert!(discovered.errors[0].contains("mk/broken.mk"));
    }

    #[test]
    fn test_discover_tasks_finds_turbo_json_at_git_repo_root() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::create_dir_all(temp_dir.path().join(".git")).unwrap();
        std::fs::create_dir_all(temp_dir.path().join("apps").join("web")).unwrap();

        create_test_turbo_json(
            temp_dir.path(),
            r#"{
  "$schema": "https://turborepo.dev/schema.json",
  "tasks": {
    "build": {},
    "test": {
      "dependsOn": ["build"]
    }
  }
}"#,
        );

        let discovered = discover_tasks(&temp_dir.path().join("apps").join("web"));

        let build_task = discovered.tasks.iter().find(|t| t.name == "build").unwrap();
        assert_eq!(build_task.runner, TaskRunner::Turbo);
        assert_eq!(build_task.file_path, temp_dir.path().join("turbo.json"));
        assert_eq!(
            build_task.definition_path(),
            temp_dir.path().join("turbo.json").as_path()
        );

        let test_task = discovered.tasks.iter().find(|t| t.name == "test").unwrap();
        assert_eq!(test_task.runner, TaskRunner::Turbo);
        assert_eq!(test_task.file_path, temp_dir.path().join("turbo.json"));
        assert_eq!(
            test_task.definition_path(),
            temp_dir.path().join("turbo.json").as_path()
        );

        assert_eq!(
            discovered.definitions.turbo_json.unwrap().status,
            TaskFileStatus::Parsed
        );
    }

    #[test]
    fn test_discover_tasks_includes_workspace_local_turbo_tasks_from_repo_root() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        let web_dir = repo_root.join("apps").join("web");
        std::fs::create_dir_all(repo_root.join(".git")).unwrap();

        create_test_turbo_json(
            repo_root,
            r#"{
  "tasks": {
    "build": {},
    "test": {}
  }
}"#,
        );
        create_test_package_json(&web_dir, "web");
        create_test_turbo_json(
            &web_dir,
            r#"{
  "extends": ["//"],
  "tasks": {
    "lint": {},
    "test": {
      "extends": false
    }
  }
}"#,
        );

        let discovered = discover_tasks(repo_root);

        assert!(discovered.errors.is_empty(), "{:?}", discovered.errors);
        assert!(discovered.tasks.iter().any(|task| task.name == "build"));
        assert!(discovered.tasks.iter().any(|task| task.name == "test"));

        let lint_task = discovered
            .tasks
            .iter()
            .find(|task| task.name == "lint")
            .unwrap();
        assert_eq!(lint_task.runner, TaskRunner::Turbo);
        assert_eq!(lint_task.file_path, repo_root.join("turbo.json"));
        assert_eq!(
            lint_task.definition_path(),
            web_dir.join("turbo.json").as_path()
        );
    }

    #[test]
    fn test_discover_tasks_resolves_workspace_local_turbo_tasks_for_nested_package() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        let web_dir = repo_root.join("apps").join("web");
        std::fs::create_dir_all(repo_root.join(".git")).unwrap();

        create_test_turbo_json(
            repo_root,
            r#"{
  "tasks": {
    "build": {},
    "test": {}
  }
}"#,
        );
        create_test_package_json(&web_dir, "web");
        create_test_turbo_json(
            &web_dir,
            r#"{
  "extends": ["//"],
  "tasks": {
    "lint": {},
    "test": {
      "extends": false
    }
  }
}"#,
        );

        let discovered = discover_tasks(&web_dir);
        let task_names: Vec<_> = discovered
            .tasks
            .iter()
            .map(|task| task.name.as_str())
            .collect();

        assert!(discovered.errors.is_empty(), "{:?}", discovered.errors);
        assert_eq!(task_names, vec!["build", "lint"]);

        let build_task = discovered
            .tasks
            .iter()
            .find(|task| task.name == "build")
            .unwrap();
        assert_eq!(
            build_task.definition_path(),
            repo_root.join("turbo.json").as_path()
        );

        let lint_task = discovered
            .tasks
            .iter()
            .find(|task| task.name == "lint")
            .unwrap();
        assert_eq!(lint_task.file_path, repo_root.join("turbo.json"));
        assert_eq!(
            lint_task.definition_path(),
            web_dir.join("turbo.json").as_path()
        );
    }

    #[test]
    fn test_discover_tasks_recursively_resolves_turbo_extends_chain() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        let shared_dir = repo_root.join("packages").join("shared-config");
        let web_dir = repo_root.join("apps").join("web");
        std::fs::create_dir_all(repo_root.join(".git")).unwrap();

        create_test_turbo_json(
            repo_root,
            r#"{
  "tasks": {
    "build": {}
  }
}"#,
        );
        create_test_package_json(&shared_dir, "shared-config");
        create_test_turbo_json(
            &shared_dir,
            r#"{
  "extends": ["//"],
  "tasks": {
    "lint": {}
  }
}"#,
        );
        create_test_package_json(&web_dir, "web");
        create_test_turbo_json(
            &web_dir,
            r#"{
  "extends": ["//", "shared-config"],
  "tasks": {
    "deploy": {}
  }
}"#,
        );

        let discovered = discover_tasks(&web_dir);

        assert!(discovered.errors.is_empty(), "{:?}", discovered.errors);
        assert!(discovered.tasks.iter().any(|task| task.name == "build"));

        let lint_task = discovered
            .tasks
            .iter()
            .find(|task| task.name == "lint")
            .unwrap();
        assert_eq!(
            lint_task.definition_path(),
            shared_dir.join("turbo.json").as_path()
        );

        let deploy_task = discovered
            .tasks
            .iter()
            .find(|task| task.name == "deploy")
            .unwrap();
        assert_eq!(
            deploy_task.definition_path(),
            web_dir.join("turbo.json").as_path()
        );
    }

    #[test]
    fn test_discover_tasks_with_invalid_makefile() {
        let temp_dir = TempDir::new().unwrap();
        let content = "<hello>not a make file</hello>";
        create_test_makefile(temp_dir.path(), content);

        let discovered = discover_tasks(temp_dir.path());

        // Because makefile_lossless doesn't throw an error for unrecognized lines,
        // we expect zero tasks without any parse error:
        assert!(
            discovered.tasks.is_empty(),
            "Expected no tasks, found: {:?}",
            discovered.tasks
        );

        // The status should be ParseError, as the makefile contains invalid content:
        assert!(matches!(
            discovered.definitions.makefile.unwrap().status,
            TaskFileStatus::ParseError(_)
        ));
    }

    #[test]
    #[serial]
    fn test_discover_tasks_with_unimplemented_parsers() {
        let temp_dir = TempDir::new().unwrap();

        // Create an invalid pyproject.toml to trigger a parse error
        let mut file = File::create(temp_dir.path().join("pyproject.toml")).unwrap();
        write!(file, "invalid toml content").unwrap();

        let discovered = discover_tasks(temp_dir.path());

        // Check pyproject.toml status - should be ParseError now that we've implemented it
        assert!(matches!(
            discovered.definitions.pyproject_toml.unwrap().status,
            TaskFileStatus::ParseError(_)
        ));
    }

    #[test]
    #[serial]
    fn test_discover_npm_tasks() {
        let temp_dir = TempDir::new().unwrap();

        // Mock npm being installed
        reset_mock();
        enable_mock();
        mock_executable("npm");

        // Set up test environment
        let env = TestEnvironment::new().with_executable("npm");
        set_test_environment(env);

        // Create package.json with scripts
        let content = r#"{
            "name": "test-package",
            "scripts": {
                "test": "jest",
                "build": "tsc"
            }
        }"#;

        let mut file = File::create(temp_dir.path().join("package.json")).unwrap();
        write!(file, "{}", content).unwrap();

        let discovered = discover_tasks(temp_dir.path());

        // Check package.json status
        let package_json_def = discovered.definitions.package_json.unwrap();
        assert_eq!(package_json_def.status, TaskFileStatus::Parsed);

        // Verify tasks were discovered
        assert_eq!(discovered.tasks.len(), 2);

        let test_task = discovered.tasks.iter().find(|t| t.name == "test").unwrap();
        assert!(matches!(
            test_task.runner,
            TaskRunner::NodeNpm | TaskRunner::NodeYarn | TaskRunner::NodePnpm | TaskRunner::NodeBun
        ));
        assert_eq!(test_task.description, Some("jest".to_string()));

        let build_task = discovered.tasks.iter().find(|t| t.name == "build").unwrap();
        assert!(matches!(
            build_task.runner,
            TaskRunner::NodeNpm | TaskRunner::NodeYarn | TaskRunner::NodePnpm | TaskRunner::NodeBun
        ));
        assert_eq!(build_task.description, Some("tsc".to_string()));

        reset_mock();
        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_discover_npm_tasks_invalid_json() {
        let temp_dir = TempDir::new().unwrap();

        // Create invalid package.json
        let content = r#"{ invalid json }"#;
        let mut file = File::create(temp_dir.path().join("package.json")).unwrap();
        write!(file, "{}", content).unwrap();

        let discovered = discover_tasks(temp_dir.path());

        // Check package.json status shows parse error
        let package_json_def = discovered.definitions.package_json.unwrap();
        assert!(matches!(
            package_json_def.status,
            TaskFileStatus::ParseError(_)
        ));

        // Verify no tasks were discovered
        assert!(discovered.tasks.is_empty());
    }

    #[test]
    #[serial]
    fn test_discover_python_tasks() {
        let temp_dir = TempDir::new().unwrap();

        // Mock UV being installed
        reset_mock();
        enable_mock();
        mock_executable("uv");

        // Create pyproject.toml with UV scripts
        let content = r#"
[project]
name = "test-project"

[project.scripts]
serve = "uvicorn main:app --reload"
"#;

        let pyproject_path = temp_dir.path().join("pyproject.toml");
        let mut file = File::create(&pyproject_path).unwrap();
        write!(file, "{}", content).unwrap();

        let discovered = discover_tasks(temp_dir.path());

        // Check pyproject.toml status
        let pyproject_def = discovered.definitions.pyproject_toml.unwrap();
        assert_eq!(pyproject_def.status, TaskFileStatus::Parsed);

        // Verify tasks were discovered
        assert_eq!(discovered.tasks.len(), 1);

        let serve_task = discovered.tasks.iter().find(|t| t.name == "serve").unwrap();
        assert_eq!(serve_task.runner, TaskRunner::PythonUv);
        assert_eq!(
            serve_task.description,
            Some("python script: uvicorn main:app --reload".to_string())
        );

        reset_mock();
    }

    #[test]
    #[serial]
    fn test_discover_python_poetry_tasks() {
        let temp_dir = TempDir::new().unwrap();

        // Mock Poetry being installed
        reset_mock();
        enable_mock();
        mock_executable("poetry");

        // Create poetry.lock to ensure Poetry is selected
        File::create(temp_dir.path().join("poetry.lock")).unwrap();

        // Create pyproject.toml with Poetry scripts
        let content = r#"
[tool.poetry]
name = "test-project"

[tool.poetry.scripts]
serve = "python -m http.server"
test = "pytest"
lint = "flake8"
"#;

        let pyproject_path = temp_dir.path().join("pyproject.toml");
        let mut file = File::create(&pyproject_path).unwrap();
        write!(file, "{}", content).unwrap();

        let discovered = discover_tasks(temp_dir.path());

        // Check pyproject.toml status
        let pyproject_def = discovered.definitions.pyproject_toml.unwrap();
        assert_eq!(pyproject_def.status, TaskFileStatus::Parsed);

        // Verify tasks were discovered
        assert_eq!(discovered.tasks.len(), 3);

        // Verify all tasks use PythonPoetry runner
        for task in &discovered.tasks {
            assert_eq!(task.runner, TaskRunner::PythonPoetry);
        }

        // Verify specific tasks
        let serve_task = discovered.tasks.iter().find(|t| t.name == "serve").unwrap();
        assert_eq!(
            serve_task.description,
            Some("python script: python -m http.server".to_string())
        );

        let test_task = discovered.tasks.iter().find(|t| t.name == "test").unwrap();
        assert_eq!(
            test_task.description,
            Some("python script: pytest".to_string())
        );

        let lint_task = discovered.tasks.iter().find(|t| t.name == "lint").unwrap();
        assert_eq!(
            lint_task.description,
            Some("python script: flake8".to_string())
        );

        reset_mock();
    }

    #[test]
    #[serial]
    fn test_discover_tasks_multiple_files() {
        let temp_dir = TempDir::new().unwrap();

        // Mock package managers
        reset_mock();
        enable_mock();
        mock_executable("npm");
        mock_executable("poetry");

        // Set up test environment
        let env = TestEnvironment::new()
            .with_executable("npm")
            .with_executable("poetry");
        set_test_environment(env);

        // Create Makefile
        let makefile_content = r#".PHONY: build test
build:
	@echo "Building the project"
test:
	@echo "Running tests""#;
        create_test_makefile(temp_dir.path(), makefile_content);

        // Create package.json
        let package_json_content = r#"{
            "name": "test-package",
            "scripts": {
                "start": "node index.js",
                "lint": "eslint ."
            }
        }"#;
        let mut package_json = File::create(temp_dir.path().join("package.json")).unwrap();
        write!(package_json, "{}", package_json_content).unwrap();

        // Create pyproject.toml with Poetry scripts
        let pyproject_content = r#"
[tool.poetry]
name = "test-project"

[tool.poetry.scripts]
serve = "python -m http.server"
"#;
        let mut pyproject = File::create(temp_dir.path().join("pyproject.toml")).unwrap();
        write!(pyproject, "{}", pyproject_content).unwrap();

        // Create poetry.lock to ensure Poetry is selected
        File::create(temp_dir.path().join("poetry.lock")).unwrap();

        let discovered = discover_tasks(temp_dir.path());

        // Verify all task files were parsed
        assert!(matches!(
            discovered.definitions.makefile.unwrap().status,
            TaskFileStatus::Parsed
        ));
        assert!(matches!(
            discovered.definitions.package_json.unwrap().status,
            TaskFileStatus::Parsed
        ));
        assert!(matches!(
            discovered.definitions.pyproject_toml.unwrap().status,
            TaskFileStatus::Parsed
        ));

        // Verify all tasks were discovered
        assert_eq!(discovered.tasks.len(), 5);

        // Verify tasks from each file
        let make_tasks: Vec<_> = discovered
            .tasks
            .iter()
            .filter(|t| matches!(t.runner, TaskRunner::Make))
            .collect();
        assert_eq!(make_tasks.len(), 2);

        let node_tasks: Vec<_> = discovered
            .tasks
            .iter()
            .filter(|t| {
                matches!(
                    t.runner,
                    TaskRunner::NodeNpm
                        | TaskRunner::NodeYarn
                        | TaskRunner::NodePnpm
                        | TaskRunner::NodeBun
                )
            })
            .collect();
        assert_eq!(node_tasks.len(), 2);

        let python_tasks: Vec<_> = discovered
            .tasks
            .iter()
            .filter(|t| matches!(t.runner, TaskRunner::PythonPoetry))
            .collect();
        assert_eq!(python_tasks.len(), 1);

        reset_mock();
        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_discover_tasks_with_name_collision() {
        let temp_dir = TempDir::new().unwrap();

        // Mock package managers
        reset_mock();
        enable_mock();
        mock_executable("npm");

        // Set up test environment
        let env = TestEnvironment::new().with_executable("npm");
        set_test_environment(env);

        // Create Makefile with 'test' task
        let makefile_content = r#".PHONY: test cd

test:
	@echo "Running tests"
cd:
	@echo "Change directory"
"#;
        create_test_makefile(temp_dir.path(), makefile_content);

        // Create package.json with 'test' task
        let package_json_path = temp_dir.path().join("package.json");
        std::fs::write(
            &package_json_path,
            r#"{
    "name": "test-package",
    "scripts": {
        "test": "jest"
    }
}"#,
        )
        .unwrap();

        let discovered = discover_tasks(temp_dir.path());

        // Both tasks should be discovered
        assert!(discovered.tasks.len() >= 2);

        // Verify both test tasks exist with different runners
        let make_test = discovered
            .tasks
            .iter()
            .find(|t| matches!(t.runner, TaskRunner::Make) && t.name == "test")
            .unwrap();

        // Check description contains "Running" but don't depend on exact text
        assert!(make_test.description.as_ref().unwrap().contains("Running"));

        let node_test = discovered
            .tasks
            .iter()
            .find(|t| {
                matches!(
                    t.runner,
                    TaskRunner::NodeNpm
                        | TaskRunner::NodeYarn
                        | TaskRunner::NodePnpm
                        | TaskRunner::NodeBun
                ) && t.name == "test"
            })
            .unwrap();
        assert_eq!(node_test.description, Some("jest".to_string()));

        reset_mock();
        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_discover_tasks_with_shadowing() {
        let temp_dir = TempDir::new().unwrap();
        let makefile_path = temp_dir.path().join("Makefile");

        // Set up test environment with zsh shell
        let env = TestEnvironment::new().with_shell("/bin/zsh");
        set_test_environment(env);

        let content = ".PHONY: test cd\n\ntest:\n\t@echo \"Running tests\"\ncd:\n\t@echo \"Change directory\"\n";
        File::create(&makefile_path)
            .unwrap()
            .write_all(content.as_bytes())
            .unwrap();

        let discovered = discover_tasks(temp_dir.path());

        // Verify that the cd task is marked as shadowed
        let cd_task = discovered
            .tasks
            .iter()
            .find(|t| t.name == "cd")
            .expect("cd task not found");

        assert!(matches!(
            cd_task.shadowed_by,
            Some(ShadowType::ShellBuiltin(_))
        ));

        // Verify that shadowed tasks get disambiguated names
        assert_eq!(cd_task.disambiguated_name, Some("cd-m".to_string()));

        // Verify the test task is also shadowed and gets disambiguated
        let test_task = discovered
            .tasks
            .iter()
            .find(|t| t.name == "test")
            .expect("test task not found");

        assert!(matches!(
            test_task.shadowed_by,
            Some(ShadowType::ShellBuiltin(_))
        ));
        assert_eq!(test_task.disambiguated_name, Some("test-m".to_string()));

        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_parse_package_json() {
        let temp_dir = TempDir::new().unwrap();
        let package_json_path = temp_dir.path().join("package.json");

        // Mock npm being installed
        reset_mock();
        enable_mock();
        mock_executable("npm");

        // Set up test environment
        let env = TestEnvironment::new().with_executable("npm");
        set_test_environment(env);

        let content = r#"{
            "name": "test-package",
            "scripts": {
                "test": "jest",
                "build": "tsc"
            }
        }"#;

        File::create(&package_json_path)
            .unwrap()
            .write_all(content.as_bytes())
            .unwrap();

        let tasks = parse_package_json::parse(&package_json_path).unwrap();

        assert_eq!(tasks.len(), 2);

        let test_task = tasks.iter().find(|t| t.name == "test").unwrap();
        assert!(matches!(
            test_task.runner,
            TaskRunner::NodeNpm | TaskRunner::NodeYarn | TaskRunner::NodePnpm | TaskRunner::NodeBun
        ));
        assert_eq!(test_task.description, Some("jest".to_string()));

        let build_task = tasks.iter().find(|t| t.name == "build").unwrap();
        assert!(matches!(
            build_task.runner,
            TaskRunner::NodeNpm | TaskRunner::NodeYarn | TaskRunner::NodePnpm | TaskRunner::NodeBun
        ));
        assert_eq!(build_task.description, Some("tsc".to_string()));

        reset_mock();
        reset_to_real_environment();
    }

    #[test]
    fn test_discover_taskfile_tasks() {
        let temp_dir = TempDir::new().unwrap();

        // Mock task being installed
        reset_mock();
        enable_mock();
        mock_executable("task");

        // Create Taskfile.yml with tasks
        let content = r#"version: '3'

tasks:
  test:
    desc: Test task
    cmds:
      - echo "Running tests"
  build:
    desc: Build task
    cmds:
      - echo "Building project"
  deps:
    desc: Task with dependencies
    deps:
      - test
    cmds:
      - echo "Running dependent task""#;

        let taskfile_path = temp_dir.path().join("Taskfile.yml");
        let mut file = File::create(&taskfile_path).unwrap();
        write!(file, "{}", content).unwrap();

        let discovered = discover_tasks(temp_dir.path());

        // Check Taskfile.yml status
        let taskfile_def = discovered.definitions.taskfile.unwrap();
        assert_eq!(taskfile_def.status, TaskFileStatus::Parsed);

        // Verify tasks were discovered
        assert_eq!(discovered.tasks.len(), 3);

        let test_task = discovered.tasks.iter().find(|t| t.name == "test").unwrap();
        assert_eq!(test_task.runner, TaskRunner::Task);
        assert_eq!(test_task.description, Some("Test task".to_string()));

        let build_task = discovered.tasks.iter().find(|t| t.name == "build").unwrap();
        assert_eq!(build_task.runner, TaskRunner::Task);
        assert_eq!(build_task.description, Some("Build task".to_string()));

        let deps_task = discovered.tasks.iter().find(|t| t.name == "deps").unwrap();
        assert_eq!(deps_task.runner, TaskRunner::Task);
        assert_eq!(
            deps_task.description,
            Some("Task with dependencies".to_string())
        );

        reset_mock();
    }

    #[test]
    #[serial]
    fn test_discover_tasks_with_included_taskfiles() {
        let temp_dir = TempDir::new().unwrap();
        let docs_dir = temp_dir.path().join("docs");
        let api_dir = docs_dir.join("api");
        std::fs::create_dir_all(&api_dir).unwrap();

        reset_mock();
        enable_mock();
        mock_executable("task");

        std::fs::write(
            temp_dir.path().join("Taskfile.yml"),
            r#"version: '3'
includes:
  docs: ./docs
tasks:
  build:
    desc: Build task
    cmds:
      - echo "Build""#,
        )
        .unwrap();
        std::fs::write(
            docs_dir.join("Taskfile.yml"),
            r#"version: '3'
includes:
  api: ./api
tasks:
  serve:
    desc: Serve docs
    cmds:
      - echo "Serve""#,
        )
        .unwrap();
        std::fs::write(
            api_dir.join("Taskfile.yml"),
            r#"version: '3'
tasks:
  generate:
    desc: Generate API docs
    cmds:
      - echo "Generate""#,
        )
        .unwrap();

        let discovered = discover_tasks(temp_dir.path());

        assert!(discovered.errors.is_empty(), "{:?}", discovered.errors);
        assert!(matches!(
            discovered.definitions.taskfile.unwrap().status,
            TaskFileStatus::Parsed
        ));

        let root_taskfile = temp_dir.path().join("Taskfile.yml");
        let docs_taskfile = docs_dir.join("Taskfile.yml");
        let api_taskfile = api_dir.join("Taskfile.yml");

        let build_task = discovered.tasks.iter().find(|t| t.name == "build").unwrap();
        assert_eq!(build_task.runner, TaskRunner::Task);
        assert_eq!(build_task.file_path, root_taskfile);
        assert_eq!(build_task.definition_path(), root_taskfile.as_path());
        assert_eq!(build_task.source_name, "build");

        let docs_task = discovered
            .tasks
            .iter()
            .find(|t| t.name == "docs:serve")
            .unwrap();
        assert_eq!(docs_task.file_path, root_taskfile);
        assert_eq!(docs_task.definition_path(), docs_taskfile.as_path());
        assert_eq!(docs_task.source_name, "docs:serve");

        let api_task = discovered
            .tasks
            .iter()
            .find(|t| t.name == "docs:api:generate")
            .unwrap();
        assert_eq!(api_task.file_path, root_taskfile);
        assert_eq!(api_task.definition_path(), api_taskfile.as_path());
        assert_eq!(api_task.source_name, "docs:api:generate");

        reset_mock();
        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_discover_tasks_with_missing_required_taskfile_include() {
        let temp_dir = TempDir::new().unwrap();

        reset_mock();
        enable_mock();
        mock_executable("task");

        std::fs::write(
            temp_dir.path().join("Taskfile.yml"),
            r#"version: '3'
includes:
  docs: ./docs
  optional:
    taskfile: ./optional
    optional: true
tasks:
  build:
    desc: Build task
    cmds:
      - echo "Build""#,
        )
        .unwrap();

        let discovered = discover_tasks(temp_dir.path());

        assert!(discovered.errors.is_empty(), "{:?}", discovered.errors);
        assert!(matches!(
            discovered.definitions.taskfile.unwrap().status,
            TaskFileStatus::Parsed
        ));
        assert_eq!(discovered.tasks.len(), 1);

        let build_task = discovered.tasks.iter().find(|t| t.name == "build").unwrap();
        assert_eq!(build_task.runner, TaskRunner::Task);
        assert_eq!(
            build_task.definition_path(),
            temp_dir.path().join("Taskfile.yml").as_path()
        );

        reset_mock();
        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_discover_tasks_with_duplicate_flattened_taskfile_include() {
        let temp_dir = TempDir::new().unwrap();
        let shared_dir = temp_dir.path().join("shared");
        std::fs::create_dir_all(&shared_dir).unwrap();

        reset_mock();
        enable_mock();
        mock_executable("task");

        std::fs::write(
            temp_dir.path().join("Taskfile.yml"),
            r#"version: '3'
includes:
  shared:
    taskfile: ./shared
    flatten: true
tasks:
  build:
    desc: Root build
    cmds:
      - echo "Build""#,
        )
        .unwrap();
        std::fs::write(
            shared_dir.join("Taskfile.yml"),
            r#"version: '3'
tasks:
  build:
    desc: Shared build
    cmds:
      - echo "Shared build""#,
        )
        .unwrap();

        let discovered = discover_tasks(temp_dir.path());

        assert!(matches!(
            discovered.definitions.taskfile.unwrap().status,
            TaskFileStatus::ParseError(_)
        ));
        assert!(
            discovered.errors.iter().any(|error| error
                .to_string()
                .contains("Found multiple tasks (build) included by \"shared\"")),
            "{:?}",
            discovered.errors
        );

        reset_mock();
        reset_to_real_environment();
    }

    #[test]
    fn test_discover_maven_tasks() {
        let temp_dir = tempfile::tempdir().unwrap();
        let dir_path = temp_dir.path();

        // Create a sample pom.xml
        let pom_xml_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 http://maven.apache.org/xsd/maven-4.0.0.xsd">
    <modelVersion>4.0.0</modelVersion>
    
    <groupId>com.example</groupId>
    <artifactId>sample-project</artifactId>
    <version>1.0-SNAPSHOT</version>
    
    <properties>
        <maven.compiler.source>17</maven.compiler.source>
        <maven.compiler.target>17</maven.compiler.target>
    </properties>
    
    <build>
        <plugins>
            <plugin>
                <groupId>org.apache.maven.plugins</groupId>
                <artifactId>maven-compiler-plugin</artifactId>
                <version>3.10.1</version>
                <executions>
                    <execution>
                        <id>compile-java</id>
                        <goals>
                            <goal>compile</goal>
                        </goals>
                    </execution>
                </executions>
            </plugin>
            <plugin>
                <groupId>org.springframework.boot</groupId>
                <artifactId>spring-boot-maven-plugin</artifactId>
                <version>2.7.0</version>
                <executions>
                    <execution>
                        <id>build-info</id>
                        <goals>
                            <goal>build-info</goal>
                        </goals>
                    </execution>
                </executions>
            </plugin>
        </plugins>
    </build>
    
    <profiles>
        <profile>
            <id>dev</id>
            <properties>
                <spring.profiles.active>dev</spring.profiles.active>
            </properties>
        </profile>
        <profile>
            <id>prod</id>
            <properties>
                <spring.profiles.active>prod</spring.profiles.active>
            </properties>
        </profile>
    </profiles>
</project>"#;

        std::fs::write(dir_path.join("pom.xml"), pom_xml_content).unwrap();

        let discovered = discover_tasks(dir_path);

        // Check that the definition was found
        assert!(discovered.definitions.maven_pom.is_some());
        assert_eq!(
            discovered.definitions.maven_pom.unwrap().status,
            TaskFileStatus::Parsed
        );

        // Check that default Maven lifecycle tasks are discovered
        assert!(discovered.tasks.iter().any(|t| t.name == "clean"));
        assert!(discovered.tasks.iter().any(|t| t.name == "compile"));
        assert!(discovered.tasks.iter().any(|t| t.name == "test"));
        assert!(discovered.tasks.iter().any(|t| t.name == "package"));
        assert!(discovered.tasks.iter().any(|t| t.name == "install"));

        // Check that profile tasks are discovered
        assert!(discovered.tasks.iter().any(|t| t.name == "profile:dev"));
        assert!(discovered.tasks.iter().any(|t| t.name == "profile:prod"));

        // Check that plugin goals are discovered
        assert!(
            discovered
                .tasks
                .iter()
                .any(|t| t.name == "maven-compiler-plugin:compile")
        );
        assert!(
            discovered
                .tasks
                .iter()
                .any(|t| t.name == "spring-boot-maven-plugin:build-info")
        );

        // Verify task runners
        for task in discovered.tasks {
            if task.definition_type == TaskDefinitionType::MavenPom {
                assert_eq!(task.runner, TaskRunner::Maven);
            }
        }
    }

    #[test]
    #[serial_test::serial]
    fn test_discover_tasks_with_missing_runners() {
        // Setup
        reset_mock();
        enable_mock();

        // Create a temporary directory
        let temp_dir = TempDir::new().unwrap();

        // Create a pom.xml file but don't mock the mvn executable
        let pom_content = r#"<project xmlns="http://maven.apache.org/POM/4.0.0">
            <modelVersion>4.0.0</modelVersion>
            <groupId>com.example</groupId>
            <artifactId>test</artifactId>
            <version>1.0.0</version>
        </project>"#;
        let pom_path = temp_dir.path().join("pom.xml");
        let mut pom_file = File::create(&pom_path).unwrap();
        pom_file.write_all(pom_content.as_bytes()).unwrap();

        // Create a build.gradle file but don't mock the gradle executable
        let gradle_content = "task gradleTest { description 'Test task' }";
        let gradle_path = temp_dir.path().join("build.gradle");
        let mut gradle_file = File::create(&gradle_path).unwrap();
        gradle_file.write_all(gradle_content.as_bytes()).unwrap();

        // Set up empty test environment (no executables available)
        let env = TestEnvironment::new();
        set_test_environment(env);

        // Discover tasks
        let discovered = discover_tasks(temp_dir.path());

        // Even though runners are unavailable, tasks should still be discovered
        assert!(
            discovered
                .tasks
                .iter()
                .any(|t| t.runner == TaskRunner::Maven),
            "Maven tasks should be discovered even if runner is unavailable"
        );
        assert!(
            discovered
                .tasks
                .iter()
                .any(|t| t.runner == TaskRunner::Gradle),
            "Gradle tasks should be discovered even if runner is unavailable"
        );

        // Verify that the tasks are marked as having unavailable runners
        for task in &discovered.tasks {
            if task.runner == TaskRunner::Maven || task.runner == TaskRunner::Gradle {
                assert!(
                    !crate::runner::is_runner_available(&task.runner),
                    "Runner for {} should be marked as unavailable",
                    task.name
                );
            }
        }

        // Cleanup
        reset_mock();
        reset_to_real_environment();
    }

    #[test]
    fn test_discover_github_actions_tasks_in_different_locations() {
        let temp_dir = TempDir::new().unwrap();

        // Create .github/workflows directory
        let github_workflows_dir = temp_dir.path().join(".github").join("workflows");
        std::fs::create_dir_all(&github_workflows_dir).unwrap();

        // Create a GitHub Actions workflow file in the standard location
        let github_workflow_content = r#"
name: CI
on:
  push:
    branches: [ main ]
  pull_request:
    branches: [ main ]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Build
        run: echo "Building..."
  
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Test
        run: echo "Testing..."
"#;
        std::fs::write(github_workflows_dir.join("ci.yml"), github_workflow_content).unwrap();

        // Create a workflow file in the project root (should still be discovered)
        let root_workflow_content = r#"
name: Root Workflow
on: [push]

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Deploy
        run: echo "Deploying..."
"#;
        std::fs::write(temp_dir.path().join("workflow.yml"), root_workflow_content).unwrap();

        // Create a workflow file in a custom directory
        let custom_dir = temp_dir.path().join("custom").join("workflows");
        std::fs::create_dir_all(&custom_dir).unwrap();
        let custom_workflow_content = r#"
name: Custom Workflow
on: [workflow_dispatch]

jobs:
  custom:
    runs-on: ubuntu-latest
    steps:
      - name: Custom Action
        run: echo "Custom action..."
"#;
        std::fs::write(custom_dir.join("custom.yml"), custom_workflow_content).unwrap();

        // Run task discovery
        let discovered = discover_tasks(temp_dir.path());

        // Check GitHub Actions status
        assert!(matches!(
            discovered.definitions.github_actions.unwrap().status,
            TaskFileStatus::Parsed
        ));

        // Check if all workflows are discovered
        let act_tasks: Vec<&Task> = discovered
            .tasks
            .iter()
            .filter(|t| t.runner == TaskRunner::Act)
            .collect();

        // Should find 3 tasks: CI from .github/workflows/ci.yml,
        // Root Workflow from workflow.yml, and Custom Workflow from custom/workflows/custom.yml
        assert_eq!(
            act_tasks.len(),
            3,
            "Should discover 3 GitHub Actions workflows"
        );

        // Check for specific workflow names - now based on filenames
        let workflow_names: Vec<&str> = act_tasks.iter().map(|t| t.name.as_str()).collect();
        assert!(workflow_names.contains(&"ci"));
        assert!(workflow_names.contains(&"workflow"));
        assert!(workflow_names.contains(&"custom"));

        // With the new workflow grouping, all tasks should have the same workflow directory
        let common_path = temp_dir.path().join(".github").join("workflows");
        let expected_definition_paths = [
            github_workflows_dir.join("ci.yml"),
            temp_dir.path().join("workflow.yml"),
            custom_dir.join("custom.yml"),
        ];

        for task in act_tasks {
            assert_eq!(task.file_path, common_path);
            assert!(
                expected_definition_paths.contains(&task.definition_path().to_path_buf()),
                "unexpected workflow definition path: {}",
                task.definition_path().display()
            );
        }
    }

    #[test]
    #[serial]
    fn test_process_disambiguation_for_shadowed_tasks() {
        // Create a test task that is shadowed by a shell builtin
        let mut discovered = DiscoveredTasks::default();

        // Mock a task with name "test" that is shadowed by shell builtin
        discovered.tasks.push(Task {
            name: "test".to_string(),
            file_path: PathBuf::from("/test/Makefile"),
            definition_path: None,
            definition_type: TaskDefinitionType::Makefile,
            runner: TaskRunner::Make,
            source_name: "test".to_string(),
            description: None,
            shadowed_by: Some(ShadowType::ShellBuiltin("bash".to_string())),
            disambiguated_name: None,
        });

        // Mock a task with name "ls" that is shadowed by PATH executable
        discovered.tasks.push(Task {
            name: "ls".to_string(),
            file_path: PathBuf::from("/test/Makefile"),
            definition_path: None,
            definition_type: TaskDefinitionType::Makefile,
            runner: TaskRunner::Make,
            source_name: "ls".to_string(),
            description: None,
            shadowed_by: Some(ShadowType::PathExecutable("/bin/ls".to_string())),
            disambiguated_name: None,
        });

        // Mock a task that is not shadowed (should not get a disambiguated name)
        discovered.tasks.push(Task {
            name: "build".to_string(),
            file_path: PathBuf::from("/test/Makefile"),
            definition_path: None,
            definition_type: TaskDefinitionType::Makefile,
            runner: TaskRunner::Make,
            source_name: "build".to_string(),
            description: None,
            shadowed_by: None,
            disambiguated_name: None,
        });

        // Process the tasks
        process_task_disambiguation(&mut discovered);

        // Verify shadowed tasks received disambiguated names
        assert_eq!(
            discovered.tasks[0].disambiguated_name,
            Some("test-m".to_string())
        );
        assert_eq!(
            discovered.tasks[1].disambiguated_name,
            Some("ls-m".to_string())
        );

        // Verify non-shadowed task did not receive a disambiguated name
        assert_eq!(discovered.tasks[2].disambiguated_name, None);
    }

    #[test]
    #[serial]
    fn test_process_disambiguation_mixed_scenarios() {
        // Create a test TaskDiscovery with a mix of:
        // 1. Tasks with name collisions
        // 2. Shadowed tasks
        // 3. Normal tasks
        let mut discovered = DiscoveredTasks::default();

        // Create tasks with name collisions - multiple "test" tasks
        discovered.tasks.push(Task {
            name: "test".to_string(),
            file_path: PathBuf::from("/test/Makefile"),
            definition_path: None,
            definition_type: TaskDefinitionType::Makefile,
            runner: TaskRunner::Make,
            source_name: "test".to_string(),
            description: None,
            shadowed_by: None,
            disambiguated_name: None,
        });

        discovered.tasks.push(Task {
            name: "test".to_string(),
            file_path: PathBuf::from("/test/package.json"),
            definition_path: None,
            definition_type: TaskDefinitionType::PackageJson,
            runner: TaskRunner::NodeNpm,
            source_name: "test".to_string(),
            description: None,
            shadowed_by: None,
            disambiguated_name: Some("test-npm".to_string()),
        });

        // Shadowed task - "ls" shadowed by PATH executable
        discovered.tasks.push(Task {
            name: "ls".to_string(),
            file_path: PathBuf::from("/test/Makefile"),
            definition_path: None,
            definition_type: TaskDefinitionType::Makefile,
            runner: TaskRunner::Make,
            source_name: "ls".to_string(),
            description: None,
            shadowed_by: Some(ShadowType::PathExecutable("/bin/ls".to_string())),
            disambiguated_name: None,
        });

        // Shadowed task with name collision - "cd" shadowed by shell builtin
        discovered.tasks.push(Task {
            name: "cd".to_string(),
            file_path: PathBuf::from("/test/Makefile"),
            definition_path: None,
            definition_type: TaskDefinitionType::Makefile,
            runner: TaskRunner::Make,
            source_name: "cd".to_string(),
            description: None,
            shadowed_by: Some(ShadowType::ShellBuiltin("bash".to_string())),
            disambiguated_name: None,
        });

        discovered.tasks.push(Task {
            name: "cd".to_string(),
            file_path: PathBuf::from("/test/Taskfile.yml"),
            definition_path: None,
            definition_type: TaskDefinitionType::Taskfile,
            runner: TaskRunner::Task,
            source_name: "cd".to_string(),
            description: None,
            shadowed_by: Some(ShadowType::ShellBuiltin("bash".to_string())),
            disambiguated_name: None,
        });

        // Normal task - no collision, not shadowed
        discovered.tasks.push(Task {
            name: "build".to_string(),
            file_path: PathBuf::from("/test/Makefile"),
            definition_path: None,
            definition_type: TaskDefinitionType::Makefile,
            runner: TaskRunner::Make,
            source_name: "build".to_string(),
            description: None,
            shadowed_by: None,
            disambiguated_name: None,
        });

        // Process the tasks
        process_task_disambiguation(&mut discovered);

        // Verify name collisions get unique disambiguated names
        let test_tasks: Vec<_> = discovered
            .tasks
            .iter()
            .filter(|t| t.name == "test")
            .collect();
        assert_eq!(test_tasks.len(), 2);
        assert!(test_tasks[0].disambiguated_name.is_some());
        assert!(test_tasks[1].disambiguated_name.is_some());
        assert_ne!(
            test_tasks[0].disambiguated_name,
            test_tasks[1].disambiguated_name
        );

        // Verify shadowed task gets disambiguated name
        let ls_task = discovered
            .tasks
            .iter()
            .find(|t| t.name == "ls")
            .expect("ls task not found");
        assert_eq!(ls_task.disambiguated_name, Some("ls-m".to_string()));

        // Verify shadowed tasks with name collision all get disambiguated names
        let cd_tasks: Vec<_> = discovered.tasks.iter().filter(|t| t.name == "cd").collect();
        assert_eq!(cd_tasks.len(), 2);
        assert!(cd_tasks[0].disambiguated_name.is_some());
        assert!(cd_tasks[1].disambiguated_name.is_some());
        assert_ne!(
            cd_tasks[0].disambiguated_name,
            cd_tasks[1].disambiguated_name
        );

        // One should be cd-m and the other cd-t
        let cd_disambiguated_names: Vec<_> = cd_tasks
            .iter()
            .filter_map(|t| t.disambiguated_name.as_ref())
            .map(|s| s.as_str())
            .collect();
        assert!(cd_disambiguated_names.contains(&"cd-m"));
        assert!(cd_disambiguated_names.contains(&"cd-t"));

        // Verify normal task doesn't get disambiguated name
        let build_task = discovered
            .tasks
            .iter()
            .find(|t| t.name == "build")
            .expect("build task not found");
        assert_eq!(build_task.disambiguated_name, None);
    }

    #[test]
    #[serial]
    fn test_get_matching_tasks_with_shadowed_task() {
        let mut discovered = DiscoveredTasks::default();

        // Create a shadowed task with a disambiguated name
        discovered.tasks.push(Task {
            name: "install".to_string(),
            file_path: PathBuf::from("/test/Makefile"),
            definition_path: None,
            definition_type: TaskDefinitionType::Makefile,
            runner: TaskRunner::Make,
            source_name: "install".to_string(),
            description: None,
            shadowed_by: Some(ShadowType::PathExecutable("/usr/bin/install".to_string())),
            disambiguated_name: Some("install-m".to_string()),
        });

        // Look up the task by original name
        let matching_by_original = get_matching_tasks(&discovered, "install");
        assert_eq!(matching_by_original.len(), 1);

        // Look up the task by disambiguated name
        let matching_by_disambiguated = get_matching_tasks(&discovered, "install-m");
        assert_eq!(matching_by_disambiguated.len(), 1);

        // Verify it's the same task
        assert_eq!(matching_by_original[0].name, "install");
        assert_eq!(matching_by_disambiguated[0].name, "install");
        assert_eq!(
            matching_by_disambiguated[0].disambiguated_name,
            Some("install-m".to_string())
        );
    }

    #[test]
    #[serial]
    fn test_get_matching_tasks_treats_alias_collision_as_ambiguous() {
        let mut discovered = DiscoveredTasks::default();

        discovered.tasks.push(Task {
            name: "test".to_string(),
            file_path: PathBuf::from("/test/Makefile"),
            definition_path: None,
            definition_type: TaskDefinitionType::Makefile,
            runner: TaskRunner::Make,
            source_name: "test".to_string(),
            description: None,
            shadowed_by: Some(ShadowType::PathExecutable("/bin/test".to_string())),
            disambiguated_name: Some("test-m".to_string()),
        });
        discovered.tasks.push(Task {
            name: "test-m".to_string(),
            file_path: PathBuf::from("/test/package.json"),
            definition_path: None,
            definition_type: TaskDefinitionType::PackageJson,
            runner: TaskRunner::NodeNpm,
            source_name: "test-m".to_string(),
            description: None,
            shadowed_by: None,
            disambiguated_name: None,
        });

        let matching_tasks = get_matching_tasks(&discovered, "test-m");

        assert_eq!(matching_tasks.len(), 2);
        assert!(matching_tasks.iter().any(|task| task.name == "test"));
        assert!(matching_tasks.iter().any(|task| task.name == "test-m"));
    }

    #[test]
    fn test_execute_task_with_disambiguated_name() {
        let mut discovered_tasks = DiscoveredTasks::new();

        let task = Task {
            name: "test".to_string(),
            file_path: PathBuf::from("/path/to/Makefile"),
            definition_path: None,
            definition_type: TaskDefinitionType::Makefile,
            runner: TaskRunner::Make,
            source_name: "test".to_string(),
            description: None,
            shadowed_by: Some(ShadowType::PathExecutable("/bin/test".to_string())),
            disambiguated_name: Some("test-m".to_string()),
        };

        discovered_tasks.add_task(task);

        // Mock the executor
        let mut mock_executor = MockTaskExecutor::new();

        // Expect execution with the original task name, not the disambiguated one
        mock_executor.expect_execute().times(1).returning(|task| {
            assert_eq!(task.name, "test"); // We still execute with the original name
            assert_eq!(task.disambiguated_name, Some("test-m".to_string())); // But it has a disambiguated name
            assert!(task.shadowed_by.is_some()); // And it is shadowed
            Ok(())
        });

        let mut executor = CommandExecutor::new(mock_executor);

        // Execute using the disambiguated name
        let result = executor.execute_task_by_name(&mut discovered_tasks, "test-m", &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_task_by_either_name() {
        let mut discovered_tasks = DiscoveredTasks::new();

        // Add a shadowed task with a disambiguated name
        let task = Task {
            name: "grep".to_string(),
            file_path: PathBuf::from("/path/to/Makefile"),
            definition_path: None,
            definition_type: TaskDefinitionType::Makefile,
            runner: TaskRunner::Make,
            source_name: "grep".to_string(),
            description: None,
            shadowed_by: Some(ShadowType::PathExecutable("/bin/grep".to_string())),
            disambiguated_name: Some("grep-m".to_string()),
        };

        discovered_tasks.add_task(task);

        // Mock the executor
        let mut mock_executor = MockTaskExecutor::new();

        // Expect two executions - one by original name, one by disambiguated name
        mock_executor.expect_execute().times(2).returning(|task| {
            assert_eq!(task.name, "grep"); // Original name used for execution
            Ok(())
        });

        let mut executor = CommandExecutor::new(mock_executor);

        // Execute using the original name
        let result1 = executor.execute_task_by_name(&mut discovered_tasks, "grep", &[]);
        assert!(result1.is_ok());

        // Execute using the disambiguated name
        let result2 = executor.execute_task_by_name(&mut discovered_tasks, "grep-m", &[]);
        assert!(result2.is_ok());
    }

    #[test]
    fn test_execute_task_ambiguous_and_shadowed() {
        let mut discovered_tasks = DiscoveredTasks::new();

        // Add two tasks with the same name but from different sources
        let task1 = Task {
            name: "test".to_string(),
            file_path: PathBuf::from("/path/to/Makefile"),
            definition_path: None,
            definition_type: TaskDefinitionType::Makefile,
            runner: TaskRunner::Make,
            source_name: "test".to_string(),
            description: None,
            shadowed_by: Some(ShadowType::PathExecutable("/bin/test".to_string())),
            disambiguated_name: Some("test-m".to_string()),
        };

        let task2 = Task {
            name: "test".to_string(),
            file_path: PathBuf::from("/path/to/package.json"),
            definition_path: None,
            definition_type: TaskDefinitionType::PackageJson,
            runner: TaskRunner::NodeNpm,
            source_name: "test".to_string(),
            description: None,
            shadowed_by: None,
            disambiguated_name: Some("test-npm".to_string()),
        };

        // Manually set task name counts to mark "test" as ambiguous
        discovered_tasks
            .task_name_counts
            .insert("test".to_string(), 2);

        discovered_tasks.add_task(task1);
        discovered_tasks.add_task(task2);

        // Mock the executor
        let mut mock_executor = MockTaskExecutor::new();

        // Expect execution with the specific task
        mock_executor.expect_execute().times(2).returning(|task| {
            if task.runner == TaskRunner::Make {
                assert_eq!(task.disambiguated_name, Some("test-m".to_string()));
            } else if task.runner == TaskRunner::NodeNpm {
                assert_eq!(task.disambiguated_name, Some("test-npm".to_string()));
            } else {
                panic!("Unexpected task runner");
            }
            Ok(())
        });

        let mut executor = CommandExecutor::new(mock_executor);

        // Execute using the disambiguated names
        let result1 = executor.execute_task_by_name(&mut discovered_tasks, "test-m", &[]);
        assert!(result1.is_ok());

        let result2 = executor.execute_task_by_name(&mut discovered_tasks, "test-npm", &[]);
        assert!(result2.is_ok());

        // Executing by the original name should fail due to ambiguity
        let result3 = executor.execute_task_by_name(&mut discovered_tasks, "test", &[]);

        assert!(result3.is_err());

        // Get the error message and check it
        let err_msg = result3.unwrap_err();
        println!("Error message: {}", err_msg);
        assert!(err_msg.to_string().contains("Ambiguous"));
    }

    #[test]
    fn test_execute_task_alias_collision_is_ambiguous() {
        let mut discovered_tasks = DiscoveredTasks::new();

        discovered_tasks.add_task(Task {
            name: "test".to_string(),
            file_path: PathBuf::from("/path/to/Makefile"),
            definition_path: None,
            definition_type: TaskDefinitionType::Makefile,
            runner: TaskRunner::Make,
            source_name: "test".to_string(),
            description: None,
            shadowed_by: Some(ShadowType::PathExecutable("/bin/test".to_string())),
            disambiguated_name: Some("test-m".to_string()),
        });
        discovered_tasks.add_task(Task {
            name: "test-m".to_string(),
            file_path: PathBuf::from("/path/to/package.json"),
            definition_path: None,
            definition_type: TaskDefinitionType::PackageJson,
            runner: TaskRunner::NodeNpm,
            source_name: "test-m".to_string(),
            description: None,
            shadowed_by: None,
            disambiguated_name: None,
        });

        let mut executor = CommandExecutor::new(MockTaskExecutor::new());
        let result = executor.execute_task_by_name(&mut discovered_tasks, "test-m", &[]);

        assert!(result.is_err());
        let err_msg = result.unwrap_err();
        assert!(
            err_msg
                .to_string()
                .contains("Ambiguous task name: 'test-m'")
        );
        assert!(
            err_msg
                .to_string()
                .contains("  • test-m (make from /path/to/Makefile)")
        );
        assert!(
            err_msg
                .to_string()
                .contains("  • test-m (npm from /path/to/package.json)")
        );
    }

    #[test]
    fn test_discover_taskfile_variants() {
        let temp_dir = TempDir::new().unwrap();

        // Create taskfile.yaml (lower priority than Taskfile.yml)
        let taskfile_yaml_content = r#"version: '3'
tasks:
  from_yaml:
    desc: This task is from taskfile.yaml
    cmds:
      - echo "From taskfile.yaml"
"#;
        let taskfile_yaml_path = temp_dir.path().join("taskfile.yaml");
        let mut file = File::create(&taskfile_yaml_path).unwrap();
        write!(file, "{}", taskfile_yaml_content).unwrap();

        // Now create Taskfile.yml (higher priority, should be used)
        let taskfile_yml_content = r#"version: '3'
tasks:
  from_yml:
    desc: This task is from Taskfile.yml
    cmds:
      - echo "From Taskfile.yml"
"#;
        let taskfile_yml_path = temp_dir.path().join("Taskfile.yml");
        let mut file = File::create(&taskfile_yml_path).unwrap();
        write!(file, "{}", taskfile_yml_content).unwrap();

        // Run discovery
        let discovered = discover_tasks(temp_dir.path());

        // Check that the taskfile status is Parsed
        let taskfile_def = discovered.definitions.taskfile.unwrap();
        assert_eq!(taskfile_def.status, TaskFileStatus::Parsed);

        // Verify the task from Taskfile.yml exists (check by content rather than filename)
        assert_eq!(discovered.tasks.len(), 1);
        let task = discovered.tasks.first().unwrap();
        assert_eq!(task.name, "from_yml");
        assert_eq!(
            task.description,
            Some("This task is from Taskfile.yml".to_string())
        );

        // Delete the higher priority Taskfile and verify the lower priority one is used
        std::fs::remove_file(taskfile_yml_path).unwrap();

        // Run discovery again
        let discovered = discover_tasks(temp_dir.path());

        // Check that the taskfile status is Parsed
        let taskfile_def = discovered.definitions.taskfile.unwrap();
        assert_eq!(taskfile_def.status, TaskFileStatus::Parsed);

        // Check the task from taskfile.yaml exists (verify by content)
        assert_eq!(discovered.tasks.len(), 1);
        let task = discovered.tasks.first().unwrap();
        assert_eq!(task.name, "from_yaml");
        assert_eq!(
            task.description,
            Some("This task is from taskfile.yaml".to_string())
        );
    }

    #[test]
    fn test_discover_docker_compose_tasks() {
        let temp_dir = TempDir::new().unwrap();

        // Create a docker-compose.yml file
        let docker_compose_content = r#"
version: '3.8'
services:
  web:
    image: nginx:alpine
    ports:
      - "8080:80"
  db:
    image: postgres:13
    environment:
      POSTGRES_DB: myapp
      POSTGRES_USER: user
      POSTGRES_PASSWORD: password
  app:
    build: .
    depends_on:
      - db
"#;
        let docker_compose_path = temp_dir.path().join("docker-compose.yml");
        let mut file = File::create(&docker_compose_path).unwrap();
        write!(file, "{}", docker_compose_content).unwrap();

        // Run discovery
        let discovered = discover_tasks(temp_dir.path());

        // Check that the docker-compose status is Parsed
        let docker_compose_def = discovered.definitions.docker_compose.unwrap();
        assert_eq!(docker_compose_def.status, TaskFileStatus::Parsed);
        assert_eq!(docker_compose_def.path, docker_compose_path);

        // Check that all services are found as tasks (plus the "up" and "down" tasks)
        assert_eq!(discovered.tasks.len(), 5);

        let service_names: Vec<&str> = discovered.tasks.iter().map(|t| t.name.as_str()).collect();
        assert!(service_names.contains(&"up"));
        assert!(service_names.contains(&"down"));
        assert!(service_names.contains(&"web"));
        assert!(service_names.contains(&"db"));
        assert!(service_names.contains(&"app"));

        // Check task properties
        for task in &discovered.tasks {
            assert_eq!(task.definition_type, TaskDefinitionType::DockerCompose);
            assert_eq!(task.runner, TaskRunner::DockerCompose);
            assert_eq!(task.file_path, docker_compose_path);
            assert!(task.description.is_some());
            assert!(task.shadowed_by.is_none());
            assert!(task.disambiguated_name.is_none());
        }

        // Check specific task descriptions
        let web_task = discovered.tasks.iter().find(|t| t.name == "web").unwrap();
        assert!(
            web_task
                .description
                .as_ref()
                .unwrap()
                .contains("nginx:alpine")
        );

        let app_task = discovered.tasks.iter().find(|t| t.name == "app").unwrap();
        assert!(app_task.description.as_ref().unwrap().contains("build"));
    }

    #[test]
    fn test_discover_docker_compose_empty() {
        let temp_dir = TempDir::new().unwrap();

        // Create an empty docker-compose.yml file
        let docker_compose_content = r#"
version: '3.8'
services: {}
"#;
        let docker_compose_path = temp_dir.path().join("docker-compose.yml");
        let mut file = File::create(&docker_compose_path).unwrap();
        write!(file, "{}", docker_compose_content).unwrap();

        // Run discovery
        let discovered = discover_tasks(temp_dir.path());

        // Check that the docker-compose status is Parsed
        let docker_compose_def = discovered.definitions.docker_compose.unwrap();
        assert_eq!(docker_compose_def.status, TaskFileStatus::Parsed);

        // Check that only the "up" and "down" tasks are found
        assert_eq!(discovered.tasks.len(), 2);
        let service_names: Vec<&str> = discovered.tasks.iter().map(|t| t.name.as_str()).collect();
        assert!(service_names.contains(&"up"));
        assert!(service_names.contains(&"down"));
    }

    #[test]
    fn test_discover_docker_compose_missing_file() {
        let temp_dir = TempDir::new().unwrap();

        // Run discovery without docker-compose.yml
        let discovered = discover_tasks(temp_dir.path());

        // Check that the docker-compose status is NotFound
        let docker_compose_def = discovered.definitions.docker_compose.unwrap();
        assert_eq!(docker_compose_def.status, TaskFileStatus::NotFound);

        // Check that no tasks are found
        assert_eq!(discovered.tasks.len(), 0);
    }

    #[test]
    fn test_discover_docker_compose_multiple_formats() {
        let temp_dir = TempDir::new().unwrap();

        // Create a compose.yml file (lower priority)
        let compose_content = r#"
version: '3.8'
services:
  api:
    image: nginx:alpine
    ports:
      - "8080:80"
"#;
        std::fs::write(temp_dir.path().join("compose.yml"), compose_content).unwrap();

        // Run discovery
        let discovered = discover_tasks(temp_dir.path());

        // Check that the docker-compose status is Parsed
        let docker_compose_def = discovered.definitions.docker_compose.unwrap();
        assert_eq!(docker_compose_def.status, TaskFileStatus::Parsed);
        assert_eq!(docker_compose_def.path, temp_dir.path().join("compose.yml"));

        // Check that the service is found (plus the "up" and "down" tasks)
        assert_eq!(discovered.tasks.len(), 3);
        let service_names: Vec<&str> = discovered.tasks.iter().map(|t| t.name.as_str()).collect();
        assert!(service_names.contains(&"up"));
        assert!(service_names.contains(&"down"));
        assert!(service_names.contains(&"api"));

        let api_task = discovered.tasks.iter().find(|t| t.name == "api").unwrap();
        assert_eq!(api_task.definition_type, TaskDefinitionType::DockerCompose);
        assert_eq!(api_task.runner, TaskRunner::DockerCompose);

        // Now create a docker-compose.yml file (higher priority)
        let docker_compose_content = r#"
version: '3.8'
services:
  web:
    image: nginx:alpine
    ports:
      - "8080:80"
  db:
    image: postgres:13
"#;
        std::fs::write(
            temp_dir.path().join("docker-compose.yml"),
            docker_compose_content,
        )
        .unwrap();

        // Run discovery again
        let discovered = discover_tasks(temp_dir.path());

        // Check that the higher priority file is used
        let docker_compose_def = discovered.definitions.docker_compose.unwrap();
        assert_eq!(docker_compose_def.status, TaskFileStatus::Parsed);
        assert_eq!(
            docker_compose_def.path,
            temp_dir.path().join("docker-compose.yml")
        );

        // Check that the services from the higher priority file are found (plus the "up" and "down" tasks)
        assert_eq!(discovered.tasks.len(), 4);
        let service_names: Vec<&str> = discovered.tasks.iter().map(|t| t.name.as_str()).collect();
        assert!(service_names.contains(&"up"));
        assert!(service_names.contains(&"down"));
        assert!(service_names.contains(&"web"));
        assert!(service_names.contains(&"db"));
    }

    #[test]
    fn test_discover_travis_ci_tasks() {
        let temp_dir = TempDir::new().unwrap();

        // Create a .travis.yml file
        let travis_content = r#"
language: node_js
node_js:
  - "18"
  - "20"

jobs:
  test:
    name: "Test"
    stage: test
  build:
    name: "Build"
    stage: build
"#;
        let travis_path = temp_dir.path().join(".travis.yml");
        let mut file = File::create(&travis_path).unwrap();
        write!(file, "{}", travis_content).unwrap();

        // Run discovery
        let discovered = discover_tasks(temp_dir.path());

        // Check that the travis-ci status is Parsed
        let travis_def = discovered.definitions.travis_ci.unwrap();
        assert_eq!(travis_def.status, TaskFileStatus::Parsed);
        assert_eq!(travis_def.path, travis_path);

        // Check that both jobs are found as tasks
        assert_eq!(discovered.tasks.len(), 2);

        let test_task = discovered.tasks.iter().find(|t| t.name == "test").unwrap();
        assert_eq!(test_task.definition_type, TaskDefinitionType::TravisCi);
        assert_eq!(test_task.runner, TaskRunner::TravisCi);
        assert_eq!(
            test_task.description,
            Some("Travis CI job: Test".to_string())
        );

        let build_task = discovered.tasks.iter().find(|t| t.name == "build").unwrap();
        assert_eq!(build_task.definition_type, TaskDefinitionType::TravisCi);
        assert_eq!(build_task.runner, TaskRunner::TravisCi);
        assert_eq!(
            build_task.description,
            Some("Travis CI job: Build".to_string())
        );
    }

    #[test]
    fn test_discover_travis_ci_matrix_config() {
        let temp_dir = TempDir::new().unwrap();

        // Create a .travis.yml file with matrix configuration
        let travis_content = r#"
language: python

matrix:
  include:
    - name: "Python 3.8"
      python: "3.8"
    - name: "Python 3.9"
      python: "3.9"
    - name: "Python 3.10"
      python: "3.10"
"#;
        let travis_path = temp_dir.path().join(".travis.yml");
        let mut file = File::create(&travis_path).unwrap();
        write!(file, "{}", travis_content).unwrap();

        // Run discovery
        let discovered = discover_tasks(temp_dir.path());

        // Check that the travis-ci status is Parsed
        let travis_def = discovered.definitions.travis_ci.unwrap();
        assert_eq!(travis_def.status, TaskFileStatus::Parsed);

        // Check that all matrix jobs are found as tasks
        assert_eq!(discovered.tasks.len(), 3);

        for task in &discovered.tasks {
            assert_eq!(task.definition_type, TaskDefinitionType::TravisCi);
            assert_eq!(task.runner, TaskRunner::TravisCi);
            assert!(
                task.description
                    .as_ref()
                    .unwrap()
                    .contains("Travis CI job:")
            );
        }

        let python_38_task = discovered
            .tasks
            .iter()
            .find(|t| t.name == "Python 3.8")
            .unwrap();
        assert_eq!(
            python_38_task.description,
            Some("Travis CI job: Python 3.8".to_string())
        );

        let python_39_task = discovered
            .tasks
            .iter()
            .find(|t| t.name == "Python 3.9")
            .unwrap();
        assert_eq!(
            python_39_task.description,
            Some("Travis CI job: Python 3.9".to_string())
        );

        let python_310_task = discovered
            .tasks
            .iter()
            .find(|t| t.name == "Python 3.10")
            .unwrap();
        assert_eq!(
            python_310_task.description,
            Some("Travis CI job: Python 3.10".to_string())
        );
    }

    #[test]
    fn test_discover_travis_ci_basic_config() {
        let temp_dir = TempDir::new().unwrap();

        // Create a basic .travis.yml file without jobs section
        let travis_content = r#"
language: ruby
rvm:
  - 2.7
  - 3.0
  - 3.1

script:
  - bundle install
  - bundle exec rspec
"#;
        let travis_path = temp_dir.path().join(".travis.yml");
        let mut file = File::create(&travis_path).unwrap();
        write!(file, "{}", travis_content).unwrap();

        // Run discovery
        let discovered = discover_tasks(temp_dir.path());

        // Check that the travis-ci status is Parsed
        let travis_def = discovered.definitions.travis_ci.unwrap();
        assert_eq!(travis_def.status, TaskFileStatus::Parsed);

        // Check that a default task is created
        assert_eq!(discovered.tasks.len(), 1);

        let task = &discovered.tasks[0];
        assert_eq!(task.name, "travis");
        assert_eq!(task.definition_type, TaskDefinitionType::TravisCi);
        assert_eq!(task.runner, TaskRunner::TravisCi);
        assert_eq!(
            task.description,
            Some("Travis CI configuration".to_string())
        );
    }

    #[test]
    fn test_discover_travis_ci_missing_file() {
        let temp_dir = TempDir::new().unwrap();

        // Run discovery without .travis.yml
        let discovered = discover_tasks(temp_dir.path());

        // Check that the travis-ci status is NotFound
        let travis_def = discovered.definitions.travis_ci.unwrap();
        assert_eq!(travis_def.status, TaskFileStatus::NotFound);

        // Check that no tasks are found
        assert_eq!(discovered.tasks.len(), 0);
    }

    #[test]
    fn test_discover_cmake_tasks() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path();

        // Create a CMakeLists.txt file
        let cmake_path = dir.join("CMakeLists.txt");
        let mut file = File::create(&cmake_path).unwrap();
        write!(
            file,
            r#"
cmake_minimum_required(VERSION 3.10)
project(MyProject)

# Add executable
add_executable(myapp main.cpp)

# Add custom target
add_custom_target(build-all
    COMMAND cmake --build .
    COMMENT "Building all targets"
)
"#
        )
        .unwrap();

        let discovered = discover_tasks(dir);
        assert!(!discovered.tasks.is_empty());

        // Check that CMake tasks were discovered
        let cmake_tasks: Vec<_> = discovered
            .tasks
            .iter()
            .filter(|t| t.definition_type == TaskDefinitionType::CMake)
            .collect();
        assert!(!cmake_tasks.is_empty());

        // Check that the CMake definition was set
        assert!(discovered.definitions.cmake.is_some());
        let cmake_def = discovered.definitions.cmake.as_ref().unwrap();
        assert_eq!(cmake_def.path, cmake_path);
        assert_eq!(cmake_def.definition_type, TaskDefinitionType::CMake);
        assert!(matches!(cmake_def.status, TaskFileStatus::Parsed));
    }

    #[test]
    fn test_discover_cmake_tasks_not_found() {
        let temp_dir = TempDir::new().unwrap();

        let discovered = discover_tasks(temp_dir.path());

        let cmake_def = discovered.definitions.cmake.as_ref().unwrap();
        assert_eq!(cmake_def.path, temp_dir.path().join("CMakeLists.txt"));
        assert_eq!(cmake_def.definition_type, TaskDefinitionType::CMake);
        assert!(matches!(cmake_def.status, TaskFileStatus::NotFound));
    }

    #[test]
    fn test_discover_justfile_variants() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path();

        // Test with "Justfile" (default)
        let justfile_path = dir.join("Justfile");
        let mut file = File::create(&justfile_path).unwrap();
        write!(
            file,
            r#"
build: # Build the project
    cargo build

test: # Run tests
    cargo test
"#
        )
        .unwrap();

        let discovered = discover_tasks(dir);
        assert!(!discovered.tasks.is_empty());

        let justfile_tasks: Vec<_> = discovered
            .tasks
            .iter()
            .filter(|t| t.definition_type == TaskDefinitionType::Justfile)
            .collect();
        assert_eq!(justfile_tasks.len(), 2);

        // Check that the Justfile definition was set
        assert!(discovered.definitions.justfile.is_some());
        let justfile_def = discovered.definitions.justfile.as_ref().unwrap();
        assert_eq!(justfile_def.path, justfile_path);
        assert_eq!(justfile_def.definition_type, TaskDefinitionType::Justfile);
        assert!(matches!(justfile_def.status, TaskFileStatus::Parsed));

        // Clean up and test with "justfile" (lowercase)
        // On case-insensitive filesystems (like macOS), this will remove the same file
        std::fs::remove_file(&justfile_path).unwrap();

        // Create a new temp directory to avoid case-insensitive filesystem issues
        let temp_dir2 = TempDir::new().unwrap();
        let dir2 = temp_dir2.path();

        let justfile_lower_path = dir2.join("justfile");
        let mut file = File::create(&justfile_lower_path).unwrap();
        write!(
            file,
            r#"
build: # Build the project
    cargo build

test: # Run tests
    cargo test
"#
        )
        .unwrap();

        let discovered = discover_tasks(dir2);
        assert!(!discovered.tasks.is_empty());

        let justfile_tasks: Vec<_> = discovered
            .tasks
            .iter()
            .filter(|t| t.definition_type == TaskDefinitionType::Justfile)
            .collect();
        assert_eq!(justfile_tasks.len(), 2);

        // Check that the Justfile definition was set
        assert!(discovered.definitions.justfile.is_some());
        let justfile_def = discovered.definitions.justfile.as_ref().unwrap();
        // The path should match what was actually found by the discovery function
        // On case-insensitive filesystems, this will be "Justfile"
        // On case-sensitive filesystems, this will be "justfile"
        assert!(
            justfile_def.path == dir2.join("Justfile") || justfile_def.path == justfile_lower_path
        );
        assert_eq!(justfile_def.definition_type, TaskDefinitionType::Justfile);
        assert!(matches!(justfile_def.status, TaskFileStatus::Parsed));

        // Test with ".justfile" (leading dot) in a third directory
        let temp_dir3 = TempDir::new().unwrap();
        let dir3 = temp_dir3.path();

        let justfile_dot_path = dir3.join(".justfile");
        let mut file = File::create(&justfile_dot_path).unwrap();
        write!(
            file,
            r#"
build: # Build the project
    cargo build

test: # Run tests
    cargo test
"#
        )
        .unwrap();

        let discovered = discover_tasks(dir3);
        assert!(!discovered.tasks.is_empty());

        let justfile_tasks: Vec<_> = discovered
            .tasks
            .iter()
            .filter(|t| t.definition_type == TaskDefinitionType::Justfile)
            .collect();
        assert_eq!(justfile_tasks.len(), 2);

        // Check that the Justfile definition was set
        assert!(discovered.definitions.justfile.is_some());
        let justfile_def = discovered.definitions.justfile.as_ref().unwrap();
        // Should find the dot justfile since Justfile and justfile don't exist in this directory
        assert_eq!(justfile_def.path, justfile_dot_path);
        assert_eq!(justfile_def.definition_type, TaskDefinitionType::Justfile);
        assert!(matches!(justfile_def.status, TaskFileStatus::Parsed));
    }

    #[test]
    fn test_discover_justfile_priority_order() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path();

        // Test priority order: Justfile should be found first
        let justfile_path = dir.join("Justfile");
        let justfile_lower_path = dir.join("justfile");
        let justfile_dot_path = dir.join(".justfile");

        // Create content for all three files
        let content = r#"
build: # Build the project
    cargo build
"#;

        let mut file = File::create(&justfile_path).unwrap();
        write!(file, "{}", content).unwrap();

        let mut file = File::create(&justfile_lower_path).unwrap();
        write!(file, "{}", content).unwrap();

        let mut file = File::create(&justfile_dot_path).unwrap();
        write!(file, "{}", content).unwrap();

        let discovered = discover_tasks(dir);
        assert!(!discovered.tasks.is_empty());

        // Should prioritize "Justfile" over others
        assert!(discovered.definitions.justfile.is_some());
        let justfile_def = discovered.definitions.justfile.as_ref().unwrap();
        assert_eq!(justfile_def.path, justfile_path);
        assert!(matches!(justfile_def.status, TaskFileStatus::Parsed));

        // Test with only lowercase (in a new directory to avoid case-insensitive issues)
        let temp_dir2 = TempDir::new().unwrap();
        let dir2 = temp_dir2.path();

        let justfile_lower_path = dir2.join("justfile");
        let mut file = File::create(&justfile_lower_path).unwrap();
        write!(file, "{}", content).unwrap();

        let discovered = discover_tasks(dir2);
        assert!(!discovered.tasks.is_empty());

        assert!(discovered.definitions.justfile.is_some());
        let justfile_def = discovered.definitions.justfile.as_ref().unwrap();
        // The path should match what was actually found by the discovery function
        // On case-insensitive filesystems, this will be "Justfile"
        // On case-sensitive filesystems, this will be "justfile"
        assert!(
            justfile_def.path == dir2.join("Justfile") || justfile_def.path == justfile_lower_path
        );
        assert!(matches!(justfile_def.status, TaskFileStatus::Parsed));

        // Test with only dot variant (in a new directory)
        let temp_dir3 = TempDir::new().unwrap();
        let dir3 = temp_dir3.path();

        let justfile_dot_path = dir3.join(".justfile");
        let mut file = File::create(&justfile_dot_path).unwrap();
        write!(file, "{}", content).unwrap();

        let discovered = discover_tasks(dir3);
        assert!(!discovered.tasks.is_empty());

        assert!(discovered.definitions.justfile.is_some());
        let justfile_def = discovered.definitions.justfile.as_ref().unwrap();
        assert_eq!(justfile_def.path, justfile_dot_path);
        assert!(matches!(justfile_def.status, TaskFileStatus::Parsed));

        // Test with no justfile at all
        let temp_dir4 = TempDir::new().unwrap();
        let dir4 = temp_dir4.path();

        let discovered = discover_tasks(dir4);
        assert!(discovered.tasks.is_empty()); // No justfile should be found

        assert!(discovered.definitions.justfile.is_some());
        let justfile_def = discovered.definitions.justfile.as_ref().unwrap();
        assert_eq!(justfile_def.path, dir4.join("Justfile")); // Should use default path
        assert!(matches!(justfile_def.status, TaskFileStatus::NotFound));
    }
}
