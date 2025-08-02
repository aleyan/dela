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
    let lines: Vec<&str> = contents.lines().collect();

    // Regex to match task definitions in Justfiles
    // Matches patterns like:
    // task_name:
    // task_name: # description
    // task_name: # description with spaces
    // task_name *args: # description
    // task_name: dependency # description
    // task_name *args: dependency # description
    let task_regex = Regex::new(r"^([a-zA-Z_][a-zA-Z0-9_-]*)(?:\s+\*[a-zA-Z_][a-zA-Z0-9_-]*)?:\s*(?:[a-zA-Z_][a-zA-Z0-9_-]*\s+)?(?:#\s*(.+))?$").unwrap();

    for (line_num, line) in lines.iter().enumerate() {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(captures) = task_regex.captures(line) {
            let task_name = captures.get(1).unwrap().as_str().to_string();
            let description = captures.get(2).map(|m| m.as_str().trim().to_string());

            // Validate indentation for this recipe
            if let Err(indent_error) = validate_recipe_indentation(&lines, line_num + 1) {
                return Err(format!("{}: {}", file_name, indent_error));
            }

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

/// Validate that a recipe's lines use consistent indentation
fn validate_recipe_indentation(lines: &[&str], task_line_num: usize) -> Result<(), String> {
    let mut recipe_lines = Vec::new();
    let mut current_line = task_line_num;

    // Find all lines that belong to this recipe (indented lines after the task definition)
    while current_line < lines.len() {
        let line = lines[current_line];

        // Skip empty lines and comments
        if line.trim().is_empty() || line.trim().starts_with('#') {
            current_line += 1;
            continue;
        }

        // Check if this line is indented (part of the recipe)
        if is_indented_line(line) {
            recipe_lines.push((current_line + 1, line));
            current_line += 1;
        } else {
            // This line is not indented, so we've reached the end of the recipe
            break;
        }
    }

    // If there are no recipe lines, no validation needed
    if recipe_lines.is_empty() {
        return Ok(());
    }

    // Determine the indentation type of the first recipe line
    let first_line = recipe_lines[0].1;
    let first_indent_type = get_indentation_type(first_line);

    // Validate that all recipe lines use the same indentation type
    for (line_num, line) in recipe_lines.iter().skip(1) {
        let indent_type = get_indentation_type(line);
        if indent_type != first_indent_type {
            return Err(format!(
                "line {}: mixed indentation in recipe - found both spaces and tabs",
                line_num
            ));
        }
    }

    Ok(())
}

/// Check if a line is indented (has leading whitespace)
fn is_indented_line(line: &str) -> bool {
    line.starts_with(' ') || line.starts_with('\t')
}

/// Determine the indentation type of a line
fn get_indentation_type(line: &str) -> IndentationType {
    let leading_whitespace = line
        .chars()
        .take_while(|c| c.is_whitespace())
        .collect::<String>();

    if leading_whitespace.is_empty() {
        return IndentationType::None;
    }

    if leading_whitespace.chars().all(|c| c == ' ') {
        IndentationType::Spaces
    } else if leading_whitespace.chars().all(|c| c == '\t') {
        IndentationType::Tabs
    } else {
        IndentationType::Mixed
    }
}

#[derive(Debug, PartialEq)]
enum IndentationType {
    None,
    Spaces,
    Tabs,
    Mixed,
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
        assert_eq!(
            clean_task.description.as_deref(),
            Some("Clean build artifacts")
        );
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
        assert_eq!(
            build_dev_task.description.as_deref(),
            Some("Build for development")
        );

        let test_integration_task = tasks.iter().find(|t| t.name == "test-integration").unwrap();
        assert_eq!(
            test_integration_task.description.as_deref(),
            Some("Run integration tests")
        );

        let deploy_staging_task = tasks.iter().find(|t| t.name == "deploy-staging").unwrap();
        assert_eq!(
            deploy_staging_task.description.as_deref(),
            Some("Deploy to staging")
        );

        let build_release_task = tasks.iter().find(|t| t.name == "build_release").unwrap();
        assert_eq!(
            build_release_task.description.as_deref(),
            Some("Build release version")
        );
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
        assert_eq!(
            deploy_task.description.as_deref(),
            Some("Deploy to production")
        );

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
        assert_eq!(
            build_task.description.as_deref(),
            Some("Build with arguments")
        );

        let test_task = tasks.iter().find(|t| t.name == "test").unwrap();
        assert_eq!(
            test_task.description.as_deref(),
            Some("Run tests after building")
        );

        let release_task = tasks.iter().find(|t| t.name == "release").unwrap();
        assert_eq!(
            release_task.description.as_deref(),
            Some("Build release version")
        );

        let script_task = tasks.iter().find(|t| t.name == "script").unwrap();
        assert_eq!(script_task.description.as_deref(), Some("Run a script"));

        let docs_task = tasks.iter().find(|t| t.name == "docs").unwrap();
        assert_eq!(
            docs_task.description.as_deref(),
            Some("Generate documentation")
        );
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
        assert_eq!(
            build_task.description.as_deref(),
            Some("Build the project (with special chars: @#$%^&*)")
        );

        let test_task = tasks.iter().find(|t| t.name == "test").unwrap();
        assert_eq!(test_task.description, None);

        let deploy_task = tasks.iter().find(|t| t.name == "deploy").unwrap();
        assert_eq!(
            deploy_task.description.as_deref(),
            Some("Deploy to: staging, production")
        );

        let docs_task = tasks.iter().find(|t| t.name == "docs").unwrap();
        assert_eq!(
            docs_task.description.as_deref(),
            Some("Generate #documentation with #hashtags")
        );

        let lint_task = tasks.iter().find(|t| t.name == "lint").unwrap();
        assert_eq!(
            lint_task.description.as_deref(),
            Some("Run \"linter\" and 'check' code")
        );

        let format_task = tasks.iter().find(|t| t.name == "format").unwrap();
        assert_eq!(format_task.description.as_deref(), Some("Format code"));

        let clean_task = tasks.iter().find(|t| t.name == "clean").unwrap();
        assert_eq!(
            clean_task.description.as_deref(),
            Some("Clean\tbuild\tartifacts")
        );
    }

    #[test]
    fn test_parse_justfile_with_spaces_indentation() {
        let temp_dir = TempDir::new().unwrap();
        let justfile_path = temp_dir.path().join("Justfile");
        let mut file = File::create(&justfile_path).unwrap();

        write!(
            file,
            r#"
# Tasks with spaces indentation
build: # Build the project
    cargo build
    cargo test

deploy: # Deploy to production
    echo "Starting deployment..."
    docker build -t myapp .
    docker push myapp:latest
    echo "Deployment complete!"

setup: # Setup project
    mkdir -p build
    cargo fetch
"#
        )
        .unwrap();

        let tasks = parse(&justfile_path).unwrap();
        assert_eq!(tasks.len(), 3);

        let build_task = tasks.iter().find(|t| t.name == "build").unwrap();
        assert_eq!(build_task.description.as_deref(), Some("Build the project"));

        let deploy_task = tasks.iter().find(|t| t.name == "deploy").unwrap();
        assert_eq!(
            deploy_task.description.as_deref(),
            Some("Deploy to production")
        );

        let setup_task = tasks.iter().find(|t| t.name == "setup").unwrap();
        assert_eq!(setup_task.description.as_deref(), Some("Setup project"));
    }

    #[test]
    fn test_parse_justfile_with_tabs_indentation() {
        let temp_dir = TempDir::new().unwrap();
        let justfile_path = temp_dir.path().join("Justfile");
        let mut file = File::create(&justfile_path).unwrap();

        write!(
            file,
            r#"
# Tasks with tabs indentation
build: # Build the project
	cargo build
	cargo test

deploy: # Deploy to production
	echo "Starting deployment..."
	docker build -t myapp .
	docker push myapp:latest
	echo "Deployment complete!"

setup: # Setup project
	mkdir -p build
	cargo fetch
"#
        )
        .unwrap();

        let tasks = parse(&justfile_path).unwrap();
        assert_eq!(tasks.len(), 3);

        let build_task = tasks.iter().find(|t| t.name == "build").unwrap();
        assert_eq!(build_task.description.as_deref(), Some("Build the project"));

        let deploy_task = tasks.iter().find(|t| t.name == "deploy").unwrap();
        assert_eq!(
            deploy_task.description.as_deref(),
            Some("Deploy to production")
        );

        let setup_task = tasks.iter().find(|t| t.name == "setup").unwrap();
        assert_eq!(setup_task.description.as_deref(), Some("Setup project"));
    }

    #[test]
    fn test_parse_justfile_with_mixed_indentation_types() {
        let temp_dir = TempDir::new().unwrap();
        let justfile_path = temp_dir.path().join("Justfile");
        let mut file = File::create(&justfile_path).unwrap();

        write!(
            file,
            r#"
# Different recipes can use different indentation types
build: # Build with spaces
    cargo build
    cargo test

deploy: # Deploy with tabs
	echo "Starting deployment..."
	docker build -t myapp .
	docker push myapp:latest

setup: # Setup with spaces
    mkdir -p build
    cargo fetch

clean: # Clean with tabs
	cargo clean
	rm -rf target/
"#
        )
        .unwrap();

        let tasks = parse(&justfile_path).unwrap();
        assert_eq!(tasks.len(), 4);

        let build_task = tasks.iter().find(|t| t.name == "build").unwrap();
        assert_eq!(build_task.description.as_deref(), Some("Build with spaces"));

        let deploy_task = tasks.iter().find(|t| t.name == "deploy").unwrap();
        assert_eq!(deploy_task.description.as_deref(), Some("Deploy with tabs"));

        let setup_task = tasks.iter().find(|t| t.name == "setup").unwrap();
        assert_eq!(setup_task.description.as_deref(), Some("Setup with spaces"));

        let clean_task = tasks.iter().find(|t| t.name == "clean").unwrap();
        assert_eq!(clean_task.description.as_deref(), Some("Clean with tabs"));
    }

    #[test]
    fn test_parse_justfile_with_mixed_indentation_error() {
        let temp_dir = TempDir::new().unwrap();
        let justfile_path = temp_dir.path().join("Justfile");
        let mut file = File::create(&justfile_path).unwrap();

        write!(
            file,
            r#"
# This recipe has mixed indentation (spaces and tabs) - should error
build: # Build the project
    cargo build
	cargo test
    cargo fmt
"#
        )
        .unwrap();

        let result = parse(&justfile_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("mixed indentation in recipe"));
    }

    #[test]
    fn test_parse_justfile_with_mixed_indentation_error_tabs_first() {
        let temp_dir = TempDir::new().unwrap();
        let justfile_path = temp_dir.path().join("Justfile");
        let mut file = File::create(&justfile_path).unwrap();

        write!(
            file,
            r#"
# This recipe has mixed indentation (tabs first, then spaces) - should error
build: # Build the project
	cargo build
    cargo test
	cargo fmt
"#
        )
        .unwrap();

        let result = parse(&justfile_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("mixed indentation in recipe"));
    }

    #[test]
    fn test_parse_justfile_with_no_recipe_lines() {
        let temp_dir = TempDir::new().unwrap();
        let justfile_path = temp_dir.path().join("Justfile");
        let mut file = File::create(&justfile_path).unwrap();

        write!(
            file,
            r#"
# Task with no recipe lines
build: # Build the project

# Another task with no recipe lines
test: # Run tests

# Task with recipe lines
deploy: # Deploy to production
    echo "Deploying"
"#
        )
        .unwrap();

        let tasks = parse(&justfile_path).unwrap();
        assert_eq!(tasks.len(), 3);

        let build_task = tasks.iter().find(|t| t.name == "build").unwrap();
        assert_eq!(build_task.description.as_deref(), Some("Build the project"));

        let test_task = tasks.iter().find(|t| t.name == "test").unwrap();
        assert_eq!(test_task.description.as_deref(), Some("Run tests"));

        let deploy_task = tasks.iter().find(|t| t.name == "deploy").unwrap();
        assert_eq!(
            deploy_task.description.as_deref(),
            Some("Deploy to production")
        );
    }

    #[test]
    fn test_parse_justfile_with_comments_in_recipes() {
        let temp_dir = TempDir::new().unwrap();
        let justfile_path = temp_dir.path().join("Justfile");
        let mut file = File::create(&justfile_path).unwrap();

        write!(
            file,
            r#"
# Task with comments in recipe (spaces)
build: # Build the project
    # This is a comment in the recipe
    cargo build
    # Another comment
    cargo test

# Task with comments in recipe (tabs)
deploy: # Deploy to production
	# This is a comment in the recipe
	echo "Starting deployment..."
	# Another comment
	docker build -t myapp .
"#
        )
        .unwrap();

        let tasks = parse(&justfile_path).unwrap();
        assert_eq!(tasks.len(), 2);

        let build_task = tasks.iter().find(|t| t.name == "build").unwrap();
        assert_eq!(build_task.description.as_deref(), Some("Build the project"));

        let deploy_task = tasks.iter().find(|t| t.name == "deploy").unwrap();
        assert_eq!(
            deploy_task.description.as_deref(),
            Some("Deploy to production")
        );
    }

    #[test]
    fn test_indentation_validation_functions() {
        // Test is_indented_line
        assert!(is_indented_line("    cargo build"));
        assert!(is_indented_line("\tcargo build"));
        assert!(!is_indented_line("cargo build"));
        assert!(!is_indented_line(""));

        // Test get_indentation_type
        assert_eq!(get_indentation_type("cargo build"), IndentationType::None);
        assert_eq!(
            get_indentation_type("    cargo build"),
            IndentationType::Spaces
        );
        assert_eq!(get_indentation_type("\tcargo build"), IndentationType::Tabs);
        assert_eq!(
            get_indentation_type("  \tcargo build"),
            IndentationType::Mixed
        );
        assert_eq!(
            get_indentation_type("\t  cargo build"),
            IndentationType::Mixed
        );
    }

    #[test]
    fn test_parse_justfile_with_correct_mixed_indentation() {
        let temp_dir = TempDir::new().unwrap();
        let justfile_path = temp_dir.path().join("Justfile");
        let mut file = File::create(&justfile_path).unwrap();

        write!(
            file,
            r#"
# Test Justfile demonstrating correct Justfile indentation rules
# Different recipes can use different indentation types

# Recipe using spaces
build: # Build the project
    cargo build
    cargo test
    cargo fmt

# Recipe using tabs
deploy: # Deploy to production
	echo "Starting deployment..."
	docker build -t myapp .
	docker push myapp:latest
	echo "Deployment complete!"

# Another recipe using spaces
setup: # Setup project
    mkdir -p build
    cargo fetch
    echo "Setup complete"

# Another recipe using tabs
clean: # Clean project
	cargo clean
	rm -rf target/
	echo "Clean complete"
"#
        )
        .unwrap();

        let tasks = parse(&justfile_path).unwrap();
        assert_eq!(tasks.len(), 4);

        let build_task = tasks.iter().find(|t| t.name == "build").unwrap();
        assert_eq!(build_task.description.as_deref(), Some("Build the project"));

        let deploy_task = tasks.iter().find(|t| t.name == "deploy").unwrap();
        assert_eq!(
            deploy_task.description.as_deref(),
            Some("Deploy to production")
        );

        let setup_task = tasks.iter().find(|t| t.name == "setup").unwrap();
        assert_eq!(setup_task.description.as_deref(), Some("Setup project"));

        let clean_task = tasks.iter().find(|t| t.name == "clean").unwrap();
        assert_eq!(clean_task.description.as_deref(), Some("Clean project"));
    }
}
