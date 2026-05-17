use crate::parsers::parse_pyproject_toml;
use crate::task_discovery::support::{handle_discovery_error, handle_discovery_success};
use crate::task_discovery::{DiscoveredTasks, TaskDiscovery};
use crate::types::{TaskDefinitionFile, TaskDefinitionType, TaskFileStatus};
use std::path::Path;

pub(crate) struct PythonDiscovery;

impl TaskDiscovery for PythonDiscovery {
    fn discover(&self, dir: &Path, discovered: &mut DiscoveredTasks) {
        let _ = discover_python_tasks(dir, discovered);
    }
}

fn discover_python_tasks(dir: &Path, discovered: &mut DiscoveredTasks) -> anyhow::Result<()> {
    let pyproject_toml = dir.join("pyproject.toml");

    if !pyproject_toml.exists() {
        discovered.definitions.pyproject_toml = Some(TaskDefinitionFile {
            path: pyproject_toml,
            definition_type: TaskDefinitionType::PyprojectToml,
            status: TaskFileStatus::NotFound,
        });
        return Ok(());
    }

    match parse_pyproject_toml::parse(&pyproject_toml) {
        Ok(tasks) => {
            handle_discovery_success(
                tasks,
                pyproject_toml,
                TaskDefinitionType::PyprojectToml,
                discovered,
            );
        }
        Err(error) => {
            handle_discovery_error(
                error,
                pyproject_toml,
                TaskDefinitionType::PyprojectToml,
                discovered,
            );
        }
    }

    Ok(())
}
