use once_cell::sync::Lazy;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;

/// Abstraction for environment interactions
pub trait Environment: Send + Sync {
    fn get_shell(&self) -> Option<String>;
    fn check_executable(&self, name: &str) -> Option<String>;
    fn get_home(&self) -> Option<String>;
}

/// Production environment implementation
pub struct RealEnvironment;

impl Environment for RealEnvironment {
    fn get_shell(&self) -> Option<String> {
        std::env::var("SHELL").ok()
    }

    fn check_executable(&self, name: &str) -> Option<String> {
        use std::process::Command;
        let output = Command::new("which").arg(name).output().ok()?;
        if output.status.success() {
            Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            None
        }
    }

    fn get_home(&self) -> Option<String> {
        std::env::var("HOME").ok()
    }
}

/// Test environment implementation
#[derive(Default, Clone)]
pub struct TestEnvironment {
    shell: Option<String>,
    executables: HashSet<String>,
    home: Option<String>,
}

#[cfg(test)]
impl TestEnvironment {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_shell(mut self, shell: impl Into<String>) -> Self {
        self.shell = Some(shell.into());
        self
    }

    pub fn with_executable(mut self, name: impl Into<String>) -> Self {
        self.executables.insert(name.into());
        self
    }

    pub fn with_home(mut self, home: impl Into<String>) -> Self {
        self.home = Some(home.into());
        self
    }
}

impl Environment for TestEnvironment {
    fn get_shell(&self) -> Option<String> {
        self.shell.clone()
    }

    fn check_executable(&self, name: &str) -> Option<String> {
        if self.executables.contains(name) {
            Some(format!("/mock/bin/{}", name))
        } else {
            None
        }
    }

    fn get_home(&self) -> Option<String> {
        self.home.clone()
    }
}

/// Global environment instance
pub static ENVIRONMENT: Lazy<Mutex<Arc<dyn Environment>>> =
    Lazy::new(|| Mutex::new(Arc::new(RealEnvironment)));

/// Helper to set the environment for testing
#[cfg(test)]
pub fn set_test_environment(env: TestEnvironment) {
    *ENVIRONMENT.lock().unwrap() = Arc::new(env);
}

/// Helper to reset to real environment
#[cfg(test)]
pub fn reset_to_real_environment() {
    *ENVIRONMENT.lock().unwrap() = Arc::new(RealEnvironment);
}

/// Helper to get the current environment's HOME value
pub fn get_current_home() -> Option<String> {
    ENVIRONMENT.lock().unwrap().get_home()
}

/// Helper to get the current environment's SHELL value
pub fn get_current_shell() -> Option<String> {
    ENVIRONMENT.lock().unwrap().get_shell()
}
