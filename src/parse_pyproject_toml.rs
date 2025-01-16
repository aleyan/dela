use std::fs;
use std::path::Path;
use toml::Value;
use crate::types::{Task, TaskRunner, TaskDefinitionFile, TaskFileStatus};

/// Create a TaskDefinitionFile for a pyproject.toml
pub fn create_definition(path: &Path, status: TaskFileStatus, runner: TaskRunner) -> TaskDefinitionFile {
    TaskDefinitionFile {
        path: path.to_path_buf(),
        runner,
        status,
    }
}

/// Parse a pyproject.toml file at the given path and extract tasks
pub fn parse(path: &Path) -> Result<(Vec<Task>, TaskRunner), String> {
    // Read and parse the pyproject.toml file
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read pyproject.toml: {}", e))?;
    
    let toml: Value = content.parse()
        .map_err(|e| format!("Failed to parse pyproject.toml: {}", e))?;

    // Try to find poetry scripts first
    if let Some(poetry_scripts) = toml.get("tool")
        .and_then(|t| t.get("poetry"))
        .and_then(|p| p.get("scripts"))
        .and_then(|s| s.as_table()) {
        let tasks = extract_tasks(poetry_scripts, path, TaskRunner::PythonPoetry)?;
        if !tasks.is_empty() {
            return Ok((tasks, TaskRunner::PythonPoetry));
        }
    }

    // Try to find project scripts (uv)
    if let Some(project_scripts) = toml.get("project")
        .and_then(|p| p.get("scripts"))
        .and_then(|s| s.as_table()) {
        let tasks = extract_tasks(project_scripts, path, TaskRunner::PythonUv)?;
        if !tasks.is_empty() {
            return Ok((tasks, TaskRunner::PythonUv));
        }
    }

    Ok((vec![], TaskRunner::PythonUv)) // Default to UV if no tasks found
}

fn extract_tasks(scripts: &toml::map::Map<String, Value>, path: &Path, runner: TaskRunner) -> Result<Vec<Task>, String> {
    let mut tasks = Vec::new();
    
    for (name, value) in scripts {
        let description = match value {
            Value::String(cmd) => Some(format!("python script: {}", cmd)),
            Value::Table(t) => t.get("description")
                .and_then(|d| d.as_str())
                .map(|s| s.to_string()),
            _ => None,
        };

        tasks.push(Task {
            name: name.clone(),
            file_path: path.to_path_buf(),
            runner: runner.clone(),
            source_name: name.clone(),
            description,
        });
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

        let (tasks, runner) = parse(&pyproject_path).unwrap();
        
        assert_eq!(runner, TaskRunner::PythonPoetry);
        assert_eq!(tasks.len(), 2);
        
        let test_task = tasks.iter().find(|t| t.name == "test").unwrap();
        assert_eq!(test_task.runner, TaskRunner::PythonPoetry);
        assert_eq!(test_task.description, Some("python script: pytest".to_string()));
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

        let (tasks, runner) = parse(&pyproject_path).unwrap();
        
        assert_eq!(runner, TaskRunner::PythonUv);
        assert_eq!(tasks.len(), 2);
        
        let serve_task = tasks.iter().find(|t| t.name == "serve").unwrap();
        assert_eq!(serve_task.runner, TaskRunner::PythonUv);
        assert_eq!(serve_task.description, Some("python script: uvicorn main:app --reload".to_string()));
    }
} 