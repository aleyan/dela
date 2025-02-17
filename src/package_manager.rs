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

/// Detect which package manager to use by checking PATH
pub fn detect_package_manager() -> Option<PackageManager> {
    // Check in order of preference: bun, yarn, npm
    if check_executable("bun") {
        Some(PackageManager::Bun)
    } else if check_executable("yarn") {
        Some(PackageManager::Yarn)
    } else if check_executable("npm") {
        Some(PackageManager::Npm)
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

    #[test]
    fn test_package_manager_run_commands() {
        assert_eq!(PackageManager::Bun.get_run_command("test"), "bun run test");
        assert_eq!(PackageManager::Yarn.get_run_command("test"), "yarn test");
        assert_eq!(PackageManager::Npm.get_run_command("test"), "npm run test");
    }
}
