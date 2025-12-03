use crate::allowlist::load_allowlist;
use crate::types::{AllowScope, Allowlist, Task};
use std::path::Path;

/// MCP-specific allowlist evaluator for task permissions
///
/// This module provides read-only access to the allowlist for MCP operations.
/// It follows the same precedence rules as the main allowlist but is designed
/// specifically for MCP server operations without prompting capabilities.
#[derive(Clone)]
pub struct McpAllowlistEvaluator {
    pub allowlist: Allowlist,
}

impl McpAllowlistEvaluator {
    /// Create a new MCP allowlist evaluator by loading the allowlist from disk
    pub fn new() -> Result<Self, String> {
        let allowlist = load_allowlist()?;
        Ok(Self { allowlist })
    }

    /// Check if a task is allowed for MCP execution
    ///
    /// Returns:
    /// - `Ok(true)` if the task is explicitly allowed
    /// - `Ok(false)` if the task is explicitly denied or not found in allowlist
    /// - `Err(msg)` if there was an error loading the allowlist
    ///
    /// Precedence order (highest to lowest):
    /// 1. Deny entries (highest precedence)
    /// 2. Directory scope allow entries
    /// 3. File scope allow entries  
    /// 4. Task scope allow entries
    /// 5. Not found in allowlist (deny by default for MCP)
    pub fn is_task_allowed(&self, task: &Task) -> Result<bool, String> {
        // First pass: Check for deny entries (highest precedence)
        for entry in &self.allowlist.entries {
            if let AllowScope::Deny = entry.scope {
                if self.path_matches(&task.file_path, &entry.path, true) {
                    return Ok(false);
                }
            }
        }

        // Second pass: Check for allow entries
        for entry in &self.allowlist.entries {
            match entry.scope {
                AllowScope::Directory => {
                    if self.path_matches(&task.file_path, &entry.path, true) {
                        return Ok(true);
                    }
                }
                AllowScope::File => {
                    if self.path_matches(&task.file_path, &entry.path, false) {
                        return Ok(true);
                    }
                }
                AllowScope::Task => {
                    if self.path_matches(&task.file_path, &entry.path, false) {
                        if let Some(ref tasks) = entry.tasks {
                            if tasks.contains(&task.name) {
                                return Ok(true);
                            }
                        }
                    }
                }
                AllowScope::Deny | AllowScope::Once => {
                    // Already handled deny in first pass, skip Once (not applicable for MCP)
                    continue;
                }
            }
        }

        // If no matching entry found, deny by default for MCP
        Ok(false)
    }

    /// Check if two paths match, considering directory scope
    fn path_matches(&self, task_path: &Path, allowlist_path: &Path, allow_subdirs: bool) -> bool {
        if allow_subdirs {
            task_path.starts_with(allowlist_path)
        } else {
            task_path == allowlist_path
        }
    }

    /// Get the number of entries in the allowlist
    #[allow(dead_code)]
    pub fn entry_count(&self) -> usize {
        self.allowlist.entries.len()
    }

    /// Check if the allowlist is empty
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.allowlist.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{TestEnvironment, reset_to_real_environment, set_test_environment};
    use crate::types::{Allowlist, AllowlistEntry};
    use crate::types::{Task, TaskDefinitionType, TaskRunner};
    use serial_test::serial;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_task(name: &str, file_path: std::path::PathBuf) -> Task {
        Task {
            name: name.to_string(),
            file_path,
            definition_type: TaskDefinitionType::Makefile,
            runner: TaskRunner::Make,
            source_name: name.to_string(),
            description: None,
            shadowed_by: None,
            disambiguated_name: None,
        }
    }

    fn setup_test_env() -> (TempDir, Task) {
        let temp_dir = TempDir::new().unwrap();

        // Set up test environment with the temp directory as HOME
        let test_env = TestEnvironment::new().with_home(temp_dir.path().to_string_lossy());
        set_test_environment(test_env);

        // Create ~/.dela directory
        fs::create_dir_all(temp_dir.path().join(".dela"))
            .expect("Failed to create .dela directory");

        let task = create_test_task("test-task", std::path::PathBuf::from("Makefile"));

        (temp_dir, task)
    }

    #[test]
    #[serial]
    fn test_mcp_allowlist_evaluator_new() {
        let (_temp_dir, _task) = setup_test_env();

        let evaluator = McpAllowlistEvaluator::new().unwrap();
        assert!(evaluator.is_empty());
        assert_eq!(evaluator.entry_count(), 0);

        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_mcp_allowlist_evaluator_empty_deny_by_default() {
        let (_temp_dir, task) = setup_test_env();

        let evaluator = McpAllowlistEvaluator::new().unwrap();

        // With empty allowlist, task should be denied by default for MCP
        assert_eq!(evaluator.is_task_allowed(&task).unwrap(), false);

        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_mcp_allowlist_evaluator_file_scope_allow() {
        let (temp_dir, task) = setup_test_env();

        // Create allowlist with file scope
        let mut allowlist = Allowlist::default();
        let entry = AllowlistEntry {
            path: std::path::PathBuf::from("Makefile"),
            scope: AllowScope::File,
            tasks: None,
        };
        allowlist.entries.push(entry);
        crate::allowlist::save_allowlist(&allowlist).unwrap();

        let evaluator = McpAllowlistEvaluator::new().unwrap();

        // Task should be allowed
        assert_eq!(evaluator.is_task_allowed(&task).unwrap(), true);

        drop(temp_dir);
        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_mcp_allowlist_evaluator_task_scope_allow() {
        let (temp_dir, task) = setup_test_env();

        // Create allowlist with task scope for specific task
        let mut allowlist = Allowlist::default();
        let entry = AllowlistEntry {
            path: std::path::PathBuf::from("Makefile"),
            scope: AllowScope::Task,
            tasks: Some(vec!["test-task".to_string()]),
        };
        allowlist.entries.push(entry);
        crate::allowlist::save_allowlist(&allowlist).unwrap();

        let evaluator = McpAllowlistEvaluator::new().unwrap();

        // Task should be allowed
        assert_eq!(evaluator.is_task_allowed(&task).unwrap(), true);

        // Create a different task that should be denied
        let other_task = create_test_task("other-task", std::path::PathBuf::from("Makefile"));
        assert_eq!(evaluator.is_task_allowed(&other_task).unwrap(), false);

        drop(temp_dir);
        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_mcp_allowlist_evaluator_directory_scope_allow() {
        let (temp_dir, _task) = setup_test_env();

        // Create allowlist with directory scope
        let mut allowlist = Allowlist::default();
        let entry = AllowlistEntry {
            path: std::path::PathBuf::from("/project"),
            scope: AllowScope::Directory,
            tasks: None,
        };
        allowlist.entries.push(entry);
        crate::allowlist::save_allowlist(&allowlist).unwrap();

        let evaluator = McpAllowlistEvaluator::new().unwrap();

        // Task in subdirectory should be allowed
        let subdir_task = create_test_task(
            "build",
            std::path::PathBuf::from("/project/subdir/Makefile"),
        );
        assert_eq!(evaluator.is_task_allowed(&subdir_task).unwrap(), true);

        // Task outside directory should be denied
        let outside_task = create_test_task("build", std::path::PathBuf::from("/other/Makefile"));
        assert_eq!(evaluator.is_task_allowed(&outside_task).unwrap(), false);

        drop(temp_dir);
        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_mcp_allowlist_evaluator_deny_scope() {
        let (temp_dir, task) = setup_test_env();

        // Create allowlist with deny scope
        let mut allowlist = Allowlist::default();
        let entry = AllowlistEntry {
            path: std::path::PathBuf::from("Makefile"),
            scope: AllowScope::Deny,
            tasks: None,
        };
        allowlist.entries.push(entry);
        crate::allowlist::save_allowlist(&allowlist).unwrap();

        let evaluator = McpAllowlistEvaluator::new().unwrap();

        // Task should be denied
        assert_eq!(evaluator.is_task_allowed(&task).unwrap(), false);

        drop(temp_dir);
        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_mcp_allowlist_evaluator_precedence() {
        let (temp_dir, task) = setup_test_env();

        // Create allowlist with both allow and deny entries
        // Deny should take precedence when both match
        let mut allowlist = Allowlist::default();

        // First add an allow entry
        let allow_entry = AllowlistEntry {
            path: std::path::PathBuf::from("Makefile"),
            scope: AllowScope::File,
            tasks: None,
        };
        allowlist.entries.push(allow_entry);

        // Then add a deny entry for the same path
        let deny_entry = AllowlistEntry {
            path: std::path::PathBuf::from("Makefile"),
            scope: AllowScope::Deny,
            tasks: None,
        };
        allowlist.entries.push(deny_entry);

        crate::allowlist::save_allowlist(&allowlist).unwrap();

        let evaluator = McpAllowlistEvaluator::new().unwrap();

        // Task should be denied (deny takes precedence)
        assert_eq!(evaluator.is_task_allowed(&task).unwrap(), false);

        drop(temp_dir);
        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_mcp_allowlist_evaluator_uninitialized() {
        // Set up environment without .dela directory
        let temp_dir = TempDir::new().unwrap();
        let test_env = TestEnvironment::new().with_home(temp_dir.path().to_string_lossy());
        set_test_environment(test_env);

        // Should return error when dela is not initialized
        assert!(McpAllowlistEvaluator::new().is_err());

        drop(temp_dir);
        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_mcp_allowlist_evaluator_multiple_entries() {
        let (temp_dir, task) = setup_test_env();

        // Create allowlist with multiple entries
        let mut allowlist = Allowlist::default();

        // Add a directory allow entry
        let dir_entry = AllowlistEntry {
            path: std::path::PathBuf::from("/project"),
            scope: AllowScope::Directory,
            tasks: None,
        };
        allowlist.entries.push(dir_entry);

        // Add a file deny entry
        let file_deny_entry = AllowlistEntry {
            path: std::path::PathBuf::from("Makefile"),
            scope: AllowScope::Deny,
            tasks: None,
        };
        allowlist.entries.push(file_deny_entry);

        crate::allowlist::save_allowlist(&allowlist).unwrap();

        let evaluator = McpAllowlistEvaluator::new().unwrap();

        // Task should be denied due to file deny entry (higher precedence than directory allow)
        assert_eq!(evaluator.is_task_allowed(&task).unwrap(), false);

        // But a task in a different file in the same directory should be allowed
        let other_file_task =
            create_test_task("build", std::path::PathBuf::from("/project/other.mk"));
        assert_eq!(evaluator.is_task_allowed(&other_file_task).unwrap(), true);

        drop(temp_dir);
        reset_to_real_environment();
    }
}
