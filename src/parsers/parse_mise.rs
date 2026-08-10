use crate::parsers::errors::DelaParseError;
use crate::types::{Task, TaskDefinitionType, TaskRunner};
use std::path::Path;

/// Parse tasks from a mise configuration file.
pub fn parse(path: &Path) -> Result<Vec<Task>, DelaParseError> {
    let contents = std::fs::read_to_string(path)?;
    let config: toml::Value = toml::from_str(&contents)?;

    let Some(tasks) = config.get("tasks") else {
        return Ok(Vec::new());
    };

    parse_task_table(tasks, path)
}

/// Parse a TOML file included through `task_config.includes`.
///
/// Included task files use the contents of a `[tasks]` table directly, without
/// the `tasks` prefix used by a regular mise configuration file.
pub fn parse_included_toml(path: &Path) -> Result<Vec<Task>, DelaParseError> {
    let contents = std::fs::read_to_string(path)?;
    let tasks: toml::Value = toml::from_str(&contents)?;
    parse_task_table(&tasks, path)
}

/// Return local task sources configured through `task_config.includes`.
pub fn task_includes(path: &Path) -> Result<Option<Vec<String>>, DelaParseError> {
    let contents = std::fs::read_to_string(path)?;
    let config: toml::Value = toml::from_str(&contents)?;
    let Some(includes) = config
        .get("task_config")
        .and_then(|task_config| task_config.get("includes"))
    else {
        return Ok(None);
    };

    let includes = includes.as_array().ok_or_else(|| {
        DelaParseError::Syntax(format!(
            "task_config.includes in '{}' must be an array",
            path.display()
        ))
    })?;

    includes
        .iter()
        .map(|include| {
            include.as_str().map(str::to_string).ok_or_else(|| {
                DelaParseError::Syntax(format!(
                    "task_config.includes in '{}' must contain only strings",
                    path.display()
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

/// Parse a standalone mise file task.
pub fn parse_file_task(task_directory: &Path, path: &Path) -> Result<Option<Task>, DelaParseError> {
    let Some(name) = file_task_name(task_directory, path) else {
        return Ok(None);
    };
    let contents = std::fs::read_to_string(path)?;
    let metadata = parse_file_task_metadata(&contents)?;

    if metadata
        .get("hide")
        .and_then(toml::Value::as_bool)
        .unwrap_or(false)
    {
        return Ok(None);
    }

    Ok(Some(Task {
        name: name.clone(),
        file_path: path.to_path_buf(),
        definition_path: None,
        definition_type: TaskDefinitionType::Mise,
        runner: TaskRunner::Mise,
        source_name: name,
        description: metadata
            .get("description")
            .and_then(toml::Value::as_str)
            .map(str::to_string),
        shadowed_by: None,
        disambiguated_name: None,
    }))
}

fn parse_task_table(value: &toml::Value, path: &Path) -> Result<Vec<Task>, DelaParseError> {
    let task_table = value.as_table().ok_or_else(|| {
        DelaParseError::Syntax(format!(
            "mise tasks in '{}' must be a TOML table",
            path.display()
        ))
    })?;

    let mut task_entries: Vec<_> = task_table.iter().collect();
    task_entries.sort_by_key(|(name, _)| *name);

    let mut tasks = Vec::new();
    for (name, definition) in task_entries {
        let (hidden, description) = match definition {
            toml::Value::String(_) => (false, None),
            toml::Value::Table(table) => (
                table
                    .get("hide")
                    .and_then(toml::Value::as_bool)
                    .unwrap_or(false),
                table
                    .get("description")
                    .and_then(toml::Value::as_str)
                    .map(str::to_string),
            ),
            _ => {
                return Err(DelaParseError::Syntax(format!(
                    "mise task '{}' in '{}' must be a string or table",
                    name,
                    path.display()
                )));
            }
        };

        if hidden {
            continue;
        }

        tasks.push(Task {
            name: name.clone(),
            file_path: path.to_path_buf(),
            definition_path: None,
            definition_type: TaskDefinitionType::Mise,
            runner: TaskRunner::Mise,
            source_name: name.clone(),
            description,
            shadowed_by: None,
            disambiguated_name: None,
        });
    }

    Ok(tasks)
}

fn file_task_name(task_directory: &Path, path: &Path) -> Option<String> {
    let relative_path = path.strip_prefix(task_directory).ok()?;
    let mut components: Vec<_> = relative_path
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect();

    if components.last().is_some_and(|name| name == "_default") {
        components.pop();
    }

    (!components.is_empty()).then(|| components.join(":"))
}

fn parse_file_task_metadata(contents: &str) -> Result<toml::Table, DelaParseError> {
    let directives = contents
        .lines()
        .filter_map(mise_directive)
        .collect::<Vec<_>>()
        .join("\n");

    if directives.is_empty() {
        return Ok(toml::Table::new());
    }

    let metadata: toml::Value = toml::from_str(&directives)?;
    metadata.as_table().cloned().ok_or_else(|| {
        DelaParseError::Syntax("mise file-task metadata must be a TOML table".to_string())
    })
}

fn mise_directive(line: &str) -> Option<&str> {
    let line = line.trim_start();
    ["#MISE", "# [MISE]", "//MISE", "// [MISE]"]
        .iter()
        .find_map(|prefix| line.strip_prefix(prefix).map(str::trim_start))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_mise_toml_tasks() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("mise.toml");
        std::fs::write(
            &path,
            r#"
[tasks]
build = "cargo build"

[tasks.test]
description = "Run automated tests"
run = ["cargo test", "./scripts/test-e2e.sh"]

[tasks.ci]
description = "Run continuous integration tasks"
depends = ["build", "test"]

[tasks.internal]
run = "echo hidden"
hide = true
"#,
        )
        .unwrap();

        let tasks = parse(&path).unwrap();

        assert_eq!(
            tasks
                .iter()
                .map(|task| task.name.as_str())
                .collect::<Vec<_>>(),
            vec!["build", "ci", "test"]
        );
        assert!(tasks.iter().all(|task| task.runner == TaskRunner::Mise));
        assert_eq!(
            tasks
                .iter()
                .find(|task| task.name == "test")
                .unwrap()
                .description,
            Some("Run automated tests".to_string())
        );
        assert!(!tasks.iter().any(|task| task.name == "internal"));
    }

    #[test]
    fn test_parse_mise_toml_without_tasks() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("mise.toml");
        std::fs::write(&path, "[tools]\nrust = 'latest'\n").unwrap();

        assert!(parse(&path).unwrap().is_empty());
    }

    #[test]
    fn test_parse_mise_toml_rejects_invalid_task_definition() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("mise.toml");
        std::fs::write(&path, "[tasks]\nbuild = 42\n").unwrap();

        let error = parse(&path).unwrap_err();

        assert!(error.to_string().contains("must be a string or table"));
    }

    #[test]
    fn test_parse_included_toml_tasks() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("tasks.toml");
        std::fs::write(
            &path,
            "build = 'cargo build'\n[test]\ndescription = 'Test it'\nrun = 'cargo test'\n",
        )
        .unwrap();

        let tasks = parse_included_toml(&path).unwrap();

        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[1].description, Some("Test it".to_string()));
    }

    #[test]
    fn test_parse_task_includes() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("mise.toml");
        std::fs::write(
            &path,
            "[task_config]\nincludes = ['tasks.toml', 'project-tasks']\n",
        )
        .unwrap();

        assert_eq!(
            task_includes(&path).unwrap(),
            Some(vec!["tasks.toml".to_string(), "project-tasks".to_string()])
        );
    }

    #[test]
    fn test_parse_grouped_file_tasks_and_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let task_directory = temp_dir.path().join("mise-tasks");
        let task_path = task_directory.join("test").join("integration");
        std::fs::create_dir_all(task_path.parent().unwrap()).unwrap();
        std::fs::write(
            &task_path,
            "#!/usr/bin/env bash\n#MISE description=\"Run integration tests\"\necho test\n",
        )
        .unwrap();

        let task = parse_file_task(&task_directory, &task_path)
            .unwrap()
            .unwrap();

        assert_eq!(task.name, "test:integration");
        assert_eq!(task.source_name, "test:integration");
        assert_eq!(task.description, Some("Run integration tests".to_string()));
        assert_eq!(task.file_path, task_path);
    }

    #[test]
    fn test_parse_default_file_task_name_and_multiline_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let task_directory = temp_dir.path().join(".mise").join("tasks");
        let task_path = task_directory.join("test").join("_default");
        std::fs::create_dir_all(task_path.parent().unwrap()).unwrap();
        std::fs::write(
            &task_path,
            "#!/bin/sh\n# [MISE] description='Run all tests'\n#MISE depends=[\n#MISE   'lint',\n#MISE   'units',\n#MISE ]\necho test\n",
        )
        .unwrap();

        let task = parse_file_task(&task_directory, &task_path)
            .unwrap()
            .unwrap();

        assert_eq!(task.name, "test");
        assert_eq!(task.description, Some("Run all tests".to_string()));
    }

    #[test]
    fn test_parse_hidden_file_task() {
        let temp_dir = TempDir::new().unwrap();
        let task_directory = temp_dir.path().join("mise-tasks");
        let task_path = task_directory.join("internal");
        std::fs::create_dir_all(&task_directory).unwrap();
        std::fs::write(&task_path, "#!/bin/sh\n#MISE hide=true\n").unwrap();

        assert!(
            parse_file_task(&task_directory, &task_path)
                .unwrap()
                .is_none()
        );
    }
}
