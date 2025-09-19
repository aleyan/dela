use rmcp::model::{ErrorCode, ErrorData};
use serde_json::Value;
use std::borrow::Cow;

/// Custom error codes for dela MCP server
/// These extend the standard JSON-RPC error codes with dela-specific errors
#[derive(Debug, Clone, PartialEq)]
pub struct DelaErrorCode(pub i32);

impl DelaErrorCode {
    // Standard JSON-RPC error codes (re-exported for convenience)
    pub const RESOURCE_NOT_FOUND: Self = Self(-32002);
    pub const INVALID_REQUEST: Self = Self(-32600);
    pub const METHOD_NOT_FOUND: Self = Self(-32601);
    pub const INVALID_PARAMS: Self = Self(-32602);
    pub const INTERNAL_ERROR: Self = Self(-32603);
    pub const PARSE_ERROR: Self = Self(-32700);

    // Dela-specific error codes (using range -32000 to -32099 for custom errors)
    pub const NOT_ALLOWLISTED: Self = Self(-32010);
    pub const RUNNER_UNAVAILABLE: Self = Self(-32011);
    pub const TASK_NOT_FOUND: Self = Self(-32012);
}

impl From<DelaErrorCode> for ErrorCode {
    fn from(code: DelaErrorCode) -> Self {
        ErrorCode(code.0)
    }
}

/// Dela-specific error types with helpful messages and hints
#[derive(Debug, Clone, PartialEq)]
pub enum DelaError {
    /// Task is not allowlisted for MCP execution
    NotAllowlisted {
        task_name: String,
        hint: Option<String>,
    },
    /// Required task runner is not available on the system
    RunnerUnavailable {
        runner_name: String,
        task_name: String,
        hint: Option<String>,
    },
    /// Task with the given name was not found
    TaskNotFound {
        task_name: String,
        hint: Option<String>,
    },
    /// Generic internal error
    InternalError {
        message: String,
        hint: Option<String>,
    },
}

impl DelaError {
    /// Convert a DelaError to an ErrorData for MCP responses
    pub fn to_error_data(&self) -> ErrorData {
        match self {
            DelaError::NotAllowlisted { task_name, hint } => ErrorData {
                code: DelaErrorCode::NOT_ALLOWLISTED.into(),
                message: Cow::Owned(format!("Task '{}' is not allowlisted for MCP execution", task_name)),
                data: hint.as_ref().map(|h| Value::String(h.clone())),
            },
            DelaError::RunnerUnavailable { runner_name, task_name, hint } => ErrorData {
                code: DelaErrorCode::RUNNER_UNAVAILABLE.into(),
                message: Cow::Owned(format!("Runner '{}' is not available for task '{}'", runner_name, task_name)),
                data: hint.as_ref().map(|h| Value::String(h.clone())),
            },
            DelaError::TaskNotFound { task_name, hint } => ErrorData {
                code: DelaErrorCode::TASK_NOT_FOUND.into(),
                message: Cow::Owned(format!("Task '{}' not found", task_name)),
                data: hint.as_ref().map(|h| Value::String(h.clone())),
            },
            DelaError::InternalError { message, hint } => ErrorData {
                code: DelaErrorCode::INTERNAL_ERROR.into(),
                message: Cow::Owned(message.clone()),
                data: hint.as_ref().map(|h| Value::String(h.clone())),
            },
        }
    }

    /// Create a NotAllowlisted error with a helpful hint
    pub fn not_allowlisted(task_name: String) -> Self {
        DelaError::NotAllowlisted {
            task_name,
            hint: Some("Ask a human to grant MCP access to this task".to_string()),
        }
    }

    /// Create a RunnerUnavailable error with a helpful hint
    pub fn runner_unavailable(runner_name: String, task_name: String) -> Self {
        let hint = match runner_name.as_str() {
            "make" => Some("Install make: brew install make (macOS) or apt-get install make (Ubuntu)".to_string()),
            "npm" => Some("Install Node.js and npm: https://nodejs.org/".to_string()),
            "gradle" => Some("Install Gradle: https://gradle.org/install/".to_string()),
            "maven" => Some("Install Maven: https://maven.apache.org/install.html".to_string()),
            "python" => Some("Install Python: https://python.org/downloads/".to_string()),
            "uv" => Some("Install uv: pip install uv or https://github.com/astral-sh/uv".to_string()),
            "just" => Some("Install just: https://github.com/casey/just#installation".to_string()),
            _ => Some(format!("Install {} to run this task", runner_name)),
        };

        DelaError::RunnerUnavailable {
            runner_name,
            task_name,
            hint,
        }
    }

    /// Create a TaskNotFound error with a helpful hint
    pub fn task_not_found(task_name: String) -> Self {
        DelaError::TaskNotFound {
            task_name,
            hint: Some("Use 'list_tasks' to see available tasks".to_string()),
        }
    }

    /// Create an InternalError with a helpful hint
    pub fn internal_error(message: String, hint: Option<String>) -> Self {
        DelaError::InternalError { message, hint }
    }
}

impl From<DelaError> for ErrorData {
    fn from(error: DelaError) -> Self {
        error.to_error_data()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_not_allowlisted_error() {
        let error = DelaError::not_allowlisted("build".to_string());
        let error_data = error.to_error_data();
        
        assert_eq!(error_data.code.0, -32010);
        assert!(error_data.message.contains("not allowlisted"));
        assert!(error_data.data.as_ref().unwrap().as_str().unwrap().contains("Ask a human"));
    }

    #[test]
    fn test_runner_unavailable_error() {
        let error = DelaError::runner_unavailable("make".to_string(), "build".to_string());
        let error_data = error.to_error_data();
        
        assert_eq!(error_data.code.0, -32011);
        assert!(error_data.message.contains("Runner 'make' is not available"));
        assert!(error_data.data.as_ref().unwrap().as_str().unwrap().contains("brew install make"));
    }

    #[test]
    fn test_task_not_found_error() {
        let error = DelaError::task_not_found("nonexistent".to_string());
        let error_data = error.to_error_data();
        
        assert_eq!(error_data.code.0, -32012);
        assert!(error_data.message.contains("not found"));
        assert!(error_data.data.as_ref().unwrap().as_str().unwrap().contains("list_tasks"));
    }
}
