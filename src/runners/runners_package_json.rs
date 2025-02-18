use crate::task_shadowing::check_path_executable;
use crate::types::TaskRunner;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, PartialEq)]
pub enum PackageManager {
    Bun,
    Yarn,
    Npm,
}

impl PackageManager {
    pub fn command(&self) -> &'static str {
        match self {
            PackageManager::Bun => "bun",
            PackageManager::Yarn => "yarn",
            PackageManager::Npm => "npm",
        }
    }

    pub fn get_run_command(&self, script_name: &str) -> String {
        match self {
            PackageManager::Bun => format!("bun run {}", script_name),
            PackageManager::Yarn => format!("yarn {}", script_name),
            PackageManager::Npm => format!("npm run {}", script_name),
        }
    }
}

/// Detect which Node.js package manager to use based on lock files and available commands
pub fn detect_package_manager(dir: &Path) -> Option<TaskRunner> {
    // First check for lock files
    if dir.join("package-lock.json").exists() && check_path_executable("npm").is_some() {
        return Some(TaskRunner::NodeNpm);
    }
    if dir.join("bun.lockb").exists() && check_path_executable("bun").is_some() {
        return Some(TaskRunner::NodeBun);
    }
    if dir.join("pnpm-lock.yaml").exists() && check_path_executable("pnpm").is_some() {
        return Some(TaskRunner::NodePnpm);
    }
    if dir.join("yarn.lock").exists() && check_path_executable("yarn").is_some() {
        return Some(TaskRunner::NodeYarn);
    }

    // If no lock file, check for available package managers in preferred order
    if check_path_executable("npm").is_some() {
        Some(TaskRunner::NodeNpm)
    } else if check_path_executable("bun").is_some() {
        Some(TaskRunner::NodeBun)
    } else if check_path_executable("pnpm").is_some() {
        Some(TaskRunner::NodePnpm)
    } else if check_path_executable("yarn").is_some() {
        Some(TaskRunner::NodeYarn)
    } else {
        None
    }
}

/// Check if an executable exists in PATH
fn check_executable(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
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
