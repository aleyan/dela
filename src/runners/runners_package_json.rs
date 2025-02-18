use crate::task_shadowing::check_path_executable;
use crate::types::TaskRunner;
use std::path::Path;

/// Detect which Node.js package manager to use based on lock files and available commands
pub fn detect_package_manager(dir: &Path) -> Option<TaskRunner> {
    // First check for available package managers
    let has_npm = check_path_executable("npm").is_some();
    let has_bun = check_path_executable("bun").is_some();
    let has_pnpm = check_path_executable("pnpm").is_some();
    let has_yarn = check_path_executable("yarn").is_some();

    // If only one package manager is available, use it
    let available_count = [has_npm, has_bun, has_pnpm, has_yarn]
        .iter()
        .filter(|&&x| x)
        .count();
    if available_count == 1 {
        if has_npm {
            return Some(TaskRunner::NodeNpm);
        }
        if has_bun {
            return Some(TaskRunner::NodeBun);
        }
        if has_pnpm {
            return Some(TaskRunner::NodePnpm);
        }
        if has_yarn {
            return Some(TaskRunner::NodeYarn);
        }
    }

    // If multiple package managers are available, use lock files to disambiguate
    if dir.join("package-lock.json").exists() && has_npm {
        return Some(TaskRunner::NodeNpm);
    }
    if dir.join("bun.lockb").exists() && has_bun {
        return Some(TaskRunner::NodeBun);
    }
    if dir.join("pnpm-lock.yaml").exists() && has_pnpm {
        return Some(TaskRunner::NodePnpm);
    }
    if dir.join("yarn.lock").exists() && has_yarn {
        return Some(TaskRunner::NodeYarn);
    }

    // If no lock file but multiple package managers, use preferred order
    if has_npm {
        Some(TaskRunner::NodeNpm)
    } else if has_bun {
        Some(TaskRunner::NodeBun)
    } else if has_pnpm {
        Some(TaskRunner::NodePnpm)
    } else if has_yarn {
        Some(TaskRunner::NodeYarn)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::TempDir;

    fn create_lock_file(dir: &Path, filename: &str) {
        File::create(dir.join(filename)).unwrap();
    }

    #[test]
    fn test_detect_package_manager_with_lock_files() {
        let temp_dir = TempDir::new().unwrap();

        // Test bun.lockb
        create_lock_file(temp_dir.path(), "bun.lockb");
        if check_path_executable("bun").is_some() {
            assert_eq!(
                detect_package_manager(temp_dir.path()),
                Some(TaskRunner::NodeBun)
            );
        }

        // Test pnpm-lock.yaml
        std::fs::remove_file(temp_dir.path().join("bun.lockb")).unwrap();
        create_lock_file(temp_dir.path(), "pnpm-lock.yaml");
        if check_path_executable("pnpm").is_some() {
            assert_eq!(
                detect_package_manager(temp_dir.path()),
                Some(TaskRunner::NodePnpm)
            );
        }

        // Test yarn.lock
        std::fs::remove_file(temp_dir.path().join("pnpm-lock.yaml")).unwrap();
        create_lock_file(temp_dir.path(), "yarn.lock");
        if check_path_executable("yarn").is_some() {
            assert_eq!(
                detect_package_manager(temp_dir.path()),
                Some(TaskRunner::NodeYarn)
            );
        }

        // Test package-lock.json
        std::fs::remove_file(temp_dir.path().join("yarn.lock")).unwrap();
        create_lock_file(temp_dir.path(), "package-lock.json");
        if check_path_executable("npm").is_some() {
            assert_eq!(
                detect_package_manager(temp_dir.path()),
                Some(TaskRunner::NodeNpm)
            );
        }
    }

    #[test]
    fn test_detect_package_manager_no_lock_files() {
        let temp_dir = TempDir::new().unwrap();

        let result = detect_package_manager(temp_dir.path());
        // Result depends on which package managers are installed
        if let Some(runner) = result {
            assert!(matches!(
                runner,
                TaskRunner::NodeBun
                    | TaskRunner::NodePnpm
                    | TaskRunner::NodeYarn
                    | TaskRunner::NodeNpm
            ));
        }
    }
}
