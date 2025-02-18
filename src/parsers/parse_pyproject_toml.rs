use crate::runners::runners_pyproject_toml;
use crate::task_shadowing::check_path_executable;
use crate::types::{Task, TaskDefinitionFile, TaskDefinitionType, TaskFileStatus, TaskRunner};
use std::fs;
use std::path::Path;

/// Create a TaskDefinitionFile for a pyproject.toml
pub fn create_definition(path: &Path, status: TaskFileStatus) -> TaskDefinitionFile {
    TaskDefinitionFile {
        path: path.to_path_buf(),
        definition_type: TaskDefinitionType::PyprojectToml,
        status,
    }
}

/// Parse a pyproject.toml file at the given path and extract tasks
pub fn parse(path: &Path) -> Result<Vec<Task>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read pyproject.toml: {}", e))?;

    let toml: toml::Value =
        toml::from_str(&content).map_err(|e| format!("Failed to parse pyproject.toml: {}", e))?;

    let mut tasks = Vec::new();

    // Check for poetry configuration
    if let Some(tool) = toml.get("tool") {
        if let Some(poetry) = tool.get("poetry") {
            if let Some(scripts) = poetry.get("scripts") {
                if let Some(scripts_table) = scripts.as_table() {
                    if check_path_executable("poetry").is_some() {
                        for (name, cmd) in scripts_table {
                            tasks.push(Task {
                                name: name.clone(),
                                file_path: path.to_path_buf(),
                                definition_type: TaskDefinitionType::PyprojectToml,
                                runner: TaskRunner::PythonPoetry,
                                source_name: name.clone(),
                                description: cmd.as_str().map(|s| format!("python script: {}", s)),
                                shadowed_by: None,
                            });
                        }
                    }
                }
            }
        }
    }

    // Check for uv configuration
    if let Some(project) = toml.get("project") {
        if let Some(scripts) = project.get("scripts") {
            if let Some(scripts_table) = scripts.as_table() {
                if check_path_executable("uv").is_some() {
                    for (name, cmd) in scripts_table {
                        tasks.push(Task {
                            name: name.clone(),
                            file_path: path.to_path_buf(),
                            definition_type: TaskDefinitionType::PyprojectToml,
                            runner: TaskRunner::PythonUv,
                            source_name: name.clone(),
                            description: cmd.as_str().map(|s| format!("python script: {}", s)),
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
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_parse_poetry_scripts() {
        let temp_dir = TempDir::new().unwrap();
        let pyproject_path = temp_dir.path().join("pyproject.toml");

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
        assert!(matches!(
            test_task.runner,
            TaskRunner::PythonUv | TaskRunner::PythonPoetry
        ));
        assert_eq!(
            test_task.description,
            Some("python script: pytest".to_string())
        );
    }

    #[test]
    fn test_parse_uv_scripts() {
        let temp_dir = TempDir::new().unwrap();
        let pyproject_path = temp_dir.path().join("pyproject.toml");

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
        assert!(matches!(
            serve_task.runner,
            TaskRunner::PythonUv | TaskRunner::PythonPoetry
        ));
        assert_eq!(
            serve_task.description,
            Some("python script: uvicorn main:app --reload".to_string())
        );
    }

    #[test]
    fn test_parse_both_uv_and_poetry_scripts() {
        let temp_dir = TempDir::new().unwrap();
        let pyproject_path = temp_dir.path().join("pyproject.toml");

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

        // Check tasks
        let uv_serve = tasks.iter().find(|t| t.name == "uv-serve").unwrap();
        assert!(matches!(
            uv_serve.runner,
            TaskRunner::PythonUv | TaskRunner::PythonPoetry
        ));
        assert_eq!(
            uv_serve.description,
            Some("python script: uvicorn main:app --reload".to_string())
        );

        let poetry_serve = tasks.iter().find(|t| t.name == "poetry-serve").unwrap();
        assert!(matches!(
            poetry_serve.runner,
            TaskRunner::PythonUv | TaskRunner::PythonPoetry
        ));
        assert_eq!(
            poetry_serve.description,
            Some("python script: python -m http.server".to_string())
        );
    }
}
