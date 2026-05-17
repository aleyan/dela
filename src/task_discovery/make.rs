use crate::composed_paths::{ComposedDefinitionSource, RecursiveDiscoveryState, VisitState};
use crate::parsers::parse_makefile;
use crate::task_discovery::support::{apply_shadowing, set_definition};
use crate::task_discovery::{DiscoveredTasks, TaskDiscovery};
use crate::types::{Task, TaskDefinitionFile, TaskDefinitionType, TaskFileStatus};
#[allow(unused_imports)]
use anyhow::anyhow;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

pub(crate) struct MakefileDiscovery;

const MAKEFILE_NAMES: [&str; 3] = ["GNUmakefile", "makefile", "Makefile"];

impl TaskDiscovery for MakefileDiscovery {
    fn discover(&self, dir: &Path, discovered: &mut DiscoveredTasks) {
        discover_makefile_tasks(dir, discovered);
    }
}

fn discover_makefile_tasks(dir: &Path, discovered: &mut DiscoveredTasks) {
    let Some(makefile_path) = find_makefile_path(dir) else {
        set_definition(
            discovered,
            TaskDefinitionFile {
                path: dir.join("Makefile"),
                definition_type: TaskDefinitionType::Makefile,
                status: TaskFileStatus::NotFound,
            },
        );
        return;
    };

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
            TaskFileStatus::ParseError(error.to_string())
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

fn find_makefile_path(dir: &Path) -> Option<std::path::PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    let mut paths_by_name = std::collections::HashMap::new();

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        paths_by_name.insert(file_name.to_string(), path);
    }

    MAKEFILE_NAMES
        .iter()
        .find_map(|name| paths_by_name.remove(*name))
}

fn collect_makefile_tasks_recursive(
    root_makefile_path: &Path,
    current_source: &ComposedDefinitionSource,
    traversal_state: &mut RecursiveDiscoveryState,
    seen_task_names: &mut HashSet<String>,
    collected_tasks: &mut Vec<Task>,
    include_errors: &mut Vec<String>,
) -> anyhow::Result<()> {
    match traversal_state.mark_visited(current_source.definition_path()) {
        VisitState::AlreadyVisited(_) => return Ok(()),
        VisitState::New(_) => {}
    }

    let mut tasks = parse_makefile::parse(current_source.definition_path())?;
    for task in &mut tasks {
        current_source.apply_to_task(task);
    }
    // We intentionally keep discovery name-oriented instead of reimplementing GNU make's
    // full override semantics. Dela only needs a stable task list here; `make` remains the
    // source of truth for which recipe actually executes when duplicate targets exist.
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
            include_errors.push(error);
        }
    }

    Ok(())
}
