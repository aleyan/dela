use crate::types::{Task, TaskDefinitionType, TaskRunner};
use std::path::PathBuf;

pub fn parse(path: &PathBuf) -> Result<Vec<Task>, String> {
    let contents =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read turbo.json: {}", e))?;
    let json: serde_json::Value = serde_json::from_str(&contents)
        .map_err(|e| format!("Failed to parse turbo.json: {}", e))?;

    let tasks = json
        .get("tasks")
        .or_else(|| json.get("pipeline"))
        .and_then(|value| value.as_object())
        .map(|task_map| {
            task_map
                .keys()
                .map(|name| Task {
                    name: name.clone(),
                    file_path: path.clone(),
                    definition_type: TaskDefinitionType::TurboJson,
                    runner: TaskRunner::Turbo,
                    source_name: name.clone(),
                    description: None,
                    shadowed_by: None,
                    disambiguated_name: None,
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(tasks)
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
