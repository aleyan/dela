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
    discovered.definitions.insert(definition);
}

pub(crate) fn handle_discovery_error(
    error: impl std::fmt::Display,
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
            status: TaskFileStatus::ParseError(error.to_string()),
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
