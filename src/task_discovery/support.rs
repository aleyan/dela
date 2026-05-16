use crate::task_discovery::{DiscoveredTasks, TaskDefinitionFile};
use crate::task_shadowing::check_shadowing;
use crate::types::{Task, TaskDefinitionType, TaskFileStatus};
use std::path::PathBuf;

pub(crate) fn apply_shadowing(tasks: &mut [Task]) {
    for task in tasks {
        task.shadowed_by = check_shadowing(&task.name);
    }
}

pub(crate) fn set_definition(discovered: &mut DiscoveredTasks, definition: TaskDefinitionFile) {
    match definition.definition_type {
        TaskDefinitionType::Makefile => discovered.definitions.makefile = Some(definition),
        TaskDefinitionType::PackageJson => discovered.definitions.package_json = Some(definition),
        TaskDefinitionType::PyprojectToml => {
            discovered.definitions.pyproject_toml = Some(definition)
        }
        TaskDefinitionType::Taskfile => discovered.definitions.taskfile = Some(definition),
        TaskDefinitionType::TurboJson => discovered.definitions.turbo_json = Some(definition),
        TaskDefinitionType::MavenPom => discovered.definitions.maven_pom = Some(definition),
        TaskDefinitionType::Gradle => discovered.definitions.gradle = Some(definition),
        TaskDefinitionType::GitHubActions => {
            discovered.definitions.github_actions = Some(definition)
        }
        TaskDefinitionType::DockerCompose => {
            discovered.definitions.docker_compose = Some(definition)
        }
        TaskDefinitionType::TravisCi => discovered.definitions.travis_ci = Some(definition),
        TaskDefinitionType::CMake => discovered.definitions.cmake = Some(definition),
        TaskDefinitionType::Justfile => discovered.definitions.justfile = Some(definition),
        _ => {}
    }
}

pub(crate) fn handle_discovery_error(
    error: String,
    file_path: PathBuf,
    definition_type: TaskDefinitionType,
    discovered: &mut DiscoveredTasks,
) {
    discovered.errors.push(format!(
        "Failed to parse {}: {}",
        file_path.display(),
        error
    ));
    set_definition(
        discovered,
        TaskDefinitionFile {
            path: file_path,
            definition_type,
            status: TaskFileStatus::ParseError(error),
        },
    );
}

pub(crate) fn handle_discovery_success(
    mut tasks: Vec<Task>,
    file_path: PathBuf,
    definition_type: TaskDefinitionType,
    discovered: &mut DiscoveredTasks,
) {
    apply_shadowing(&mut tasks);
    set_definition(
        discovered,
        TaskDefinitionFile {
            path: file_path,
            definition_type,
            status: TaskFileStatus::Parsed,
        },
    );
    discovered.tasks.extend(tasks);
}
