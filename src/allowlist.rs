use std::fs;
use std::path::{Path, PathBuf};
use std::io::Write;
use crate::types::{Allowlist, AllowlistEntry, AllowScope, Task};
use serde::{Serialize, Deserialize};

/// Returns the path to ~/.dela/allowlist.toml
fn allowlist_path() -> Result<PathBuf, String> {
    let home = std::env::var("HOME")
        .map_err(|_| "HOME environment variable not set".to_string())?;
    Ok(PathBuf::from(home).join(".dela").join("allowlist.toml"))
}

/// Load the allowlist from ~/.dela/allowlist.toml.
/// If the file does not exist, return an empty allowlist.
pub fn load_allowlist() -> Result<Allowlist, String> {
    let path = allowlist_path()?;
    if !path.exists() {
        return Ok(Allowlist::default());
    }

    let contents = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read allowlist file: {}", e))?;

    match toml::from_str::<Allowlist>(&contents) {
        Ok(allowlist) => Ok(allowlist),
        Err(e) => Err(format!("Failed to parse allowlist TOML: {}", e)),
    }
}

/// Save the allowlist to ~/.dela/allowlist.toml
pub fn save_allowlist(allowlist: &Allowlist) -> Result<(), String> {
    let path = allowlist_path()?;

    // Ensure ~/.dela directory exists
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory {}: {}", parent.display(), e))?;
        }
    }

    let toml_str = toml::to_string_pretty(allowlist)
        .map_err(|e| format!("Failed to serialize allowlist: {}", e))?;

    let mut file = fs::File::create(&path)
        .map_err(|e| format!("Failed to create allowlist file: {}", e))?;

    file.write_all(toml_str.as_bytes())
        .map_err(|e| format!("Failed to write allowlist file: {}", e))?;

    Ok(())
}

/// Check if a given task is allowed, based on the loaded allowlist
/// This function can be extended to handle interactive prompting later.
pub fn is_task_allowed(task: &Task) -> Result<bool, String> {
    // 1. Load the allowlist from disk
    let allowlist = load_allowlist()?;

    // 2. Check each entry to see if it matches
    for entry in &allowlist.entries {
        match entry.scope {
            AllowScope::Deny => {
                // If the user has specifically denied the file or directory
                // check if task's file is within that path
                if path_matches(&task.file_path, &entry.path, true) {
                    return Ok(false);
                }
            }
            AllowScope::Directory => {
                // If the user allowed an entire directory
                if path_matches(&task.file_path, &entry.path, true) {
                    return Ok(true);
                }
            }
            AllowScope::File => {
                // If the user allowed all tasks in a specific file
                if path_matches(&task.file_path, &entry.path, false) {
                    return Ok(true);
                }
            }
            AllowScope::Task => {
                // If the user allowed a specific set of tasks from a file
                if path_matches(&task.file_path, &entry.path, false) {
                    if let Some(ref tasks) = entry.tasks {
                        // Check if the requested task name is in tasks
                        if tasks.contains(&task.name) {
                            return Ok(true);
                        }
                    }
                }
            }
            AllowScope::Once => {
                // Once is ephemeral. In a real application, you might store it in memory but not in TOML
                // For demonstration, we won't handle ephemeral here.
            }
        }
    }

    // If no entry matched in an allow sense, default to not allowed
    Ok(false)
}

/// Utility function to see if a file path is contained by
/// or matches some reference path.
/// If `check_subdir` is true, then we allow subdirectory matching.
fn path_matches(file_path: &Path, reference: &Path, check_subdir: bool) -> bool {
    if check_subdir {
        // e.g. if reference is /home/user/projects, allow /home/user/projects/foo/Makefile
        file_path.starts_with(reference)
    } else {
        // exact file match
        file_path == reference
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Task, TaskRunner};
    use std::fs;
    use tempfile::TempDir;
    use serial_test::serial;
    use std::path::PathBuf;

    #[test]
    #[serial]
    fn test_load_empty_allowlist() {
        // If no file, we get an empty allowlist
        let temp = TempDir::new().unwrap();
        std::env::set_var("HOME", temp.path());
        
        let loaded = load_allowlist().unwrap();
        assert_eq!(loaded.entries.len(), 0);
    }

    #[test]
    #[serial]
    fn test_save_and_load_allowlist() {
        let temp = TempDir::new().unwrap();
        std::env::set_var("HOME", temp.path());

        let mut allowlist = Allowlist::default();
        allowlist.entries.push(AllowlistEntry {
            path: PathBuf::from("/tmp/test_project/Makefile"),
            scope: AllowScope::File,
            tasks: None,
        });

        // Save
        save_allowlist(&allowlist).unwrap();

        // Load
        let loaded = load_allowlist().unwrap();
        assert_eq!(loaded.entries.len(), 1);
        let entry = &loaded.entries[0];
        assert_eq!(entry.path, PathBuf::from("/tmp/test_project/Makefile"));
        assert_eq!(entry.scope, AllowScope::File);
    }

    #[test]
    #[serial]
    fn test_is_task_allowed_file_scope() {
        let temp = TempDir::new().unwrap();
        std::env::set_var("HOME", temp.path());

        // create an allowlist
        let mut allowlist = Allowlist::default();
        allowlist.entries.push(AllowlistEntry {
            path: PathBuf::from("/some/project/Makefile"),
            scope: AllowScope::File,
            tasks: None,
        });
        save_allowlist(&allowlist).unwrap();

        // define a task from that Makefile
        let task = Task {
            name: "build".to_string(),
            file_path: PathBuf::from("/some/project/Makefile"),
            runner: TaskRunner::Make,
            source_name: "build".to_string(),
            description: None,
        };

        let allowed = is_task_allowed(&task).unwrap();
        assert!(allowed, "Task should be allowed by file scope");
    }

    #[test]
    #[serial]
    fn test_is_task_allowed_directory_scope() {
        let temp = TempDir::new().unwrap();
        std::env::set_var("HOME", temp.path());

        let mut allowlist = Allowlist::default();
        allowlist.entries.push(AllowlistEntry {
            path: PathBuf::from("/some/project"),
            scope: AllowScope::Directory,
            tasks: None,
        });
        save_allowlist(&allowlist).unwrap();

        let task = Task {
            name: "build".to_string(),
            file_path: PathBuf::from("/some/project/subdir/Makefile"),
            runner: TaskRunner::Make,
            source_name: "build".to_string(),
            description: None,
        };

        let allowed = is_task_allowed(&task).unwrap();
        assert!(allowed, "Task should be allowed by directory scope");
    }

    #[test]
    #[serial]
    fn test_is_task_allowed_task_scope() {
        let temp = TempDir::new().unwrap();
        std::env::set_var("HOME", temp.path());

        let mut allowlist = Allowlist::default();
        allowlist.entries.push(AllowlistEntry {
            path: PathBuf::from("/some/project/Makefile"),
            scope: AllowScope::Task,
            tasks: Some(vec!["test".to_string()]),
        });
        save_allowlist(&allowlist).unwrap();

        let test_task = Task {
            name: "test".to_string(),
            file_path: PathBuf::from("/some/project/Makefile"),
            runner: TaskRunner::Make,
            source_name: "test".to_string(),
            description: None,
        };

        let build_task = Task {
            name: "build".to_string(),
            file_path: PathBuf::from("/some/project/Makefile"),
            runner: TaskRunner::Make,
            source_name: "build".to_string(),
            description: None,
        };

        // 'test' should be allowed, 'build' not
        assert!(is_task_allowed(&test_task).unwrap());
        assert!(!is_task_allowed(&build_task).unwrap());
    }

    #[test]
    #[serial]
    fn test_is_task_denied_scope() {
        let temp = TempDir::new().unwrap();
        std::env::set_var("HOME", temp.path());

        let mut allowlist = Allowlist::default();
        allowlist.entries.push(AllowlistEntry {
            path: PathBuf::from("/some/project/Makefile"),
            scope: AllowScope::Deny,
            tasks: None,
        });
        save_allowlist(&allowlist).unwrap();

        let any_task = Task {
            name: "some".to_string(),
            file_path: PathBuf::from("/some/project/Makefile"),
            runner: TaskRunner::Make,
            source_name: "some".to_string(),
            description: None,
        };

        let allowed = is_task_allowed(&any_task).unwrap();
        assert!(!allowed, "All tasks should be denied for that file");
    }
}