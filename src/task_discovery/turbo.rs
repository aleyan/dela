use crate::composed_paths::{ComposedDefinitionSource, RecursiveDiscoveryState, VisitState};
use crate::parsers::parse_turbo_json;
use crate::repo_root::find_git_repo_root;
use crate::task_discovery::support::{apply_shadowing, set_definition};
use crate::task_discovery::{DiscoveredTasks, TaskDiscovery};
use crate::types::{Task, TaskDefinitionFile, TaskDefinitionType, TaskFileStatus};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) struct TurboDiscovery;

impl TaskDiscovery for TurboDiscovery {
    fn discover(&self, dir: &Path, discovered: &mut DiscoveredTasks) {
        let _ = discover_turbo_tasks(dir, discovered);
    }
}

fn discover_turbo_tasks(dir: &Path, discovered: &mut DiscoveredTasks) -> anyhow::Result<()> {
    let repo_root = find_git_repo_root(dir).unwrap_or_else(|| dir.to_path_buf());
    let turbo_json = repo_root.join("turbo.json");

    if !turbo_json.exists() {
        set_definition(
            discovered,
            TaskDefinitionFile {
                path: turbo_json,
                definition_type: TaskDefinitionType::TurboJson,
                status: TaskFileStatus::NotFound,
            },
        );
        return Ok(());
    }

    let mut tasks_by_name = BTreeMap::new();
    let mut config_errors = Vec::new();

    let result = collect_turbo_tasks_for_context(
        &repo_root,
        dir,
        &turbo_json,
        &mut tasks_by_name,
        &mut config_errors,
    );

    let mut tasks: Vec<_> = tasks_by_name.into_values().collect();
    apply_shadowing(&mut tasks);
    discovered.tasks.extend(tasks);
    discovered.errors.extend(config_errors);

    let status = match result {
        Ok(()) => TaskFileStatus::Parsed,
        Err(e) => {
            discovered
                .errors
                .push(format!("Failed to parse {}: {}", turbo_json.display(), e));
            TaskFileStatus::ParseError(e.to_string())
        }
    };

    set_definition(
        discovered,
        TaskDefinitionFile {
            path: turbo_json,
            definition_type: TaskDefinitionType::TurboJson,
            status,
        },
    );

    Ok(())
}

fn collect_turbo_tasks_for_context(
    repo_root: &Path,
    dir: &Path,
    root_turbo_json: &Path,
    collected_tasks: &mut BTreeMap<String, Task>,
    config_errors: &mut Vec<String>,
) -> anyhow::Result<()> {
    let root_source = ComposedDefinitionSource::direct(root_turbo_json.to_path_buf());
    let mut package_configs_by_name = None;
    let root_tasks = resolve_effective_turbo_tasks(
        &root_source,
        repo_root,
        root_turbo_json,
        &mut package_configs_by_name,
        &mut RecursiveDiscoveryState::new(),
    )?;
    collected_tasks.extend(root_tasks);

    let mut first_error = None;

    if dir == repo_root {
        for config_path in collect_descendant_turbo_config_paths(repo_root) {
            let config_source =
                ComposedDefinitionSource::composed(root_turbo_json, config_path.clone());
            match resolve_effective_turbo_tasks(
                &config_source,
                repo_root,
                root_turbo_json,
                &mut package_configs_by_name,
                &mut RecursiveDiscoveryState::new(),
            ) {
                Ok(tasks) => {
                    for (name, task) in tasks {
                        collected_tasks.entry(name).or_insert(task);
                    }
                }
                Err(e) => {
                    let error = format!(
                        "Failed to parse workspace-local turbo config '{}': {}",
                        config_path.display(),
                        e
                    );
                    config_errors.push(error.clone());
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
            }
        }
    } else {
        for config_path in collect_turbo_ancestor_config_paths(dir, repo_root) {
            let config_source =
                ComposedDefinitionSource::composed(root_turbo_json, config_path.clone());
            match resolve_effective_turbo_tasks(
                &config_source,
                repo_root,
                root_turbo_json,
                &mut package_configs_by_name,
                &mut RecursiveDiscoveryState::new(),
            ) {
                Ok(tasks) if !tasks.is_empty() => {
                    *collected_tasks = tasks;
                    break;
                }
                Ok(_) => {}
                Err(e) => {
                    let error = format!(
                        "Failed to parse workspace-local turbo config '{}': {}",
                        config_path.display(),
                        e
                    );
                    config_errors.push(error.clone());
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                    break;
                }
            }
        }
    }

    if let Some(error) = first_error {
        Err(anyhow::anyhow!(error))
    } else {
        Ok(())
    }
}

fn resolve_effective_turbo_tasks(
    current_source: &ComposedDefinitionSource,
    repo_root: &Path,
    root_turbo_json: &Path,
    package_configs_by_name: &mut Option<HashMap<String, PathBuf>>,
    traversal_state: &mut RecursiveDiscoveryState,
) -> anyhow::Result<BTreeMap<String, Task>> {
    match traversal_state.mark_visited(current_source.definition_path()) {
        VisitState::AlreadyVisited(_) => return Ok(BTreeMap::new()),
        VisitState::New(_) => {}
    }

    let config = parse_turbo_json::load_config(current_source.definition_path())?;

    if current_source.definition_path() != root_turbo_json && config.extends.is_empty() {
        return Ok(BTreeMap::new());
    }

    let mut tasks = BTreeMap::new();

    if current_source.definition_path() != root_turbo_json {
        for extend_entry in &config.extends {
            let Some(parent_config_path) = resolve_turbo_extends_entry(
                current_source,
                extend_entry,
                repo_root,
                root_turbo_json,
                package_configs_by_name,
            ) else {
                continue;
            };

            if !parent_config_path.is_file() {
                continue;
            }

            let parent_source =
                ComposedDefinitionSource::composed(root_turbo_json, parent_config_path.clone());
            let inherited_tasks = resolve_effective_turbo_tasks(
                &parent_source,
                repo_root,
                root_turbo_json,
                package_configs_by_name,
                traversal_state,
            )?;
            tasks.extend(inherited_tasks);
        }
    }

    for (name, task_config) in &config.tasks {
        if !task_config.is_effective_task_definition() {
            tasks.remove(name.as_str());
        }
    }

    let mut local_tasks = parse_turbo_json::parse(current_source.definition_path())?;
    for task in &mut local_tasks {
        current_source.apply_to_task(task);
    }
    for task in local_tasks {
        tasks.insert(task.name.clone(), task);
    }

    Ok(tasks)
}

fn resolve_turbo_extends_entry(
    current_source: &ComposedDefinitionSource,
    extend_entry: &str,
    repo_root: &Path,
    root_turbo_json: &Path,
    package_configs_by_name: &mut Option<HashMap<String, PathBuf>>,
) -> Option<PathBuf> {
    if extend_entry == "//" {
        return Some(root_turbo_json.to_path_buf());
    }

    if looks_like_turbo_config_path(extend_entry) {
        let candidate = current_source.resolve_child(extend_entry);
        return Some(resolve_turbo_config_path_candidate(&candidate));
    }

    let package_configs_by_name =
        package_configs_by_name.get_or_insert_with(|| build_turbo_package_config_index(repo_root));
    package_configs_by_name.get(extend_entry).cloned()
}

fn collect_turbo_ancestor_config_paths(dir: &Path, repo_root: &Path) -> Vec<PathBuf> {
    let mut current = dir.to_path_buf();
    let mut config_paths = Vec::new();

    while current.starts_with(repo_root) && current != repo_root {
        let candidate = current.join("turbo.json");
        if candidate.is_file() {
            config_paths.push(candidate);
        }

        if !current.pop() {
            break;
        }
    }

    config_paths
}

fn collect_descendant_turbo_config_paths(repo_root: &Path) -> Vec<PathBuf> {
    let mut config_paths = Vec::new();
    collect_descendant_turbo_config_paths_recursive(repo_root, repo_root, &mut config_paths);
    config_paths.sort();
    config_paths
}

fn collect_descendant_turbo_config_paths_recursive(
    repo_root: &Path,
    current_dir: &Path,
    config_paths: &mut Vec<PathBuf>,
) {
    let Ok(entries) = fs::read_dir(current_dir) else {
        return;
    };

    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() || !file_type.is_dir() {
            continue;
        }

        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if should_skip_turbo_config_scan(file_name) {
            continue;
        }

        let candidate = path.join("turbo.json");
        if candidate.is_file() && candidate != repo_root.join("turbo.json") {
            config_paths.push(candidate);
        }

        collect_descendant_turbo_config_paths_recursive(repo_root, &path, config_paths);
    }
}

fn should_skip_turbo_config_scan(file_name: &str) -> bool {
    matches!(file_name, ".git" | "node_modules")
}

fn looks_like_turbo_config_path(extend_entry: &str) -> bool {
    let extend_path = Path::new(extend_entry);
    let is_scoped_package = extend_entry.starts_with('@');
    extend_path.is_absolute()
        || extend_entry.starts_with('.')
        || (!is_scoped_package && extend_entry.contains(std::path::MAIN_SEPARATOR))
        || (!is_scoped_package && extend_entry.contains('/'))
        || (!is_scoped_package && extend_entry.contains('\\'))
}

fn resolve_turbo_config_path_candidate(candidate: &Path) -> PathBuf {
    if candidate
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "turbo.json")
    {
        return candidate.to_path_buf();
    }

    if candidate.extension().is_none() || candidate.is_dir() {
        return candidate.join("turbo.json");
    }

    candidate.to_path_buf()
}

fn build_turbo_package_config_index(repo_root: &Path) -> HashMap<String, PathBuf> {
    let mut package_configs = HashMap::new();

    let root_turbo_json = repo_root.join("turbo.json");
    if root_turbo_json.is_file()
        && let Some(package_name) = read_package_name(repo_root)
    {
        package_configs.insert(package_name, root_turbo_json);
    }

    for config_path in collect_descendant_turbo_config_paths(repo_root) {
        let Some(config_dir) = config_path.parent() else {
            continue;
        };
        let Some(package_name) = read_package_name(config_dir) else {
            continue;
        };
        package_configs.entry(package_name).or_insert(config_path);
    }

    package_configs
}

fn read_package_name(dir: &Path) -> Option<String> {
    let package_json_path = dir.join("package.json");
    let contents = fs::read_to_string(package_json_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&contents).ok()?;
    json.get("name")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::looks_like_turbo_config_path;

    #[test]
    fn looks_like_turbo_config_path_treats_scoped_packages_as_packages() {
        assert!(!looks_like_turbo_config_path("@scope/pkg"));
        assert!(looks_like_turbo_config_path("packages/shared"));
        assert!(looks_like_turbo_config_path(".turbo/shared"));
    }
}
