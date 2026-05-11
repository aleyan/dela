use crate::parsers::parse_travis_ci;
use crate::task_discovery::support::{
    handle_discovery_error, handle_discovery_success, set_definition,
};
use crate::task_discovery::{DiscoveredTasks, TaskDiscovery};
use crate::types::{TaskDefinitionFile, TaskDefinitionType, TaskFileStatus};
use std::path::Path;

pub(crate) struct TravisCiDiscovery;

impl TaskDiscovery for TravisCiDiscovery {
    fn discover(&self, dir: &Path, discovered: &mut DiscoveredTasks) {
        let _ = discover_travis_ci_tasks(dir, discovered);
    }
}

fn discover_travis_ci_tasks(dir: &Path, discovered: &mut DiscoveredTasks) -> Result<(), String> {
    let travis_ci_path = dir.join(".travis.yml");

    if travis_ci_path.exists() {
        match parse_travis_ci(&travis_ci_path) {
            Ok(tasks) => {
                handle_discovery_success(
                    tasks,
                    travis_ci_path,
                    TaskDefinitionType::TravisCi,
                    discovered,
                );
            }
            Err(error) => {
                handle_discovery_error(
                    error,
                    travis_ci_path,
                    TaskDefinitionType::TravisCi,
                    discovered,
                );
            }
        }
    } else {
        set_definition(
            discovered,
            TaskDefinitionFile {
                path: travis_ci_path,
                definition_type: TaskDefinitionType::TravisCi,
                status: TaskFileStatus::NotFound,
            },
        );
    }

    Ok(())
}
