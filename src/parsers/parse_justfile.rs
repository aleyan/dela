use crate::types::{Task, TaskDefinitionType, TaskRunner};
use regex::Regex;
use std::path::PathBuf;

/// Parse a Justfile at the given path and extract tasks
pub fn parse(path: &PathBuf) -> Result<Vec<Task>, String> {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Justfile");

    let contents = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", file_name, e))?;

    let mut tasks = Vec::new();

    // Regex to match task definitions in Justfiles
    // Matches patterns like:
    // task_name:
    // task_name: # description
    // task_name: # description with spaces
    // task_name *args: # description
    // task_name: dependency # description
    // task_name *args: dependency # description
    let task_regex = Regex::new(r"^([a-zA-Z_][a-zA-Z0-9_-]*)(?:\s+\*[a-zA-Z_][a-zA-Z0-9_-]*)?:\s*(?:[a-zA-Z_][a-zA-Z0-9_-]*\s+)?(?:#\s*(.+))?$").unwrap();

    for line in contents.lines() {
        let line = line.trim();
        
        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(captures) = task_regex.captures(line) {
            let task_name = captures.get(1).unwrap().as_str().to_string();
            let description = captures.get(2).map(|m| m.as_str().trim().to_string());

            tasks.push(Task {
                name: task_name.clone(),
                file_path: path.clone(),
                definition_type: TaskDefinitionType::Justfile,
                runner: TaskRunner::Just,
                source_name: task_name,
                description,
                shadowed_by: None,
                disambiguated_name: None,
            });
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
    fn test_parse_justfile() {
        let temp_dir = TempDir::new().unwrap();
        let justfile_path = temp_dir.path().join("Justfile");
        let mut file = File::create(&justfile_path).unwrap();

        write!(
            file,
            r#"
# This is a comment
build: # Build the project
    cargo build

test: # Run tests
    cargo test

clean: # Clean build artifacts
    cargo clean

format:
    cargo fmt

# Another comment
lint: # Run linter
    cargo clippy
"#
        )
        .unwrap();

        let tasks = parse(&justfile_path).unwrap();
        assert_eq!(tasks.len(), 5);

        let build_task = tasks.iter().find(|t| t.name == "build").unwrap();
        assert_eq!(build_task.description.as_deref(), Some("Build the project"));
        assert_eq!(build_task.runner, TaskRunner::Just);

        let test_task = tasks.iter().find(|t| t.name == "test").unwrap();
        assert_eq!(test_task.description.as_deref(), Some("Run tests"));
        assert_eq!(test_task.runner, TaskRunner::Just);

        let clean_task = tasks.iter().find(|t| t.name == "clean").unwrap();
        assert_eq!(clean_task.description.as_deref(), Some("Clean build artifacts"));
        assert_eq!(clean_task.runner, TaskRunner::Just);

        let format_task = tasks.iter().find(|t| t.name == "format").unwrap();
        assert_eq!(format_task.description, None);
        assert_eq!(format_task.runner, TaskRunner::Just);

        let lint_task = tasks.iter().find(|t| t.name == "lint").unwrap();
        assert_eq!(lint_task.description.as_deref(), Some("Run linter"));
        assert_eq!(lint_task.runner, TaskRunner::Just);
    }

    #[test]
    fn test_parse_justfile_with_complex_names() {
        let temp_dir = TempDir::new().unwrap();
        let justfile_path = temp_dir.path().join("Justfile");
        let mut file = File::create(&justfile_path).unwrap();

        write!(
            file,
            r#"
build-dev: # Build for development
    cargo build

test-integration: # Run integration tests
    cargo test --test integration

deploy-staging: # Deploy to staging
    echo "Deploying to staging"

# Task with underscore
build_release: # Build release version
    cargo build --release
"#
        )
        .unwrap();

        let tasks = parse(&justfile_path).unwrap();
        assert_eq!(tasks.len(), 4);

        let build_dev_task = tasks.iter().find(|t| t.name == "build-dev").unwrap();
        assert_eq!(build_dev_task.description.as_deref(), Some("Build for development"));

        let test_integration_task = tasks.iter().find(|t| t.name == "test-integration").unwrap();
        assert_eq!(test_integration_task.description.as_deref(), Some("Run integration tests"));

        let deploy_staging_task = tasks.iter().find(|t| t.name == "deploy-staging").unwrap();
        assert_eq!(deploy_staging_task.description.as_deref(), Some("Deploy to staging"));

        let build_release_task = tasks.iter().find(|t| t.name == "build_release").unwrap();
        assert_eq!(build_release_task.description.as_deref(), Some("Build release version"));
    }

    #[test]
    fn test_parse_justfile_with_no_tasks() {
        let temp_dir = TempDir::new().unwrap();
        let justfile_path = temp_dir.path().join("Justfile");
        let mut file = File::create(&justfile_path).unwrap();

        write!(
            file,
            r#"
# This is just a comment
# Another comment

# No tasks defined
"#
        )
        .unwrap();

        let tasks = parse(&justfile_path).unwrap();
        assert_eq!(tasks.len(), 0);
    }

    #[test]
    fn test_parse_justfile_with_invalid_syntax() {
        let temp_dir = TempDir::new().unwrap();
        let justfile_path = temp_dir.path().join("Justfile");
        let mut file = File::create(&justfile_path).unwrap();

        write!(
            file,
            r#"
# Valid task
build: # Build the project
    cargo build

# Invalid syntax (no colon)
invalid-task
    echo "This won't be parsed"

# Another valid task
test: # Run tests
    cargo test
"#
        )
        .unwrap();

        let tasks = parse(&justfile_path).unwrap();
        assert_eq!(tasks.len(), 2);

        // Only the valid tasks should be parsed
        assert!(tasks.iter().find(|t| t.name == "build").is_some());
        assert!(tasks.iter().find(|t| t.name == "test").is_some());
        assert!(tasks.iter().find(|t| t.name == "invalid-task").is_none());
    }

    #[test]
    fn test_parse_justfile_with_multiline_commands() {
        let temp_dir = TempDir::new().unwrap();
        let justfile_path = temp_dir.path().join("Justfile");
        let mut file = File::create(&justfile_path).unwrap();

        write!(
            file,
            r#"
# Task with multiline command
build: # Build the project
    cargo build
    cargo test

# Task with complex multiline command
deploy: # Deploy to production
    echo "Starting deployment..."
    docker build -t myapp .
    docker push myapp:latest
    echo "Deployment complete!"

# Task with no description but multiline
setup:
    mkdir -p build
    cargo fetch
    echo "Setup complete"

# Task with description but single line
lint: # Run linter
    cargo clippy
"#
        )
        .unwrap();

        let tasks = parse(&justfile_path).unwrap();
        assert_eq!(tasks.len(), 4);

        let build_task = tasks.iter().find(|t| t.name == "build").unwrap();
        assert_eq!(build_task.description.as_deref(), Some("Build the project"));

        let deploy_task = tasks.iter().find(|t| t.name == "deploy").unwrap();
        assert_eq!(deploy_task.description.as_deref(), Some("Deploy to production"));

        let setup_task = tasks.iter().find(|t| t.name == "setup").unwrap();
        assert_eq!(setup_task.description, None);

        let lint_task = tasks.iter().find(|t| t.name == "lint").unwrap();
        assert_eq!(lint_task.description.as_deref(), Some("Run linter"));
    }

    #[test]
    fn test_parse_justfile_with_complex_syntax() {
        let temp_dir = TempDir::new().unwrap();
        let justfile_path = temp_dir.path().join("Justfile");
        let mut file = File::create(&justfile_path).unwrap();

        write!(
            file,
            r#"
# Task with parameters
build *args: # Build with arguments
    cargo build -- $args

# Task with dependencies
test: build # Run tests after building
    cargo test

# Task with conditional logic
release: # Build release version
    if [ "$(git branch --show-current)" = "main" ]; then
        cargo build --release
    else
        echo "Not on main branch"
        exit 1
    fi

# Task with shebang
script: # Run a script
    #!/usr/bin/env bash
    echo "Running script"
    ./script.sh

# Task with heredoc
docs: # Generate documentation
    cat << EOF > README.md
    # My Project
    This is the documentation.
    EOF
"#
        )
        .unwrap();

        let tasks = parse(&justfile_path).unwrap();
        assert_eq!(tasks.len(), 5);

        let build_task = tasks.iter().find(|t| t.name == "build").unwrap();
        assert_eq!(build_task.description.as_deref(), Some("Build with arguments"));

        let test_task = tasks.iter().find(|t| t.name == "test").unwrap();
        assert_eq!(test_task.description.as_deref(), Some("Run tests after building"));

        let release_task = tasks.iter().find(|t| t.name == "release").unwrap();
        assert_eq!(release_task.description.as_deref(), Some("Build release version"));

        let script_task = tasks.iter().find(|t| t.name == "script").unwrap();
        assert_eq!(script_task.description.as_deref(), Some("Run a script"));

        let docs_task = tasks.iter().find(|t| t.name == "docs").unwrap();
        assert_eq!(docs_task.description.as_deref(), Some("Generate documentation"));
    }

    #[test]
    fn test_parse_justfile_with_edge_cases() {
        let temp_dir = TempDir::new().unwrap();
        let justfile_path = temp_dir.path().join("Justfile");
        let mut file = File::create(&justfile_path).unwrap();

        write!(
            file,
            r#"
# Task with special characters in description
build: # Build the project (with special chars: @#$%^&*)
    cargo build

# Task with no description but lots of whitespace
test:    
    cargo test

# Task with description containing colons
deploy: # Deploy to: staging, production
    echo "Deploying"

# Task with description containing hashes
docs: # Generate #documentation with #hashtags
    echo "Generating docs"

# Task with description containing quotes
lint: # Run "linter" and 'check' code
    cargo clippy

# Task with description containing newlines (should be trimmed)
format: # Format code
    # This is a comment in the command
    cargo fmt

# Task with description containing tabs
clean: # Clean	build	artifacts
    cargo clean
"#
        )
        .unwrap();

        let tasks = parse(&justfile_path).unwrap();
        assert_eq!(tasks.len(), 7);

        let build_task = tasks.iter().find(|t| t.name == "build").unwrap();
        assert_eq!(build_task.description.as_deref(), Some("Build the project (with special chars: @#$%^&*)"));

        let test_task = tasks.iter().find(|t| t.name == "test").unwrap();
        assert_eq!(test_task.description, None);

        let deploy_task = tasks.iter().find(|t| t.name == "deploy").unwrap();
        assert_eq!(deploy_task.description.as_deref(), Some("Deploy to: staging, production"));

        let docs_task = tasks.iter().find(|t| t.name == "docs").unwrap();
        assert_eq!(docs_task.description.as_deref(), Some("Generate #documentation with #hashtags"));

        let lint_task = tasks.iter().find(|t| t.name == "lint").unwrap();
        assert_eq!(lint_task.description.as_deref(), Some("Run \"linter\" and 'check' code"));

        let format_task = tasks.iter().find(|t| t.name == "format").unwrap();
        assert_eq!(format_task.description.as_deref(), Some("Format code"));

        let clean_task = tasks.iter().find(|t| t.name == "clean").unwrap();
        assert_eq!(clean_task.description.as_deref(), Some("Clean\tbuild\tartifacts"));
    }
} 