use crate::parsers::parse_package_json;
use crate::task_discovery::support::{handle_discovery_error, handle_discovery_success};
use crate::task_discovery::{DiscoveredTasks, TaskDiscovery};
use crate::types::{TaskDefinitionFile, TaskDefinitionType, TaskFileStatus};
use std::path::Path;

pub(crate) struct NpmDiscovery;

impl TaskDiscovery for NpmDiscovery {
    fn discover(&self, dir: &Path, discovered: &mut DiscoveredTasks) {
        let _ = discover_npm_tasks(dir, discovered);
    }
}

fn discover_npm_tasks(dir: &Path, discovered: &mut DiscoveredTasks) -> anyhow::Result<()> {
    let package_json = dir.join("package.json");

    if !package_json.exists() {
        discovered.definitions.package_json = Some(TaskDefinitionFile {
            path: package_json,
            definition_type: TaskDefinitionType::PackageJson,
            status: TaskFileStatus::NotFound,
        });
        return Ok(());
    }

    match parse_package_json::parse(&package_json) {
        Ok(tasks) => {
            handle_discovery_success(
                tasks,
                package_json,
                TaskDefinitionType::PackageJson,
                discovered,
            );
        }
        Err(error) => {
            handle_discovery_error(
                error,
                package_json,
                TaskDefinitionType::PackageJson,
                discovered,
            );
        }
    }

    Ok(())
}
