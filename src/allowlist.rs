use std::fs;
use std::path::{Path, PathBuf};
use crate::types::{Allowlist, AllowlistEntry, AllowScope, Task, TaskRunner};
use crate::prompt::{self, AllowDecision};

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
    
    // Create .dela directory if it doesn't exist
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create .dela directory: {}", e))?;
    }

    let toml = toml::to_string_pretty(&allowlist).map_err(|e| format!("Failed to serialize allowlist: {}", e))?;
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
    // 1. Load the allowlist from disk
    let mut allowlist = load_allowlist()?;

    // 2. Check each entry to see if it matches
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

    // 3. If no matching entry found, prompt the user
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::env;
    use std::fs;
    use serial_test::serial;

    fn setup_test_env() -> (TempDir, Task) {
        let temp_dir = TempDir::new().unwrap();
        env::set_var("HOME", temp_dir.path());

        // Create ~/.dela directory
        fs::create_dir_all(temp_dir.path().join(".dela"))
            .expect("Failed to create .dela directory");

        let task = Task {
            name: "test-task".to_string(),
            description: Some("A test task".to_string()),
            file_path: PathBuf::from("Makefile"),
            runner: TaskRunner::Make,
            source_name: "test-task".to_string(),
        };

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
}