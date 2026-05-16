use crate::parsers::parse_docker_compose;
use crate::task_discovery::support::{handle_discovery_error, handle_discovery_success};
use crate::task_discovery::{DiscoveredTasks, TaskDiscovery};
use crate::types::{TaskDefinitionFile, TaskDefinitionType, TaskFileStatus};
use std::path::Path;

pub(crate) struct DockerComposeDiscovery;

impl TaskDiscovery for DockerComposeDiscovery {
    fn discover(&self, dir: &Path, discovered: &mut DiscoveredTasks) {
        let _ = discover_docker_compose_tasks(dir, discovered);
    }
}

fn discover_docker_compose_tasks(
    dir: &Path,
    discovered: &mut DiscoveredTasks,
) -> Result<(), String> {
    let docker_compose_files = parse_docker_compose::find_docker_compose_files(dir);

    if docker_compose_files.is_empty() {
        discovered.definitions.docker_compose = Some(TaskDefinitionFile {
            path: dir.join("docker-compose.yml"),
            definition_type: TaskDefinitionType::DockerCompose,
            status: TaskFileStatus::NotFound,
        });
        return Ok(());
    }

    let docker_compose_path = docker_compose_files[0].clone();
    match parse_docker_compose::parse(&docker_compose_path) {
        Ok(tasks) => {
            handle_discovery_success(
                tasks,
                docker_compose_path,
                TaskDefinitionType::DockerCompose,
                discovered,
            );
        }
        Err(error) => {
            handle_discovery_error(
                error,
                docker_compose_path,
                TaskDefinitionType::DockerCompose,
                discovered,
            );
        }
    }

    Ok(())
}
