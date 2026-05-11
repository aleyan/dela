use crate::parsers::parse_gradle;
use crate::task_discovery::support::{handle_discovery_error, handle_discovery_success};
use crate::task_discovery::{DiscoveredTasks, TaskDiscovery};
use crate::types::{TaskDefinitionFile, TaskDefinitionType, TaskFileStatus};
use std::path::Path;

pub(crate) struct GradleDiscovery;

impl TaskDiscovery for GradleDiscovery {
    fn discover(&self, dir: &Path, discovered: &mut DiscoveredTasks) {
        let _ = discover_gradle_tasks(dir, discovered);
    }
}

fn discover_gradle_tasks(dir: &Path, discovered: &mut DiscoveredTasks) -> Result<(), String> {
    let build_gradle_path = dir.join("build.gradle");
    if build_gradle_path.exists() {
        return match parse_gradle::parse(&build_gradle_path) {
            Ok(tasks) => {
                handle_discovery_success(
                    tasks,
                    build_gradle_path,
                    TaskDefinitionType::Gradle,
                    discovered,
                );
                Ok(())
            }
            Err(error) => {
                handle_discovery_error(
                    error,
                    build_gradle_path,
                    TaskDefinitionType::Gradle,
                    discovered,
                );
                Err("Error parsing build.gradle".to_string())
            }
        };
    }

    let build_gradle_kts_path = dir.join("build.gradle.kts");
    if build_gradle_kts_path.exists() {
        return match parse_gradle::parse(&build_gradle_kts_path) {
            Ok(tasks) => {
                handle_discovery_success(
                    tasks,
                    build_gradle_kts_path,
                    TaskDefinitionType::Gradle,
                    discovered,
                );
                Ok(())
            }
            Err(error) => {
                handle_discovery_error(
                    error,
                    build_gradle_kts_path,
                    TaskDefinitionType::Gradle,
                    discovered,
                );
                Err("Error parsing build.gradle.kts".to_string())
            }
        };
    }

    discovered.definitions.gradle = Some(TaskDefinitionFile {
        path: build_gradle_path,
        definition_type: TaskDefinitionType::Gradle,
        status: TaskFileStatus::NotFound,
    });
    Ok(())
}
