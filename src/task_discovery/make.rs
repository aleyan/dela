use crate::composed_paths::{ComposedDefinitionSource, RecursiveDiscoveryState, VisitState};
use crate::parsers::parse_makefile;
use crate::task_discovery::support::{apply_shadowing, set_definition};
use crate::task_discovery::{DiscoveredTasks, TaskDiscovery};
use crate::types::{Task, TaskDefinitionFile, TaskDefinitionType, TaskFileStatus};
use std::collections::HashSet;
use std::path::Path;

pub(crate) struct MakefileDiscovery;

impl TaskDiscovery for MakefileDiscovery {
    fn discover(&self, dir: &Path, discovered: &mut DiscoveredTasks) {
        discover_makefile_tasks(dir, discovered);
    }
}

fn discover_makefile_tasks(dir: &Path, discovered: &mut DiscoveredTasks) {
    let makefile_path = dir.join("Makefile");

    if !makefile_path.exists() {
        set_definition(
            discovered,
            TaskDefinitionFile {
                path: makefile_path,
                definition_type: TaskDefinitionType::Makefile,
                status: TaskFileStatus::NotFound,
            },
        );
        return;
    }

    let root_source = ComposedDefinitionSource::direct(makefile_path.clone());
    let mut traversal_state = RecursiveDiscoveryState::new();
    let mut seen_task_names = HashSet::new();
    let mut tasks = Vec::new();
    let mut include_errors = Vec::new();

    let result = collect_makefile_tasks_recursive(
        &makefile_path,
        &root_source,
        &mut traversal_state,
        &mut seen_task_names,
        &mut tasks,
        &mut include_errors,
    );

    apply_shadowing(&mut tasks);
    discovered.tasks.extend(tasks);
    discovered.errors.extend(include_errors);

    let status = match result {
        Ok(()) => TaskFileStatus::Parsed,
        Err(error) => {
            discovered.errors.push(format!(
                "Failed to parse {}: {}",
                makefile_path.display(),
                error
            ));
            TaskFileStatus::ParseError(error)
        }
    };

    set_definition(
        discovered,
        TaskDefinitionFile {
            path: makefile_path,
            definition_type: TaskDefinitionType::Makefile,
            status,
        },
    );
}

fn collect_makefile_tasks_recursive(
    root_makefile_path: &Path,
    current_source: &ComposedDefinitionSource,
    traversal_state: &mut RecursiveDiscoveryState,
    seen_task_names: &mut HashSet<String>,
    collected_tasks: &mut Vec<Task>,
    include_errors: &mut Vec<String>,
) -> Result<(), String> {
    match traversal_state.mark_visited(current_source.definition_path()) {
        VisitState::AlreadyVisited(_) => return Ok(()),
        VisitState::New(_) => {}
    }

    let mut first_error = None;

    let mut tasks = parse_makefile::parse(current_source.definition_path())?;
    for task in &mut tasks {
        current_source.apply_to_task(task);
    }
    for task in tasks {
        if seen_task_names.insert(task.name.clone()) {
            collected_tasks.push(task);
        }
    }

    let includes = parse_makefile::extract_include_directives(current_source.definition_path())?;
    for include in includes {
        let resolved_include = current_source.resolve_child(&include.path);
        if !resolved_include.is_file() {
            continue;
        }

        let include_source =
            ComposedDefinitionSource::composed(root_makefile_path, resolved_include.clone());
        if let Err(error) = collect_makefile_tasks_recursive(
            root_makefile_path,
            &include_source,
            traversal_state,
            seen_task_names,
            collected_tasks,
            include_errors,
        ) {
            let error = format!(
                "Failed to parse included makefile '{}': {}",
                resolved_include.display(),
                error
            );
            include_errors.push(error.clone());
            if first_error.is_none() {
                first_error = Some(error);
            }
        }
    }

    if let Some(error) = first_error {
        Err(error)
    } else {
        Ok(())
    }
}
