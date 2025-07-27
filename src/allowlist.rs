use crate::prompt::{self, AllowDecision};
use crate::types::{AllowScope, Allowlist, AllowlistEntry, Task};
use std::fs;
use std::path::{Path, PathBuf};

/// Returns the path to ~/.dela/allowlist.toml
fn allowlist_path() -> Result<PathBuf, String> {
    let home =
        std::env::var("HOME").map_err(|_| "HOME environment variable not set".to_string())?;
    Ok(PathBuf::from(home).join(".dela").join("allowlist.toml"))
}

/// Load the allowlist from ~/.dela/allowlist.toml.
/// If the file does not exist, return an empty allowlist.
pub fn load_allowlist() -> Result<Allowlist, String> {
    let path = allowlist_path()?;
    let dela_dir = path.parent().ok_or("Invalid allowlist path")?;

    // Check if ~/.dela exists
    if !dela_dir.exists() {
        return Err("Dela is not initialized. Please run 'dela init' first.".to_string());
    }

    // If allowlist file doesn't exist but ~/.dela does, return empty allowlist
    if !path.exists() {
        return Ok(Allowlist::default());
    }

    let contents =
        fs::read_to_string(&path).map_err(|e| format!("Failed to read allowlist file: {}", e))?;

    match toml::from_str::<Allowlist>(&contents) {
        Ok(allowlist) => Ok(allowlist),
        Err(e) => Err(format!("Failed to parse allowlist TOML: {}", e)),
    }
}

/// Save the allowlist to ~/.dela/allowlist.toml
pub fn save_allowlist(allowlist: &Allowlist) -> Result<(), String> {
    let path = allowlist_path()?;

    // Create .dela directory if it doesn't exist
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create .dela directory: {}", e))?;
    }

    let toml = toml::to_string_pretty(&allowlist)
        .map_err(|e| format!("Failed to serialize allowlist: {}", e))?;
    fs::write(&path, toml).map_err(|e| format!("Failed to create allowlist file: {}", e))?;
    Ok(())
}

/// Check if two paths match, considering directory scope
fn path_matches(task_path: &Path, allowlist_path: &Path, allow_subdirs: bool) -> bool {
    if allow_subdirs {
        task_path.starts_with(allowlist_path)
    } else {
        task_path == allowlist_path
    }
}

/// Check if a given task is allowed, based on the loaded allowlist
/// If the task is not in the allowlist, prompt the user for a decision
pub fn check_task_allowed(task: &Task) -> Result<bool, String> {
    // Only proceed with allowlist operations if dela is initialized
    let mut allowlist = load_allowlist()?;

    // Check each entry to see if it matches
    for entry in &allowlist.entries {
        match entry.scope {
            AllowScope::Deny => {
                if path_matches(&task.file_path, &entry.path, true) {
                    return Ok(false);
                }
            }
            AllowScope::Directory => {
                if path_matches(&task.file_path, &entry.path, true) {
                    return Ok(true);
                }
            }
            AllowScope::File => {
                if path_matches(&task.file_path, &entry.path, false) {
                    return Ok(true);
                }
            }
            AllowScope::Task => {
                if path_matches(&task.file_path, &entry.path, false) {
                    if let Some(ref tasks) = entry.tasks {
                        if tasks.contains(&task.name) {
                            return Ok(true);
                        }
                    }
                }
            }
            AllowScope::Once => {
                // Once is ephemeral and not stored in the allowlist
                continue;
            }
        }
    }

    // If no matching entry found, prompt the user
    match prompt::prompt_for_task(task)? {
        AllowDecision::Allow(scope) => {
            match scope {
                AllowScope::Once => {
                    // Don't persist Once decisions
                    Ok(true)
                }
                scope => {
                    // Create a new allowlist entry
                    let mut entry = AllowlistEntry {
                        path: task.file_path.clone(),
                        scope: scope.clone(),
                        tasks: None,
                    };

                    // For Task scope, add the specific task name
                    if scope == AllowScope::Task {
                        entry.tasks = Some(vec![task.name.clone()]);
                    }

                    // Add the entry and save
                    allowlist.entries.push(entry);
                    save_allowlist(&allowlist)?;
                    Ok(true)
                }
            }
        }
        AllowDecision::Deny => {
            // Add a deny entry and save
            let entry = AllowlistEntry {
                path: task.file_path.clone(),
                scope: AllowScope::Deny,
                tasks: None,
            };
            allowlist.entries.push(entry);
            save_allowlist(&allowlist)?;
            Ok(false)
        }
    }
}

/// Check if a given task is allowed with a specific scope, without prompting
pub fn check_task_allowed_with_scope(task: &Task, scope: AllowScope) -> Result<bool, String> {
    // Only proceed with allowlist operations if dela is initialized
    let mut allowlist = load_allowlist()?;

    // Create a new allowlist entry
    let mut entry = AllowlistEntry {
        path: task.file_path.clone(),
        scope: scope.clone(),
        tasks: None,
    };

    // For Task scope, add the specific task name
    if scope == AllowScope::Task {
        entry.tasks = Some(vec![task.name.clone()]);
    }

    // Add the entry and save
    allowlist.entries.push(entry);
    save_allowlist(&allowlist)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Task, TaskDefinitionType, TaskRunner};
    use serial_test::serial;
    use std::env;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_task(name: &str, file_path: PathBuf) -> Task {
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
        unsafe {
            env::set_var("HOME", temp_dir.path());
        }

        // Create ~/.dela directory
        fs::create_dir_all(temp_dir.path().join(".dela"))
            .expect("Failed to create .dela directory");

        let task = create_test_task("test-task", PathBuf::from("Makefile"));

        (temp_dir, task)
    }

    #[test]
    #[serial]
    fn test_empty_allowlist() {
        let (_temp_dir, _task) = setup_test_env();
        let allowlist = load_allowlist().unwrap();
        assert!(allowlist.entries.is_empty());
    }

    #[test]
    #[serial]
    fn test_save_and_load_allowlist() {
        let (temp_dir, _task) = setup_test_env();
        let mut allowlist = Allowlist::default();

        let entry = AllowlistEntry {
            path: PathBuf::from("Makefile"),
            scope: AllowScope::File,
            tasks: None,
        };

        allowlist.entries.push(entry);
        save_allowlist(&allowlist).unwrap();

        // Debug output
        let path = allowlist_path().unwrap();
        println!("Allowlist path: {}", path.display());
        println!("Allowlist exists: {}", path.exists());
        if path.exists() {
            let contents = std::fs::read_to_string(&path).unwrap();
            println!("Allowlist contents: {}", contents);
            let loaded_from_file: Allowlist = toml::from_str(&contents).unwrap();
            println!("Loaded from file: {:?}", loaded_from_file);
        }

        let loaded = load_allowlist().unwrap();
        println!("Loaded from function: {:?}", loaded);
        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(loaded.entries[0].scope, AllowScope::File);

        // Keep temp_dir around until the end of the test
        drop(temp_dir);
    }

    #[test]
    #[serial]
    fn test_path_matches() {
        let base = PathBuf::from("/home/user/project");
        let file = base.join("Makefile");
        let subdir = base.join("subdir").join("Makefile");

        // Exact file match
        assert!(path_matches(&file, &file, false));
        assert!(!path_matches(&subdir, &file, false));

        // Directory match with subdirs
        assert!(path_matches(&file, &base, true));
        assert!(path_matches(&subdir, &base, true));
    }

    #[test]
    #[serial]
    fn test_check_task_allowed_with_scope_edge_cases() {
        let (_temp_dir, task) = setup_test_env();
        
        // Test with Once scope
        let result = check_task_allowed_with_scope(&task, AllowScope::Once);
        assert!(result.is_ok());
        assert!(result.unwrap());
        
        // Test with Task scope
        let result = check_task_allowed_with_scope(&task, AllowScope::Task);
        assert!(result.is_ok());
        assert!(result.unwrap());
        
        // Test with File scope
        let result = check_task_allowed_with_scope(&task, AllowScope::File);
        assert!(result.is_ok());
        assert!(result.unwrap());
        
        // Test with Directory scope
        let result = check_task_allowed_with_scope(&task, AllowScope::Directory);
        assert!(result.is_ok());
        assert!(result.unwrap());
        
        // Verify that the allowlist was updated
        let allowlist = load_allowlist().unwrap();
        assert_eq!(allowlist.entries.len(), 4);
        
        // Check that Task scope has the specific task name
        let task_entry = allowlist.entries.iter().find(|e| e.scope == AllowScope::Task).unwrap();
        assert_eq!(task_entry.tasks, Some(vec!["test-task".to_string()]));
        
        // Check that other scopes don't have specific tasks
        let file_entry = allowlist.entries.iter().find(|e| e.scope == AllowScope::File).unwrap();
        assert_eq!(file_entry.tasks, None);
    }

    #[test]
    #[serial]
    fn test_check_task_allowed_edge_cases() {
        let (_temp_dir, task) = setup_test_env();
        
        // Test with no allowlist entries - this would normally prompt, so we test the logic differently
        // First, verify that the allowlist is empty
        let allowlist = load_allowlist().unwrap();
        assert_eq!(allowlist.entries.len(), 0);
        
        // Test the path matching logic directly instead of calling check_task_allowed
        let task_path = &task.file_path;
        let allowlist_path = &task.file_path;
        assert!(path_matches(task_path, allowlist_path, false));
        
        // Add an entry and test again
        let result = check_task_allowed_with_scope(&task, AllowScope::Task);
        assert!(result.is_ok());
        
        // Now verify the allowlist was updated
        let allowlist = load_allowlist().unwrap();
        assert_eq!(allowlist.entries.len(), 1);
        
        // Test that the entry has the correct structure
        let entry = &allowlist.entries[0];
        assert_eq!(entry.scope, AllowScope::Task);
        assert_eq!(entry.tasks, Some(vec![task.name.clone()]));
    }

    #[test]
    #[serial]
    fn test_path_matches_edge_cases() {
        let task_path = PathBuf::from("/project/Makefile");
        
        // Test exact match
        let allowlist_path = PathBuf::from("/project/Makefile");
        assert!(path_matches(&task_path, &allowlist_path, false));
        
        // Test with subdirs allowed
        let allowlist_path = PathBuf::from("/project");
        assert!(path_matches(&task_path, &allowlist_path, true));
        
        // Test with subdirs not allowed
        assert!(!path_matches(&task_path, &allowlist_path, false));
        
        // Test different paths
        let allowlist_path = PathBuf::from("/different/Makefile");
        assert!(!path_matches(&task_path, &allowlist_path, false));
        assert!(!path_matches(&task_path, &allowlist_path, true));
        
        // Test relative paths
        let task_path = PathBuf::from("Makefile");
        let allowlist_path = PathBuf::from("Makefile");
        assert!(path_matches(&task_path, &allowlist_path, false));
    }

    #[test]
    #[serial]
    fn test_allowlist_scope_comparison() {
        let (_temp_dir, task) = setup_test_env();
        
        // Test scope equality
        assert_eq!(AllowScope::Once, AllowScope::Once);
        assert_eq!(AllowScope::Task, AllowScope::Task);
        assert_eq!(AllowScope::File, AllowScope::File);
        assert_eq!(AllowScope::Directory, AllowScope::Directory);
        
        // Test scope inequality
        assert_ne!(AllowScope::Once, AllowScope::Task);
        assert_ne!(AllowScope::File, AllowScope::Directory);
        
        // Test scope in allowlist entries
        let entry = AllowlistEntry {
            path: task.file_path.clone(),
            scope: AllowScope::Task,
            tasks: Some(vec!["test-task".to_string()]),
        };
        
        assert_eq!(entry.scope, AllowScope::Task);
        assert_eq!(entry.tasks, Some(vec!["test-task".to_string()]));
    }

    #[test]
    #[serial]
    fn test_allowlist_error_handling() {
        // Test with invalid HOME environment
        unsafe {
            env::set_var("HOME", "/nonexistent/path");
        }
        
        // This should fail because the .dela directory doesn't exist
        let result = load_allowlist();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not initialized"));
        
        // Test saving to invalid path - should create the directory
        let allowlist = Allowlist::default();
        let result = save_allowlist(&allowlist);
        assert!(result.is_ok());
        
        // Now loading should work because save_allowlist creates the directory
        let result = load_allowlist();
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    fn test_allowlist_entry_validation() {
        let (_temp_dir, task) = setup_test_env();
        
        // Test valid entry
        let entry = AllowlistEntry {
            path: task.file_path.clone(),
            scope: AllowScope::Task,
            tasks: Some(vec!["test-task".to_string()]),
        };
        
        assert_eq!(entry.path, task.file_path);
        assert_eq!(entry.scope, AllowScope::Task);
        assert_eq!(entry.tasks, Some(vec!["test-task".to_string()]));
        
        // Test entry without specific tasks
        let entry = AllowlistEntry {
            path: task.file_path.clone(),
            scope: AllowScope::File,
            tasks: None,
        };
        
        assert_eq!(entry.scope, AllowScope::File);
        assert_eq!(entry.tasks, None);
    }

    #[test]
    #[serial]
    fn test_allowlist_multiple_entries() {
        let (_temp_dir, task) = setup_test_env();
        
        // Add multiple entries for the same task
        let result1 = check_task_allowed_with_scope(&task, AllowScope::Once);
        assert!(result1.is_ok());
        
        let result2 = check_task_allowed_with_scope(&task, AllowScope::Task);
        assert!(result2.is_ok());
        
        let result3 = check_task_allowed_with_scope(&task, AllowScope::File);
        assert!(result3.is_ok());
        
        // Check that all entries were added
        let allowlist = load_allowlist().unwrap();
        assert_eq!(allowlist.entries.len(), 3);
        
        // Verify the entries have the correct structure
        let once_entry = allowlist.entries.iter().find(|e| e.scope == AllowScope::Once).unwrap();
        let task_entry = allowlist.entries.iter().find(|e| e.scope == AllowScope::Task).unwrap();
        let file_entry = allowlist.entries.iter().find(|e| e.scope == AllowScope::File).unwrap();
        
        assert_eq!(once_entry.scope, AllowScope::Once);
        assert_eq!(task_entry.scope, AllowScope::Task);
        assert_eq!(task_entry.tasks, Some(vec![task.name.clone()]));
        assert_eq!(file_entry.scope, AllowScope::File);
        assert_eq!(file_entry.tasks, None);
    }
}
