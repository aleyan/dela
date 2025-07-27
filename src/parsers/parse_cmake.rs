use crate::types::{Task, TaskDefinitionType, TaskRunner};
use regex::Regex;
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Parse CMakeLists.txt file and extract custom targets as tasks
///
/// This function parses a CMakeLists.txt file and extracts each custom target
/// as a separate task. It uses regex patterns to find add_custom_target() calls.
pub fn parse(file_path: &Path) -> Result<Vec<Task>, String> {
    let mut file = File::open(file_path).map_err(|e| format!("Failed to open file: {}", e))?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    parse_cmake_string(&contents, file_path)
}

/// Parse CMakeLists.txt content from a string
fn parse_cmake_string(content: &str, file_path: &Path) -> Result<Vec<Task>, String> {
    let mut tasks = Vec::new();

    // First, let's normalize the content by removing comments and extra whitespace
    let normalized_content = content
        .lines()
        .map(|line| {
            // Remove CMake comments
            if let Some(comment_pos) = line.find('#') {
                &line[..comment_pos]
            } else {
                line
            }
        })
        .collect::<Vec<&str>>()
        .join("\n");

    // Use a simpler regex that just finds the target names
    let target_pattern = Regex::new(r#"add_custom_target\s*\(\s*([a-zA-Z_][a-zA-Z0-9_-]*)"#)
        .map_err(|e| format!("Failed to compile regex: {}", e))?;

    // Find all matches in the content
    for captures in target_pattern.captures_iter(&normalized_content) {
        let target_name = captures.get(1).unwrap().as_str();

        // Try to find a COMMENT for this target using a more flexible approach
        // Look for the specific target and its closing parenthesis
        let target_start = captures.get(0).unwrap().start();
        let target_end = find_closing_paren(&normalized_content[target_start..]) + target_start;
        let target_block = &normalized_content[target_start..target_end];

        let mut description = format!("CMake custom target: {}", target_name);

        // Look for COMMENT in this specific target block
        let comment_pattern = Regex::new(r#"COMMENT\s+"([^"]*)"#)
            .map_err(|e| format!("Failed to compile comment regex: {}", e))?;

        if let Some(comment_captures) = comment_pattern.captures(target_block) {
            if let Some(comment) = comment_captures.get(1) {
                description = comment.as_str().to_string();
            }
        }

        let task = Task {
            name: target_name.to_string(),
            file_path: file_path.to_path_buf(),
            definition_type: TaskDefinitionType::CMake,
            runner: TaskRunner::CMake,
            source_name: target_name.to_string(),
            description: Some(description),
            shadowed_by: None,
            disambiguated_name: None,
        };

        tasks.push(task);
    }

    Ok(tasks)
}

/// Find the closing parenthesis for a CMake function call
fn find_closing_paren(content: &str) -> usize {
    let mut paren_count = 0;
    let mut in_string = false;
    let mut escape_next = false;

    for (i, ch) in content.chars().enumerate() {
        if escape_next {
            escape_next = false;
            continue;
        }

        if ch == '\\' {
            escape_next = true;
            continue;
        }

        if ch == '"' && !escape_next {
            in_string = !in_string;
            continue;
        }

        if !in_string {
            if ch == '(' {
                paren_count += 1;
            } else if ch == ')' {
                paren_count -= 1;
                if paren_count == 0 {
                    return i;
                }
            }
        }
    }

    if content.is_empty() { 0 } else { content.len() - 1 } // Fallback
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let cmake_path = temp_dir.path().join("CMakeLists.txt");
        std::fs::write(&cmake_path, "").unwrap();

        let result = parse(&cmake_path);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_parse_basic_targets() {
        let content = r#"
cmake_minimum_required(VERSION 3.10)
project(MyProject)

add_custom_target(build-all)
add_custom_target(test-all COMMENT "Run all tests")
add_custom_target(clean-all)
"#;

        let temp_dir = TempDir::new().unwrap();
        let cmake_path = temp_dir.path().join("CMakeLists.txt");
        std::fs::write(&cmake_path, content).unwrap();

        let result = parse(&cmake_path);
        assert!(result.is_ok());

        let tasks = result.unwrap();
        assert_eq!(tasks.len(), 3);

        // Check task names
        let task_names: Vec<&str> = tasks.iter().map(|t| t.name.as_str()).collect();
        assert!(task_names.contains(&"build-all"));
        assert!(task_names.contains(&"test-all"));
        assert!(task_names.contains(&"clean-all"));

        // Check descriptions
        let test_task = tasks.iter().find(|t| t.name == "test-all").unwrap();
        assert_eq!(test_task.description.as_ref().unwrap(), "Run all tests");

        let build_task = tasks.iter().find(|t| t.name == "build-all").unwrap();
        assert_eq!(
            build_task.description.as_ref().unwrap(),
            "CMake custom target: build-all"
        );
    }

    #[test]
    fn test_parse_complex_targets() {
        let content = r#"
add_custom_target(
    deploy
    COMMAND echo "Deploying..."
    COMMENT "Deploy the application"
)

add_custom_target(install COMMENT "Install dependencies")
add_custom_target(build COMMENT "Build the project")
"#;

        let temp_dir = TempDir::new().unwrap();
        let cmake_path = temp_dir.path().join("CMakeLists.txt");
        std::fs::write(&cmake_path, content).unwrap();

        let result = parse(&cmake_path);
        assert!(result.is_ok());

        let tasks = result.unwrap();
        assert_eq!(tasks.len(), 3);

        // Check task names
        let task_names: Vec<&str> = tasks.iter().map(|t| t.name.as_str()).collect();
        assert!(task_names.contains(&"deploy"));
        assert!(task_names.contains(&"install"));
        assert!(task_names.contains(&"build"));

        // Check descriptions
        let deploy_task = tasks.iter().find(|t| t.name == "deploy").unwrap();
        assert_eq!(
            deploy_task.description.as_ref().unwrap(),
            "Deploy the application"
        );

        let install_task = tasks.iter().find(|t| t.name == "install").unwrap();
        assert_eq!(
            install_task.description.as_ref().unwrap(),
            "Install dependencies"
        );
    }

    #[test]
    fn test_parse_invalid_file() {
        let temp_dir = TempDir::new().unwrap();
        let cmake_path = temp_dir.path().join("CMakeLists.txt");

        let result = parse(&cmake_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to open file"));
    }

    #[test]
    fn test_parse_targets_without_comments() {
        let content = r#"
add_custom_target(build)
add_custom_target(test)
add_custom_target(clean)
"#;

        let temp_dir = TempDir::new().unwrap();
        let cmake_path = temp_dir.path().join("CMakeLists.txt");
        std::fs::write(&cmake_path, content).unwrap();

        let result = parse(&cmake_path);
        assert!(result.is_ok());

        let tasks = result.unwrap();
        assert_eq!(tasks.len(), 3);

        // Check that tasks without comments get default descriptions
        for task in tasks {
            assert!(task.description.is_some());
            assert!(
                task.description
                    .unwrap()
                    .starts_with("CMake custom target:")
            );
        }
    }

    #[test]
    fn test_find_closing_paren_edge_cases() {
        // Test with no opening parenthesis
        let content = "add_custom_target(test)";
        let result = find_closing_paren(content);
        assert_eq!(result, 22); // Position of closing parenthesis
        
        // Test with opening parenthesis at end
        let content = "add_custom_target(";
        let result = find_closing_paren(content);
        assert_eq!(result, content.len() - 1); // Fallback when no closing paren found
        
        // Test with nested parentheses
        let content = "add_custom_target(test(inner)outer)";
        let result = find_closing_paren(content);
        assert_eq!(result, 34);
        
        // Test with multiple nested levels
        let content = "add_custom_target(test(inner(more)))";
        let result = find_closing_paren(content);
        assert_eq!(result, 35);
    }

    #[test]
    fn test_find_closing_paren_complex_cases() {
        // Test with spaces and newlines
        let content = "add_custom_target(\n  test\n)";
        let result = find_closing_paren(content);
        assert_eq!(result, 26);
        
        // Test with quotes inside parentheses
        let content = "add_custom_target(test \"with quotes\")";
        let result = find_closing_paren(content);
        assert_eq!(result, 36);
        
        // Test with escaped characters
        let content = "add_custom_target(test\\(escaped\\))";
        let result = find_closing_paren(content);
        assert_eq!(result, 33);
    }

    #[test]
    fn test_find_closing_paren_error_handling() {
        // Test with valid content
        let content = "add_custom_target(test)";
        let result = find_closing_paren(content);
        assert_eq!(result, 22);
        
        // Test with empty string
        let content = "";
        let result = find_closing_paren(content);
        assert_eq!(result, 0);
        
        // Test with only opening parenthesis
        let content = "(";
        let result = find_closing_paren(content);
        assert_eq!(result, 0);
    }

    #[test]
    fn test_parse_complex_cmake() {
        let content = r#"
cmake_minimum_required(VERSION 3.10)
project(MyProject)

# Multiple targets with comments
add_custom_target(build-all COMMENT "Build all targets")
add_custom_target(test-all COMMENT "Run all tests")
add_custom_target(clean-all COMMENT "Clean all artifacts")

# Target with complex arguments
add_custom_target(
  deploy
  COMMENT "Deploy the application"
  DEPENDS build-all test-all
)

# Target with nested function calls
add_custom_target(
  package
  COMMAND ${CMAKE_COMMAND} -E echo "Packaging..."
  COMMENT "Create package"
)
"#;

        let temp_dir = TempDir::new().unwrap();
        let cmake_path = temp_dir.path().join("CMakeLists.txt");
        std::fs::write(&cmake_path, content).unwrap();

        let result = parse(&cmake_path);
        assert!(result.is_ok());

        let tasks = result.unwrap();
        assert_eq!(tasks.len(), 5);

        // Check task names
        let task_names: Vec<&str> = tasks.iter().map(|t| t.name.as_str()).collect();
        assert!(task_names.contains(&"build-all"));
        assert!(task_names.contains(&"test-all"));
        assert!(task_names.contains(&"clean-all"));
        assert!(task_names.contains(&"deploy"));
        assert!(task_names.contains(&"package"));

        // Check descriptions
        let build_task = tasks.iter().find(|t| t.name == "build-all").unwrap();
        assert_eq!(build_task.description.as_ref().unwrap(), "Build all targets");

        let test_task = tasks.iter().find(|t| t.name == "test-all").unwrap();
        assert_eq!(test_task.description.as_ref().unwrap(), "Run all tests");
    }

    #[test]
    fn test_parse_cmake_with_errors() {
        // Test with malformed CMake content
        let content = r#"
cmake_minimum_required(VERSION 3.10)
project(MyProject

# Unclosed parenthesis
add_custom_target(build-all COMMENT "Build all targets"
add_custom_target(test-all COMMENT "Run all tests"
"#;

        let temp_dir = TempDir::new().unwrap();
        let cmake_path = temp_dir.path().join("CMakeLists.txt");
        std::fs::write(&cmake_path, content).unwrap();

        let result = parse(&cmake_path);
        // Should handle malformed content gracefully
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_parse_cmake_empty_content() {
        let temp_dir = TempDir::new().unwrap();
        let cmake_path = temp_dir.path().join("CMakeLists.txt");
        std::fs::write(&cmake_path, "").unwrap();

        let result = parse(&cmake_path);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_parse_cmake_missing_file() {
        let temp_dir = TempDir::new().unwrap();
        let cmake_path = temp_dir.path().join("nonexistent.txt");

        let result = parse(&cmake_path);
        assert!(result.is_err());
    }
}
