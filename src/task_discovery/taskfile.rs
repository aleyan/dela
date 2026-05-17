#[allow(unused_imports)]
use anyhow::anyhow;
use crate::composed_paths::{ComposedDefinitionSource, RecursiveDiscoveryState, VisitState};
use crate::parsers::parse_taskfile;
use crate::task_discovery::support::{apply_shadowing, set_definition};
use crate::task_discovery::{DiscoveredTasks, TaskDiscovery};
use crate::types::{Task, TaskDefinitionFile, TaskDefinitionType, TaskFileStatus};
use std::collections::HashSet;
use std::path::Path;

pub(crate) struct TaskfileDiscovery;

impl TaskDiscovery for TaskfileDiscovery {
    fn discover(&self, dir: &Path, discovered: &mut DiscoveredTasks) {
        let _ = discover_taskfile_tasks(dir, discovered);
    }
}

fn discover_taskfile_tasks(dir: &Path, discovered: &mut DiscoveredTasks) -> anyhow::Result<()> {
    let default_path = dir.join(parse_taskfile::SUPPORTED_TASKFILE_NAMES[0]);
    let Some(taskfile_path) = parse_taskfile::find_taskfile_in_dir(dir) else {
        set_definition(
            discovered,
            TaskDefinitionFile {
                path: default_path,
                definition_type: TaskDefinitionType::Taskfile,
                status: TaskFileStatus::NotFound,
            },
        );
        return Ok(());
    };

    let root_source = ComposedDefinitionSource::direct(taskfile_path.clone());
    let mut traversal_state = RecursiveDiscoveryState::new();
    let mut seen_task_names = HashSet::new();
    let mut tasks = Vec::new();
    let mut include_errors = Vec::new();
    let no_excludes = HashSet::new();
    let mut traversal = TaskfileTraversal {
        root_taskfile_path: &taskfile_path,
        traversal_state: &mut traversal_state,
        seen_task_names: &mut seen_task_names,
        collected_tasks: &mut tasks,
        include_errors: &mut include_errors,
    };

    let result = collect_taskfile_tasks_recursive(
        &root_source,
        "",
        None,
        false,
        &no_excludes,
        &mut traversal,
    );

    apply_shadowing(&mut tasks);
    discovered.tasks.extend(tasks);
    discovered.errors.extend(include_errors);

    let status = match result {
        Ok(()) => TaskFileStatus::Parsed,
        Err(e) => {
            discovered.errors.push(format!(
                "Failed to parse {}: {}",
                taskfile_path.display(),
                e
            ));
            TaskFileStatus::ParseError(e.to_string())
        }
    };

    set_definition(
        discovered,
        TaskDefinitionFile {
            path: taskfile_path,
            definition_type: TaskDefinitionType::Taskfile,
            status,
        },
    );

    Ok(())
}

struct TaskfileTraversal<'a> {
    root_taskfile_path: &'a Path,
    traversal_state: &'a mut RecursiveDiscoveryState,
    seen_task_names: &'a mut HashSet<String>,
    collected_tasks: &'a mut Vec<Task>,
    include_errors: &'a mut Vec<String>,
}

fn collect_taskfile_tasks_recursive(
    current_source: &ComposedDefinitionSource,
    namespace_prefix: &str,
    include_label: Option<&str>,
    hide_tasks: bool,
    excluded_tasks: &HashSet<String>,
    traversal: &mut TaskfileTraversal<'_>,
) -> anyhow::Result<()> {
    match traversal
        .traversal_state
        .mark_visited(current_source.definition_path())
    {
        VisitState::AlreadyVisited(_) => return Ok(()),
        VisitState::New(_) => {}
    }

    let mut first_error = None;

    let mut tasks = parse_taskfile::parse(current_source.definition_path()).map_err(|e| anyhow::anyhow!(e))?;
    tasks.sort_by(|a, b| a.name.cmp(&b.name));

    if !hide_tasks {
        for mut task in tasks {
            let original_name = task.name.clone();
            if excluded_tasks.contains(&original_name) {
                continue;
            }

            let effective_name = prefix_taskfile_task_name(namespace_prefix, &original_name);
            task.name = effective_name.clone();
            task.source_name = effective_name;
            current_source.apply_to_task(&mut task);

            if !traversal.seen_task_names.insert(task.name.clone()) {
                let error = match include_label {
                    Some(include_label) => {
                        format!(
                            "Found multiple tasks ({}) included by \"{}\"",
                            task.name, include_label
                        )
                    }
                    None => format!("Found multiple Taskfile tasks named '{}'", task.name),
                };
                traversal.include_errors.push(error.clone());
                if first_error.is_none() {
                    first_error = Some(error);
                }
                continue;
            }

            traversal.collected_tasks.push(task);
        }
    }

    let includes = parse_taskfile::extract_include_directives(current_source.definition_path()).map_err(|e| anyhow::anyhow!(e))?;
    for include in includes {
        let resolved_candidate = current_source.resolve_child(&include.taskfile);
        let resolved_include = parse_taskfile::resolve_taskfile_include_path(&resolved_candidate);

        if !resolved_include.is_file() {
            continue;
        }

        let child_source = ComposedDefinitionSource::composed(
            traversal.root_taskfile_path,
            resolved_include.clone(),
        );
        let child_namespace = if include.flatten {
            namespace_prefix.to_string()
        } else {
            prefix_taskfile_task_name(namespace_prefix, &include.namespace)
        };
        let child_include_label = prefix_taskfile_task_name(namespace_prefix, &include.namespace);
        let child_hide_tasks = hide_tasks || include.internal;
        let child_excludes = include.excludes.into_iter().collect();

        if let Err(e) = collect_taskfile_tasks_recursive(
            &child_source,
            &child_namespace,
            Some(child_include_label.as_str()),
            child_hide_tasks,
            &child_excludes,
            traversal,
        ) {
            let error = format!(
                "Failed to parse included Taskfile '{}': {}",
                resolved_include.display(),
                e
            );
            traversal.include_errors.push(error.clone());
            if first_error.is_none() {
                first_error = Some(error);
            }
        }
    }

    if let Some(error) = first_error {
        Err(anyhow::anyhow!(error))
    } else {
        Ok(())
    }
}

fn prefix_taskfile_task_name(namespace_prefix: &str, task_name: &str) -> String {
    if namespace_prefix.is_empty() {
        task_name.to_string()
    } else {
        format!("{}:{}", namespace_prefix, task_name)
    }
}
