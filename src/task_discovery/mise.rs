use crate::parsers::parse_mise;
use crate::task_discovery::support::{apply_shadowing, set_definition};
use crate::task_discovery::{DiscoveredTasks, TaskDiscovery};
use crate::types::{Task, TaskDefinitionFile, TaskDefinitionType, TaskFileStatus};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const MISE_CONFIG_PATHS_HIGH_TO_LOW: [&str; 8] = [
    "mise.local.toml",
    ".mise.local.toml",
    "mise.toml",
    ".mise.toml",
    "mise/config.toml",
    ".mise/config.toml",
    ".config/mise.toml",
    ".config/mise/config.toml",
];

const DEFAULT_FILE_TASK_DIRECTORIES: [&str; 5] = [
    "mise-tasks",
    ".mise-tasks",
    ".mise/tasks",
    ".config/mise/tasks",
    "mise/tasks",
];

pub(crate) struct MiseDiscovery;

impl TaskDiscovery for MiseDiscovery {
    fn discover(&self, dir: &Path, discovered: &mut DiscoveredTasks) {
        discover_mise_tasks(dir, discovered);
    }
}

fn discover_mise_tasks(dir: &Path, discovered: &mut DiscoveredTasks) {
    let config_paths = find_mise_config_files(dir);
    let mut tasks_by_name = BTreeMap::new();
    let mut configured_includes = None;

    for config_path in &config_paths {
        let tasks = parse_mise::parse(config_path);
        let includes = parse_mise::task_includes(config_path, dir);

        match (tasks, includes) {
            (Ok(tasks), Ok(includes)) => {
                insert_tasks_if_absent(&mut tasks_by_name, tasks);
                if configured_includes.is_none() {
                    configured_includes = includes;
                }
                set_definition(
                    discovered,
                    TaskDefinitionFile {
                        path: config_path.clone(),
                        definition_type: TaskDefinitionType::Mise,
                        status: TaskFileStatus::Parsed,
                    },
                );
            }
            (Err(error), _) | (_, Err(error)) => record_parse_error(config_path, error, discovered),
        }
    }

    let task_sources: Vec<PathBuf> = configured_includes.map_or_else(
        || {
            DEFAULT_FILE_TASK_DIRECTORIES
                .iter()
                .map(|path| dir.join(path))
                .collect()
        },
        |includes| {
            includes
                .into_iter()
                .filter(|include| is_local_include(include))
                .map(|include| {
                    let path = PathBuf::from(include);
                    if path.is_absolute() {
                        path
                    } else {
                        dir.join(path)
                    }
                })
                .collect()
        },
    );

    let mut found_task_source = false;
    for task_source in task_sources.into_iter().rev() {
        if task_source.exists() {
            found_task_source = true;
        }
        discover_task_source(&task_source, &mut tasks_by_name, discovered);
    }

    if config_paths.is_empty() && !found_task_source {
        set_definition(
            discovered,
            TaskDefinitionFile {
                path: dir.join("mise.toml"),
                definition_type: TaskDefinitionType::Mise,
                status: TaskFileStatus::NotFound,
            },
        );
    }

    let mut tasks: Vec<_> = tasks_by_name.into_values().collect();
    apply_shadowing(&mut tasks);
    discovered.tasks.extend(tasks);
}

fn find_mise_config_files(dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<_> = MISE_CONFIG_PATHS_HIGH_TO_LOW
        .iter()
        .map(|relative_path| dir.join(relative_path))
        .filter(|path| path.is_file())
        .collect();

    let conf_d = dir.join(".config/mise/conf.d");
    if let Ok(entries) = std::fs::read_dir(conf_d) {
        let mut conf_d_paths: Vec<_> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "toml"))
            .collect();
        conf_d_paths.sort();
        conf_d_paths.reverse();
        paths.extend(conf_d_paths);
    }

    paths
}

fn discover_task_source(
    task_source: &Path,
    tasks_by_name: &mut BTreeMap<String, Task>,
    discovered: &mut DiscoveredTasks,
) {
    if task_source.is_dir() {
        discover_task_directory(task_source, task_source, tasks_by_name, discovered);
    } else if task_source.is_file() {
        if task_source.extension().is_some_and(|ext| ext == "toml") {
            discover_included_toml(task_source, tasks_by_name, discovered);
        } else if is_executable(task_source).unwrap_or(false) {
            let task_directory = task_source.parent().unwrap_or(Path::new("."));
            discover_file_task(task_directory, task_source, tasks_by_name, discovered);
        }
    }
}

fn discover_task_directory(
    task_directory: &Path,
    current_directory: &Path,
    tasks_by_name: &mut BTreeMap<String, Task>,
    discovered: &mut DiscoveredTasks,
) {
    let entries = match std::fs::read_dir(current_directory) {
        Ok(entries) => entries,
        Err(error) => {
            record_parse_error(current_directory, error, discovered);
            return;
        }
    };

    let mut paths: Vec<_> = entries.flatten().filter_map(non_symlink_path).collect();
    paths.sort();

    if paths.is_empty() && task_directory == current_directory {
        set_definition(
            discovered,
            TaskDefinitionFile {
                path: task_directory.to_path_buf(),
                definition_type: TaskDefinitionType::Mise,
                status: TaskFileStatus::Parsed,
            },
        );
    }

    for path in paths {
        if path.is_dir() {
            discover_task_directory(task_directory, &path, tasks_by_name, discovered);
        } else if path.extension().is_some_and(|ext| ext == "toml") {
            discover_included_toml(&path, tasks_by_name, discovered);
        } else {
            match is_executable(&path) {
                Ok(true) => discover_file_task(task_directory, &path, tasks_by_name, discovered),
                Ok(false) => {}
                Err(error) => record_parse_error(&path, error, discovered),
            }
        }
    }
}

fn non_symlink_path(entry: std::fs::DirEntry) -> Option<PathBuf> {
    let path = entry.path();
    (!path.is_symlink()).then_some(path)
}

fn discover_included_toml(
    path: &Path,
    tasks_by_name: &mut BTreeMap<String, Task>,
    discovered: &mut DiscoveredTasks,
) {
    match parse_mise::parse_included_toml(path) {
        Ok(tasks) => {
            insert_tasks_if_absent(tasks_by_name, tasks);
            record_parsed(path, discovered);
        }
        Err(error) => record_parse_error(path, error, discovered),
    }
}

fn discover_file_task(
    task_directory: &Path,
    path: &Path,
    tasks_by_name: &mut BTreeMap<String, Task>,
    discovered: &mut DiscoveredTasks,
) {
    match parse_mise::parse_file_task(task_directory, path) {
        Ok(Some(task)) => {
            tasks_by_name.entry(task.name.clone()).or_insert(task);
            record_parsed(path, discovered);
        }
        Ok(None) => record_parsed(path, discovered),
        Err(error) => record_parse_error(path, error, discovered),
    }
}

fn insert_tasks_if_absent(tasks_by_name: &mut BTreeMap<String, Task>, tasks: Vec<Task>) {
    for task in tasks {
        tasks_by_name.entry(task.name.clone()).or_insert(task);
    }
}

fn record_parsed(path: &Path, discovered: &mut DiscoveredTasks) {
    set_definition(
        discovered,
        TaskDefinitionFile {
            path: path.to_path_buf(),
            definition_type: TaskDefinitionType::Mise,
            status: TaskFileStatus::Parsed,
        },
    );
}

fn record_parse_error(
    path: &Path,
    error: impl std::fmt::Display,
    discovered: &mut DiscoveredTasks,
) {
    let error = error.to_string();
    discovered.errors.push(format!(
        "Failed to parse mise task definition {}: {}",
        path.display(),
        error
    ));
    set_definition(
        discovered,
        TaskDefinitionFile {
            path: path.to_path_buf(),
            definition_type: TaskDefinitionType::Mise,
            status: TaskFileStatus::ParseError(error),
        },
    );
}

fn is_local_include(include: &str) -> bool {
    !include.contains("://")
}

fn is_executable(path: &Path) -> std::io::Result<bool> {
    use std::os::unix::fs::PermissionsExt;

    Ok(path.metadata()?.permissions().mode() & 0o111 != 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    #[cfg(unix)]
    fn make_executable(path: &Path) {
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }

    #[test]
    fn test_discover_mise_toml_and_file_tasks() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(
            temp_dir.path().join("mise.toml"),
            "[tasks.build]\ndescription = 'Build the project'\nrun = 'cargo build'\n",
        )
        .unwrap();
        let task_directory = temp_dir.path().join("mise-tasks/test");
        fs::create_dir_all(&task_directory).unwrap();
        let file_task = task_directory.join("integration");
        fs::write(
            &file_task,
            "#!/bin/sh\n#MISE description='Run integration tests'\n",
        )
        .unwrap();
        #[cfg(unix)]
        make_executable(&file_task);

        let mut discovered = DiscoveredTasks::default();
        discover_mise_tasks(temp_dir.path(), &mut discovered);

        assert_eq!(
            discovered
                .tasks
                .iter()
                .map(|task| task.name.as_str())
                .collect::<Vec<_>>(),
            vec!["build", "test:integration"]
        );
        let file_task = discovered
            .tasks
            .iter()
            .find(|task| task.name == "test:integration")
            .unwrap();
        assert_eq!(
            file_task.allowlist_path(),
            task_directory.join("integration")
        );
    }

    #[test]
    fn test_non_executable_file_task_is_not_discovered() {
        let temp_dir = TempDir::new().unwrap();
        let task_directory = temp_dir.path().join(".mise/tasks");
        fs::create_dir_all(&task_directory).unwrap();
        fs::write(task_directory.join("build"), "#!/bin/sh\n").unwrap();

        let mut discovered = DiscoveredTasks::default();
        discover_mise_tasks(temp_dir.path(), &mut discovered);

        assert!(discovered.tasks.is_empty());
    }

    #[test]
    fn test_custom_includes_replace_default_file_task_directories() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(
            temp_dir.path().join(".mise.toml"),
            "[task_config]\nincludes = ['{{ config_root }}/tasks.toml', 'project-tasks']\n",
        )
        .unwrap();
        fs::write(
            temp_dir.path().join("tasks.toml"),
            "shared = 'echo shared'\n",
        )
        .unwrap();
        let custom_directory = temp_dir.path().join("project-tasks");
        fs::create_dir_all(&custom_directory).unwrap();
        let custom_task = custom_directory.join("deploy");
        fs::write(&custom_task, "#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        make_executable(&custom_task);

        let default_directory = temp_dir.path().join("mise-tasks");
        fs::create_dir_all(&default_directory).unwrap();
        let default_task = default_directory.join("ignored");
        fs::write(&default_task, "#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        make_executable(&default_task);

        let mut discovered = DiscoveredTasks::default();
        discover_mise_tasks(temp_dir.path(), &mut discovered);
        let task_names: Vec<_> = discovered
            .tasks
            .iter()
            .map(|task| task.name.as_str())
            .collect();

        assert_eq!(task_names, vec!["deploy", "shared"]);
        assert!(!task_names.contains(&"ignored"));
    }

    #[test]
    #[serial]
    fn test_later_default_task_directory_wins() {
        let temp_dir = TempDir::new().unwrap();
        for relative_directory in DEFAULT_FILE_TASK_DIRECTORIES {
            let task_directory = temp_dir.path().join(relative_directory);
            fs::create_dir_all(&task_directory).unwrap();
            let task = task_directory.join("build");
            fs::write(&task, "#!/bin/sh\n").unwrap();
            make_executable(&task);
        }

        let mut discovered = DiscoveredTasks::default();
        discover_mise_tasks(temp_dir.path(), &mut discovered);

        let task = discovered
            .tasks
            .iter()
            .find(|task| task.name == "build")
            .unwrap();
        assert_eq!(task.file_path, temp_dir.path().join("mise/tasks/build"));
    }

    #[cfg(unix)]
    #[test]
    #[serial]
    fn test_directory_symlink_is_not_followed() {
        use std::os::unix::fs::symlink;

        let temp_dir = TempDir::new().unwrap();
        let task_directory = temp_dir.path().join("mise-tasks");
        fs::create_dir_all(&task_directory).unwrap();
        let task = task_directory.join("build");
        fs::write(&task, "#!/bin/sh\n").unwrap();
        make_executable(&task);
        symlink(".", task_directory.join("loop")).unwrap();

        let mut discovered = DiscoveredTasks::default();
        discover_mise_tasks(temp_dir.path(), &mut discovered);

        assert_eq!(
            discovered
                .tasks
                .iter()
                .map(|task| task.name.as_str())
                .collect::<Vec<_>>(),
            vec!["build"]
        );
        assert!(discovered.errors.is_empty());
    }

    #[test]
    fn test_empty_directory_records_missing_mise_definition() {
        let temp_dir = TempDir::new().unwrap();
        let mut discovered = DiscoveredTasks::default();

        discover_mise_tasks(temp_dir.path(), &mut discovered);

        assert!(matches!(
            discovered
                .definitions
                .get_first(&TaskDefinitionType::Mise)
                .unwrap()
                .status,
            TaskFileStatus::NotFound
        ));
    }
}
