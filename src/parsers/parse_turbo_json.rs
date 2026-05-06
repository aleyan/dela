use crate::types::{Task, TaskDefinitionType, TaskRunner};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurboTaskConfig {
    pub inherits: bool,
    pub declared_locally: bool,
    pub has_local_configuration: bool,
}

impl TurboTaskConfig {
    pub fn is_effective_task_definition(&self) -> bool {
        self.declared_locally && (self.inherits || self.has_local_configuration)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurboConfig {
    pub extends: Vec<String>,
    pub tasks: BTreeMap<String, TurboTaskConfig>,
}

impl TurboConfig {
    pub fn task_names(&self) -> impl Iterator<Item = &String> {
        self.tasks
            .iter()
            .filter(|(_, task)| task.is_effective_task_definition())
            .map(|(name, _)| name)
    }
}

pub fn load_config(path: &Path) -> Result<TurboConfig, String> {
    let contents =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read turbo.json: {}", e))?;
    let json: Value = serde_json::from_str(&contents)
        .map_err(|e| format!("Failed to parse turbo.json: {}", e))?;

    let extends = json
        .get("extends")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    let tasks = if let Some(value) = json.get("tasks") {
        parse_task_map("tasks", value)?
    } else if let Some(value) = json.get("pipeline") {
        parse_task_map("pipeline", value)?
    } else {
        BTreeMap::new()
    };

    Ok(TurboConfig { extends, tasks })
}

pub fn parse(path: &Path) -> Result<Vec<Task>, String> {
    let config = load_config(path)?;

    Ok(config
        .task_names()
        .map(|name| Task {
            name: name.clone(),
            file_path: path.to_path_buf(),
            definition_path: None,
            definition_type: TaskDefinitionType::TurboJson,
            runner: TaskRunner::Turbo,
            source_name: name.clone(),
            description: None,
            shadowed_by: None,
            disambiguated_name: None,
        })
        .collect())
}

fn parse_task_config(value: &Value) -> TurboTaskConfig {
    let Some(object) = value.as_object() else {
        return TurboTaskConfig {
            inherits: true,
            declared_locally: false,
            has_local_configuration: false,
        };
    };

    TurboTaskConfig {
        inherits: object
            .get("extends")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        declared_locally: true,
        has_local_configuration: object.keys().any(|key| key != "extends"),
    }
}

fn parse_task_map(key: &str, value: &Value) -> Result<BTreeMap<String, TurboTaskConfig>, String> {
    let Some(task_map) = value.as_object() else {
        return Err(format!(
            "Failed to parse turbo.json: '{}' must be an object, found {}",
            key,
            json_type_name(value)
        ));
    };

    Ok(task_map
        .iter()
        .map(|(name, value)| (name.clone(), parse_task_config(value)))
        .collect())
}

fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_turbo_json_tasks() {
        let temp_dir = TempDir::new().unwrap();
        let turbo_json_path = temp_dir.path().join("turbo.json");
        std::fs::write(
            &turbo_json_path,
            r#"{
  "$schema": "https://turborepo.dev/schema.json",
  "tasks": {
    "build": {},
    "test": {
      "dependsOn": ["build"]
    }
  }
}"#,
        )
        .unwrap();

        let tasks = parse(&turbo_json_path).unwrap();
        assert_eq!(tasks.len(), 2);
        assert!(
            tasks
                .iter()
                .any(|task| task.name == "build" && task.runner == TaskRunner::Turbo)
        );
        assert!(
            tasks
                .iter()
                .any(|task| task.name == "test" && task.runner == TaskRunner::Turbo)
        );
    }

    #[test]
    fn test_load_turbo_json_tracks_extends_and_task_level_extends_false() {
        let temp_dir = TempDir::new().unwrap();
        let turbo_json_path = temp_dir.path().join("turbo.json");
        std::fs::write(
            &turbo_json_path,
            r#"{
  "extends": ["//", "shared-config"],
  "tasks": {
    "build": {},
    "test": {
      "extends": false
    },
    "lint": {
      "extends": false,
      "outputs": []
    }
  }
}"#,
        )
        .unwrap();

        let config = load_config(&turbo_json_path).unwrap();

        assert_eq!(config.extends, vec!["//", "shared-config"]);
        assert_eq!(
            config.tasks.get("build"),
            Some(&TurboTaskConfig {
                inherits: true,
                declared_locally: true,
                has_local_configuration: false,
            })
        );
        assert_eq!(
            config.tasks.get("test"),
            Some(&TurboTaskConfig {
                inherits: false,
                declared_locally: true,
                has_local_configuration: false,
            })
        );
        assert_eq!(
            config.tasks.get("lint"),
            Some(&TurboTaskConfig {
                inherits: false,
                declared_locally: true,
                has_local_configuration: true,
            })
        );
    }

    #[test]
    fn test_parse_turbo_json_excludes_task_with_only_extends_false() {
        let temp_dir = TempDir::new().unwrap();
        let turbo_json_path = temp_dir.path().join("turbo.json");
        std::fs::write(
            &turbo_json_path,
            r#"{
  "extends": ["//"],
  "tasks": {
    "build": {},
    "test": {
      "extends": false
    },
    "lint": {
      "extends": false,
      "dependsOn": []
    }
  }
}"#,
        )
        .unwrap();

        let tasks = parse(&turbo_json_path).unwrap();
        let task_names: Vec<_> = tasks.iter().map(|task| task.name.as_str()).collect();

        assert_eq!(task_names, vec!["build", "lint"]);
        assert!(!task_names.contains(&"test"));
    }

    #[test]
    fn test_parse_turbo_json_treats_non_object_task_as_inherit_only() {
        let temp_dir = TempDir::new().unwrap();
        let turbo_json_path = temp_dir.path().join("turbo.json");
        std::fs::write(
            &turbo_json_path,
            r#"{
  "tasks": {
    "build": null,
    "test": {}
  }
}"#,
        )
        .unwrap();

        let config = load_config(&turbo_json_path).unwrap();
        assert_eq!(
            config.tasks.get("build"),
            Some(&TurboTaskConfig {
                inherits: true,
                declared_locally: false,
                has_local_configuration: false,
            })
        );

        let tasks = parse(&turbo_json_path).unwrap();
        let task_names: Vec<_> = tasks.iter().map(|task| task.name.as_str()).collect();

        assert_eq!(task_names, vec!["test"]);
        assert!(!task_names.contains(&"build"));
    }

    #[test]
    fn test_parse_turbo_json_legacy_pipeline() {
        let temp_dir = TempDir::new().unwrap();
        let turbo_json_path = temp_dir.path().join("turbo.json");
        std::fs::write(
            &turbo_json_path,
            r#"{
  "pipeline": {
    "lint": {},
    "check-types": {}
  }
}"#,
        )
        .unwrap();

        let tasks = parse(&turbo_json_path).unwrap();
        assert_eq!(tasks.len(), 2);
        assert!(tasks.iter().any(|task| task.name == "lint"));
        assert!(tasks.iter().any(|task| task.name == "check-types"));
    }

    #[test]
    fn test_parse_turbo_json_errors_when_tasks_is_not_an_object() {
        let temp_dir = TempDir::new().unwrap();
        let turbo_json_path = temp_dir.path().join("turbo.json");
        std::fs::write(&turbo_json_path, r#"{"tasks":["build"]}"#).unwrap();

        let err = parse(&turbo_json_path).unwrap_err();

        assert!(err.contains("'tasks' must be an object"));
        assert!(err.contains("array"));
    }

    #[test]
    fn test_parse_turbo_json_errors_when_pipeline_is_not_an_object() {
        let temp_dir = TempDir::new().unwrap();
        let turbo_json_path = temp_dir.path().join("turbo.json");
        std::fs::write(&turbo_json_path, r#"{"pipeline":"build"}"#).unwrap();

        let err = parse(&turbo_json_path).unwrap_err();

        assert!(err.contains("'pipeline' must be an object"));
        assert!(err.contains("string"));
    }

    #[test]
    fn test_parse_turbo_json_without_tasks() {
        let temp_dir = TempDir::new().unwrap();
        let turbo_json_path = temp_dir.path().join("turbo.json");
        std::fs::write(
            &turbo_json_path,
            r#"{"$schema":"https://turborepo.dev/schema.json"}"#,
        )
        .unwrap();

        let tasks = parse(&turbo_json_path).unwrap();
        assert!(tasks.is_empty());
    }

    #[test]
    fn test_parse_turbo_json_malformed_json() {
        let temp_dir = TempDir::new().unwrap();
        let turbo_json_path = temp_dir.path().join("turbo.json");
        std::fs::write(&turbo_json_path, r#"{"tasks":{"build":{}}"#).unwrap();

        let err = parse(&turbo_json_path).unwrap_err();
        assert!(err.contains("Failed to parse turbo.json"));
    }
}
