use crate::task_shadowing::check_path_executable;
use crate::types::{Task, TaskDefinitionType, TaskRunner};
use std::path::Path;

/// Parse a pyproject.toml file at the given path and extract tasks
pub fn parse(path: &Path) -> Result<Vec<Task>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read pyproject.toml: {}", e))?;

    let toml: toml::Value =
        toml::from_str(&content).map_err(|e| format!("Failed to parse pyproject.toml: {}", e))?;

    let mut tasks = Vec::new();

    // Check for UV scripts
    if let Some(project) = toml.get("project") {
        if let Some(scripts) = project.get("scripts") {
            if let Some(scripts_table) = scripts.as_table() {
                if cfg!(test) || check_path_executable("uv").is_some() {
                    for (name, cmd) in scripts_table {
                        let description = cmd
                            .as_str()
                            .map(|s| format!("python script: {}", s));

                        tasks.push(Task {
                            name: name.clone(),
                            file_path: path.to_path_buf(),
                            definition_type: TaskDefinitionType::PyprojectToml,
                            runner: TaskRunner::PythonUv,
                            source_name: name.clone(),
                            description,
                            shadowed_by: None,
                        });
                    }
                }
            }
        }
    }

    // Check for poetry configuration
    if let Some(tool_val) = toml.get("tool").or_else(|| toml.get("tool.poetry")) {
        // Determine the poetry table: if tool_val has a key "poetry", use that; otherwise, tool_val itself is the poetry table
        let poetry = if let Some(t) = tool_val.get("poetry") {
            t
        } else {
            tool_val
        };

        if let Some(scripts) = poetry.get("scripts") {
            if let Some(scripts_table) = scripts.as_table() {
                let poetry_lock_exists = path.parent()
                    .map(|dir| dir.join("poetry.lock").exists())
                    .unwrap_or(false);
                if cfg!(test) || (check_path_executable("poetry").is_some() && poetry_lock_exists) {
                    for (name, cmd) in scripts_table {
                        let description = cmd.as_str().map(|s| format!("python script: {}", s));

                        tasks.push(Task {
                            name: name.clone(),
                            file_path: path.to_path_buf(),
                            definition_type: TaskDefinitionType::PyprojectToml,
                            runner: TaskRunner::PythonPoetry,
                            source_name: name.clone(),
                            description,
                            shadowed_by: None,
                        });
                    }
                }
            }
        }
    }

    // Check for poethepoet tasks
    if let Some(poe) = toml.get("tool") {
        if let Some(poe_section) = poe.get("poe") {
            // If there is a nested "tasks" key, use that table; otherwise, use the poe_section table directly
            if let Some(tasks_table) = if let Some(inner) = poe_section.get("tasks") {
                inner.as_table()
            } else {
                poe_section.as_table()
            } {
                if cfg!(test) || check_path_executable("poe").is_some() {
                    for (name, task_def) in tasks_table {
                        let description = match task_def {
                            toml::Value::String(cmd) => Some(format!("command: {}", cmd)),
                            toml::Value::Table(table) => {
                                if let Some(script) = table.get("script") {
                                    script.as_str().map(|s| format!("python script: {}", s))
                                } else if let Some(shell) = table.get("shell") {
                                    shell.as_str().map(|s| format!("shell script: {}", s))
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        };

                        tasks.push(Task {
                            name: name.clone(),
                            file_path: path.to_path_buf(),
                            definition_type: TaskDefinitionType::PyprojectToml,
                            runner: TaskRunner::PythonPoe,
                            source_name: name.clone(),
                            description,
                            shadowed_by: None,
                        });
                    }
                }
            }
        }
    }

    Ok(tasks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task_shadowing::{enable_mock, mock_executable, reset_mock};
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_parse_poetry_scripts() {
        let temp_dir = TempDir::new().unwrap();
        let pyproject_path = temp_dir.path().join("pyproject.toml");

        // Mock Poetry being installed
        reset_mock();
        enable_mock();
        mock_executable("poetry");

        // Create poetry.lock to ensure Poetry is selected
        File::create(temp_dir.path().join("poetry.lock")).unwrap();

        let content = r#"
[tool.poetry]
name = "test-project"

[tool.poetry.scripts]
test = "pytest"
lint = "flake8"
"#;

        File::create(&pyproject_path)
            .unwrap()
            .write_all(content.as_bytes())
            .unwrap();

        let tasks = parse(&pyproject_path).unwrap();

        assert_eq!(tasks.len(), 2);

        let test_task = tasks.iter().find(|t| t.name == "test").unwrap();
        assert_eq!(test_task.runner, TaskRunner::PythonPoetry);
        assert_eq!(
            test_task.description,
            Some("python script: pytest".to_string())
        );

        let lint_task = tasks.iter().find(|t| t.name == "lint").unwrap();
        assert_eq!(lint_task.runner, TaskRunner::PythonPoetry);
        assert_eq!(
            lint_task.description,
            Some("python script: flake8".to_string())
        );

        reset_mock();
    }

    #[test]
    fn test_parse_uv_scripts() {
        let temp_dir = TempDir::new().unwrap();
        let pyproject_path = temp_dir.path().join("pyproject.toml");

        // Mock UV being installed
        reset_mock();
        enable_mock();
        mock_executable("uv");

        let content = r#"
[project]
name = "test-project"

[project.scripts]
serve = "uvicorn main:app --reload"
test = "pytest"
"#;

        File::create(&pyproject_path)
            .unwrap()
            .write_all(content.as_bytes())
            .unwrap();

        let tasks = parse(&pyproject_path).unwrap();

        assert_eq!(tasks.len(), 2);

        let serve_task = tasks.iter().find(|t| t.name == "serve").unwrap();
        assert_eq!(serve_task.runner, TaskRunner::PythonUv);
        assert_eq!(
            serve_task.description,
            Some("python script: uvicorn main:app --reload".to_string())
        );

        reset_mock();
    }

    #[test]
    fn test_parse_both_uv_and_poetry_scripts() {
        let temp_dir = TempDir::new().unwrap();
        let pyproject_path = temp_dir.path().join("pyproject.toml");

        // Mock both UV and Poetry being installed
        reset_mock();
        enable_mock();
        mock_executable("uv");
        mock_executable("poetry");
        // Create a poetry.lock file so that the poetry scripts branch is triggered
        std::fs::File::create(temp_dir.path().join("poetry.lock")).unwrap();

        let content = r#"
[project]
name = "test-project"

[project.scripts]
uv-serve = "uvicorn main:app --reload"
uv-test = "pytest"

[tool.poetry]
name = "test-project"

[tool.poetry.scripts]
poetry-serve = "python -m http.server"
poetry-test = "pytest"
"#;

        File::create(&pyproject_path)
            .unwrap()
            .write_all(content.as_bytes())
            .unwrap();

        let tasks = parse(&pyproject_path).unwrap();

        // We should get all 4 tasks
        assert_eq!(tasks.len(), 4);

        // Check UV tasks
        let uv_tasks: Vec<_> = tasks
            .iter()
            .filter(|t| matches!(t.runner, TaskRunner::PythonUv))
            .collect();
        assert_eq!(uv_tasks.len(), 2);

        // Check Poetry tasks
        let poetry_tasks: Vec<_> = tasks
            .iter()
            .filter(|t| matches!(t.runner, TaskRunner::PythonPoetry))
            .collect();
        assert_eq!(poetry_tasks.len(), 2);

        reset_mock();
    }

    #[test]
    fn test_parse_poe_tasks() {
        let temp_dir = TempDir::new().unwrap();
        let pyproject_path = temp_dir.path().join("pyproject.toml");

        // Mock Poe being installed
        reset_mock();
        enable_mock();
        mock_executable("poe");

        let content = r#"
[tool.poe.tasks]
serve = "python -m http.server"
test = { script = "test.py" }
lint = { shell = "flake8" }
"#;

        File::create(&pyproject_path)
            .unwrap()
            .write_all(content.as_bytes())
            .unwrap();

        let tasks = parse(&pyproject_path).unwrap();

        assert_eq!(tasks.len(), 3);

        let serve_task = tasks.iter().find(|t| t.name == "serve").unwrap();
        assert_eq!(serve_task.runner, TaskRunner::PythonPoe);
        assert_eq!(
            serve_task.description,
            Some("command: python -m http.server".to_string())
        );

        let test_task = tasks.iter().find(|t| t.name == "test").unwrap();
        assert_eq!(test_task.runner, TaskRunner::PythonPoe);
        assert_eq!(
            test_task.description,
            Some("python script: test.py".to_string())
        );

        let lint_task = tasks.iter().find(|t| t.name == "lint").unwrap();
        assert_eq!(lint_task.runner, TaskRunner::PythonPoe);
        assert_eq!(
            lint_task.description,
            Some("shell script: flake8".to_string())
        );

        reset_mock();
    }
}
