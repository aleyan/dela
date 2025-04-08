use crate::types::{Task, TaskDefinitionType, TaskRunner};
use serde_yaml::Value;
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Parse GitHub Actions workflow file and extract workflows as tasks
///
/// This function parses a GitHub Actions workflow file and extracts the entire workflow as a single task.
/// The tasks can be executed using the `act` command-line tool.
pub fn parse(file_path: &Path) -> Result<Vec<Task>, String> {
    let mut file = File::open(file_path).map_err(|e| format!("Failed to open file: {}", e))?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    parse_workflow_string(&contents, file_path)
}

/// Parse GitHub Actions workflow content from a string
fn parse_workflow_string(content: &str, file_path: &Path) -> Result<Vec<Task>, String> {
    let workflow: Value = serde_yaml::from_str(content)
        .map_err(|e| format!("Failed to parse workflow YAML: {}", e))?;

    let workflow_map = match workflow {
        Value::Mapping(map) => map,
        _ => return Err("Workflow YAML is not a mapping".to_string()),
    };

    // Try to get workflow name for description
    let workflow_name = workflow_map
        .get(&Value::String("name".to_string()))
        .and_then(|v| match v {
            Value::String(s) => Some(s.clone()),
            _ => None,
        });

    // Extract jobs to confirm the workflow is valid
    let jobs = match workflow_map.get(&Value::String("jobs".to_string())) {
        Some(Value::Mapping(jobs_map)) => jobs_map,
        _ => return Err("No jobs found in workflow file".to_string()),
    };

    if jobs.is_empty() {
        return Err("Workflow contains no jobs".to_string());
    }

    // Extract filename without path for task name
    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| {
            if n.ends_with(".yml") || n.ends_with(".yaml") {
                &n[0..n.rfind('.').unwrap_or(n.len())]
            } else {
                n
            }
        })
        .unwrap_or("workflow");

    // Create a single task for the entire workflow
    // Always use file name as the task name
    let task_name = file_name.to_string();

    let task = Task {
        name: task_name.clone(),
        file_path: file_path.to_path_buf(),
        definition_type: TaskDefinitionType::GitHubActions,
        runner: TaskRunner::Act,
        source_name: task_name, // Source name is the same as the task name (entire workflow)
        description: workflow_name,
        shadowed_by: None,
    };

    Ok(vec![task])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_test_workflow(dir: &Path, filename: &str, content: &str) -> PathBuf {
        let file_path = dir.join(filename);
        fs::write(&file_path, content).expect("Failed to write test workflow file");
        file_path
    }

    #[test]
    fn test_parse_simple_workflow() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");

        let workflow_content = r#"
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
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Test
        run: echo "Testing..."
"#;

        let file_path = create_test_workflow(&temp_dir.path(), "workflow.yml", workflow_content);

        let tasks = parse(&file_path).expect("Failed to parse workflow");

        assert_eq!(tasks.len(), 1, "Should have one task");

        let task = &tasks[0];
        assert_eq!(task.name, "workflow");
        assert_eq!(task.definition_type, TaskDefinitionType::GitHubActions);
        assert_eq!(task.runner, TaskRunner::Act);
        assert_eq!(task.source_name, "workflow");
        assert_eq!(task.description, Some("CI".to_string()));
    }

    #[test]
    fn test_parse_complex_workflow() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");

        let workflow_content = r#"
name: Complex Workflow
on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main ]
  workflow_dispatch:
    inputs:
      environment:
        description: 'Environment to deploy to'
        required: true
        default: 'staging'

jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Lint Code
        run: echo "Linting..."

  build:
    runs-on: ubuntu-latest
    needs: lint
    strategy:
      matrix:
        node-version: [14.x, 16.x, 18.x]
        os: [ubuntu-latest, windows-latest]
    steps:
      - uses: actions/checkout@v3
      - name: Use Node.js ${{ matrix.node-version }}
        uses: actions/setup-node@v3
        with:
          node-version: ${{ matrix.node-version }}
      - name: Build
        run: |
          npm ci
          npm run build

  test:
    runs-on: ubuntu-latest
    needs: build
    steps:
      - uses: actions/checkout@v3
      - name: Test
        run: npm test

  deploy:
    if: github.event_name == 'workflow_dispatch' || github.ref == 'refs/heads/main'
    runs-on: ubuntu-latest
    needs: [build, test]
    environment: ${{ github.event.inputs.environment || 'production' }}
    steps:
      - uses: actions/checkout@v3
      - name: Deploy
        run: echo "Deploying to ${{ github.event.inputs.environment || 'production' }}"
"#;

        let file_path =
            create_test_workflow(&temp_dir.path(), "complex-workflow.yml", workflow_content);

        let tasks = parse(&file_path).expect("Failed to parse complex workflow");

        assert_eq!(tasks.len(), 1, "Should have one task");

        let task = &tasks[0];
        assert_eq!(task.name, "complex-workflow");
        assert_eq!(task.definition_type, TaskDefinitionType::GitHubActions);
        assert_eq!(task.runner, TaskRunner::Act);
        assert_eq!(task.source_name, "complex-workflow");
        assert_eq!(task.description, Some("Complex Workflow".to_string()));
    }

    #[test]
    fn test_parse_multiple_workflows() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");

        // Create .github/workflows directory structure
        let workflows_dir = temp_dir.path().join(".github").join("workflows");
        fs::create_dir_all(&workflows_dir).expect("Failed to create workflows directory");

        // Create first workflow
        let ci_workflow = r#"
name: CI
on: [push, pull_request]
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Build
        run: echo "Building..."
"#;
        let ci_path = workflows_dir.join("ci.yml");
        fs::write(&ci_path, ci_workflow).expect("Failed to write ci workflow");

        // Create second workflow
        let deploy_workflow = r#"
name: Deploy
on:
  push:
    branches: [main]
jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Deploy
        run: echo "Deploying..."
"#;
        let deploy_path = workflows_dir.join("deploy.yml");
        fs::write(&deploy_path, deploy_workflow).expect("Failed to write deploy workflow");

        // Parse both workflows
        let ci_tasks = parse(&ci_path).expect("Failed to parse CI workflow");
        let deploy_tasks = parse(&deploy_path).expect("Failed to parse Deploy workflow");

        // Verify CI workflow tasks
        assert_eq!(ci_tasks.len(), 1);
        assert_eq!(ci_tasks[0].name, "ci");
        assert_eq!(ci_tasks[0].description, Some("CI".to_string()));

        // Verify Deploy workflow tasks
        assert_eq!(deploy_tasks.len(), 1);
        assert_eq!(deploy_tasks[0].name, "deploy");
        assert_eq!(deploy_tasks[0].description, Some("Deploy".to_string()));
    }

    #[test]
    fn test_parse_workflow_without_name() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");

        let workflow_content = r#"
on:
  push:
    branches: [ main ]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Build
        run: echo "Building..."
"#;

        let file_path =
            create_test_workflow(&temp_dir.path(), "unnamed-workflow.yml", workflow_content);

        let tasks = parse(&file_path).expect("Failed to parse workflow without name");

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].name, "unnamed-workflow");
        assert_eq!(tasks[0].definition_type, TaskDefinitionType::GitHubActions);
        assert_eq!(tasks[0].runner, TaskRunner::Act);
        assert_eq!(
            tasks[0].description, None,
            "Description should be None when workflow has no name"
        );
    }

    #[test]
    fn test_parse_invalid_workflow() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");

        // Invalid YAML
        let workflow_content = r#"
name: Invalid Workflow
on: [push
jobs:
  build:
    runs-on: ubuntu-latest
"#;

        let file_path =
            create_test_workflow(&temp_dir.path(), "invalid-workflow.yml", workflow_content);

        let result = parse(&file_path);
        assert!(result.is_err(), "Should fail with invalid YAML");

        // Valid YAML but missing jobs section
        let workflow_content = r#"
name: No Jobs
on: [push]
"#;

        let file_path = create_test_workflow(&temp_dir.path(), "no-jobs.yml", workflow_content);

        let result = parse(&file_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No jobs found"));
    }
}
