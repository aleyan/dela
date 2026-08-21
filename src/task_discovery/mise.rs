use crate::parsers::parse_mise;
use crate::task_discovery::support::{apply_shadowing, set_definition};
use crate::task_discovery::{DiscoveredTasks, TaskDiscovery};
use crate::types::{Task, TaskDefinitionFile, TaskDefinitionType, TaskFileStatus};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// mise configuration sources below the environment-specific and local files,
/// highest precedence first. An entry naming a `conf.d` directory stands for
/// every `.toml` file inside it.
const BASE_MISE_CONFIG_PATHS_HIGH_TO_LOW: [&str; 8] = [
    "mise.toml",
    ".mise.toml",
    "mise/config.toml",
    ".mise/config.toml",
    ".mise/conf.d",
    ".config/mise.toml",
    ".config/mise/config.toml",
    ".config/mise/conf.d",
];

/// Directories mise searches for file tasks, in the order it searches them.
/// The first directory holding a given task name is the one that defines it.
const DEFAULT_FILE_TASK_DIRECTORIES_HIGH_TO_LOW: [&str; 5] = [
    "mise-tasks",
    ".mise-tasks",
    "mise/tasks",
    ".mise/tasks",
    ".config/mise/tasks",
];

pub(crate) struct MiseDiscovery;

impl TaskDiscovery for MiseDiscovery {
    fn discover(&self, dir: &Path, discovered: &mut DiscoveredTasks) {
        discover_mise_tasks(dir, discovered);
    }
}

fn discover_mise_tasks(dir: &Path, discovered: &mut DiscoveredTasks) {
    let mise_env = std::env::var("MISE_ENV").ok();
    discover_mise_tasks_with_env(dir, discovered, mise_env.as_deref());
}

fn discover_mise_tasks_with_env(
    dir: &Path,
    discovered: &mut DiscoveredTasks,
    mise_env: Option<&str>,
) {
    let config_paths = find_mise_config_files(dir, mise_env);
    let mut tasks_by_name = BTreeMap::new();
    let mut configured_includes = None;

    for config_path in &config_paths {
        let (tasks_result, includes_result) = match parse_mise::parse_config(config_path, dir) {
            Ok(results) => results,
            Err(error) => {
                record_parse_error(config_path, error, discovered);
                continue;
            }
        };

        let tasks_error = tasks_result.as_ref().err().map(ToString::to_string);
        let includes_error = includes_result.as_ref().err().map(ToString::to_string);

        if let Ok(tasks) = tasks_result {
            insert_tasks_if_absent(&mut tasks_by_name, tasks);
        }
        if let Ok(includes) = includes_result
            && configured_includes.is_none()
        {
            configured_includes = includes;
        }

        for error in tasks_error.iter().chain(includes_error.iter()) {
            push_error(config_path, error, discovered);
        }

        set_definition(
            discovered,
            TaskDefinitionFile {
                path: config_path.clone(),
                definition_type: TaskDefinitionType::Mise,
                status: match tasks_error.or(includes_error) {
                    Some(error) => TaskFileStatus::ParseError(error),
                    None => TaskFileStatus::Parsed,
                },
            },
        );
    }

    let is_explicit_includes = configured_includes.is_some();
    let task_sources: Vec<PathBuf> = configured_includes.map_or_else(
        || {
            DEFAULT_FILE_TASK_DIRECTORIES_HIGH_TO_LOW
                .iter()
                .map(|path| dir.join(path))
                .collect()
        },
        |includes| {
            includes
                .into_iter()
                .rev()
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

    // Task sources are ordered highest precedence first, and tasks are only
    // inserted under a name that is still free, so the first source naming a
    // task is the one that defines it.
    let mut found_task_source = false;
    for task_source in task_sources {
        if task_source.exists() {
            found_task_source = true;
            discover_task_source(&task_source, &mut tasks_by_name, discovered);
        } else if is_explicit_includes {
            set_definition(
                discovered,
                TaskDefinitionFile {
                    path: task_source,
                    definition_type: TaskDefinitionType::Mise,
                    status: TaskFileStatus::NotFound,
                },
            );
        }
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

fn find_mise_config_files(dir: &Path, mise_env: Option<&str>) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    for relative_path in mise_config_paths_high_to_low(mise_env) {
        let path = dir.join(&relative_path);
        if relative_path.ends_with("conf.d") {
            paths.extend(conf_d_config_files(&path));
        } else if path.is_file() {
            paths.push(path);
        }
    }

    paths
}

fn mise_config_paths_high_to_low(mise_env: Option<&str>) -> Vec<String> {
    let environments: Vec<_> = mise_env
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|environment| !environment.is_empty())
        .collect();
    let mut paths = Vec::new();

    // The final environment has the highest precedence when MISE_ENV contains
    // multiple comma-separated values.
    for environment in environments.iter().rev() {
        paths.push(format!("mise.{environment}.local.toml"));
        paths.push(format!(".mise.{environment}.local.toml"));
    }
    paths.push("mise.local.toml".to_string());
    paths.push(".mise.local.toml".to_string());
    for environment in environments.iter().rev() {
        paths.push(format!("mise.{environment}.toml"));
        paths.push(format!(".mise.{environment}.toml"));
    }
    paths.extend(
        BASE_MISE_CONFIG_PATHS_HIGH_TO_LOW
            .iter()
            .map(|path| (*path).to_string()),
    );
    paths
}

/// Return the `.toml` files in a `conf.d` directory, highest precedence first.
/// mise layers them alphabetically, so the last name alphabetically wins.
fn conf_d_config_files(conf_d: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(conf_d) else {
        return Vec::new();
    };

    let mut paths: Vec<_> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "toml"))
        .collect();
    paths.sort();
    paths.reverse();
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

    let mut paths: Vec<_> = entries.flatten().filter_map(task_directory_entry).collect();
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

/// Symlinked directories are skipped, so a task directory linking back to one of
/// its own ancestors cannot send the walk into a loop. Symlinked files are kept:
/// linking a script into a task directory is an ordinary way to define a task.
fn task_directory_entry(entry: std::fs::DirEntry) -> Option<PathBuf> {
    let path = entry.path();
    (!path.is_symlink() || path.is_file()).then_some(path)
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

fn push_error(path: &Path, error: impl std::fmt::Display, discovered: &mut DiscoveredTasks) {
    discovered.errors.push(format!(
        "Failed to parse mise task definition {}: {}",
        path.display(),
        error
    ));
}

fn record_parse_error(
    path: &Path,
    error: impl std::fmt::Display,
    discovered: &mut DiscoveredTasks,
) {
    let error = error.to_string();
    push_error(path, &error, discovered);
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
    fn test_inline_tasks_survive_unsupported_includes_template() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(
            temp_dir.path().join("mise.toml"),
            "[tasks.build]\nrun = 'echo build'\n\n[task_config]\nincludes = ['{{ env.HOME }}/tasks.toml']\n",
        )
        .unwrap();

        let mut discovered = DiscoveredTasks::default();
        discover_mise_tasks(temp_dir.path(), &mut discovered);

        assert!(discovered.tasks.iter().any(|task| task.name == "build"));
        assert!(discovered
            .errors
            .iter()
            .any(|error| error.contains("only config_root is supported")));
    }

    #[test]
    #[serial]
    fn test_missing_include_target_is_recorded() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(
            temp_dir.path().join("mise.toml"),
            "[task_config]\nincludes = ['missing-tasks.toml']\n",
        )
        .unwrap();

        let mut discovered = DiscoveredTasks::default();
        discover_mise_tasks(temp_dir.path(), &mut discovered);

        let missing_path = temp_dir.path().join("missing-tasks.toml");
        let recorded = discovered
            .definitions
            .iter()
            .flat_map(|(_, files)| files)
            .find(|file| file.path == missing_path)
            .unwrap();
        assert_eq!(recorded.status, TaskFileStatus::NotFound);
    }

    #[test]
    #[serial]
    fn test_later_custom_include_wins() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(
            temp_dir.path().join("mise.toml"),
            "[task_config]\nincludes = ['first.toml', 'second.toml']\n",
        )
        .unwrap();
        fs::write(
            temp_dir.path().join("first.toml"),
            "[build]\ndescription = 'First include'\nrun = 'echo first'\n",
        )
        .unwrap();
        fs::write(
            temp_dir.path().join("second.toml"),
            "[build]\ndescription = 'Second include'\nrun = 'echo second'\n",
        )
        .unwrap();

        let mut discovered = DiscoveredTasks::default();
        discover_mise_tasks(temp_dir.path(), &mut discovered);

        let build = discovered
            .tasks
            .iter()
            .find(|task| task.name == "build")
            .unwrap();
        assert_eq!(build.description, Some("Second include".to_string()));
        assert_eq!(build.file_path, temp_dir.path().join("second.toml"));
    }

    #[test]
    #[serial]
    fn test_mise_env_config_files_follow_precedence() {
        let temp_dir = TempDir::new().unwrap();
        let config_paths = [
            "mise.ci.local.toml",
            ".mise.ci.local.toml",
            "mise.local.toml",
            ".mise.local.toml",
            "mise.ci.toml",
            ".mise.ci.toml",
            "mise.toml",
            ".mise.toml",
        ];
        for config_path in config_paths {
            fs::write(
                temp_dir.path().join(config_path),
                format!("[tasks.build]\ndescription = '{config_path}'\nrun = 'echo build'\n"),
            )
            .unwrap();
        }

        assert_eq!(
            find_mise_config_files(temp_dir.path(), Some("ci")),
            config_paths
                .iter()
                .map(|path| temp_dir.path().join(path))
                .collect::<Vec<_>>()
        );

        let mut discovered = DiscoveredTasks::default();
        discover_mise_tasks_with_env(temp_dir.path(), &mut discovered, Some("ci"));

        let build = discovered
            .tasks
            .iter()
            .find(|task| task.name == "build")
            .unwrap();
        assert_eq!(build.description, Some("mise.ci.local.toml".to_string()));
        assert_eq!(build.file_path, temp_dir.path().join("mise.ci.local.toml"));
    }

    #[cfg(unix)]
    #[test]
    #[serial]
    fn test_first_default_task_directory_searched_wins() {
        let temp_dir = TempDir::new().unwrap();
        for relative_directory in DEFAULT_FILE_TASK_DIRECTORIES_HIGH_TO_LOW {
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
        assert_eq!(task.file_path, temp_dir.path().join("mise-tasks/build"));
        assert_eq!(
            task.allowlist_path(),
            temp_dir.path().join("mise-tasks/build")
        );
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

    #[cfg(unix)]
    #[test]
    #[serial]
    fn test_symlinked_file_task_is_discovered() {
        use std::os::unix::fs::symlink;

        let temp_dir = TempDir::new().unwrap();
        let script_directory = temp_dir.path().join("scripts");
        fs::create_dir_all(&script_directory).unwrap();
        let script = script_directory.join("deploy.sh");
        fs::write(&script, "#!/bin/sh\n#MISE description='Deploy it'\n").unwrap();
        make_executable(&script);

        let task_directory = temp_dir.path().join("mise-tasks");
        fs::create_dir_all(&task_directory).unwrap();
        let link = task_directory.join("deploy");
        symlink(&script, &link).unwrap();

        let mut discovered = DiscoveredTasks::default();
        discover_mise_tasks(temp_dir.path(), &mut discovered);

        let task = discovered
            .tasks
            .iter()
            .find(|task| task.name == "deploy")
            .unwrap();
        assert_eq!(task.description, Some("Deploy it".to_string()));
        assert_eq!(task.allowlist_path(), link);
    }

    #[test]
    fn test_conf_d_tasks_are_discovered_below_their_sibling_config() {
        let temp_dir = TempDir::new().unwrap();
        let conf_d = temp_dir.path().join(".mise/conf.d");
        fs::create_dir_all(&conf_d).unwrap();
        fs::write(
            conf_d.join("10-build.toml"),
            "[tasks.build]\ndescription = 'Lower precedence'\nrun = 'echo low'\n",
        )
        .unwrap();
        fs::write(
            conf_d.join("20-extra.toml"),
            "[tasks.build]\ndescription = 'Higher precedence'\nrun = 'echo high'\n[tasks.extra]\nrun = 'echo extra'\n",
        )
        .unwrap();
        fs::write(
            temp_dir.path().join("mise.toml"),
            "[tasks.build]\ndescription = 'Root config wins'\nrun = 'echo root'\n",
        )
        .unwrap();

        let mut discovered = DiscoveredTasks::default();
        discover_mise_tasks(temp_dir.path(), &mut discovered);

        let task_names: Vec<_> = discovered
            .tasks
            .iter()
            .map(|task| task.name.as_str())
            .collect();
        assert_eq!(task_names, vec!["build", "extra"]);

        let build = discovered
            .tasks
            .iter()
            .find(|task| task.name == "build")
            .unwrap();
        assert_eq!(build.description, Some("Root config wins".to_string()));
        assert_eq!(build.file_path, temp_dir.path().join("mise.toml"));
    }

    #[test]
    fn test_conf_d_layers_alphabetically_last_over_earlier_files() {
        let temp_dir = TempDir::new().unwrap();
        let conf_d = temp_dir.path().join(".mise/conf.d");
        fs::create_dir_all(&conf_d).unwrap();
        fs::write(
            conf_d.join("10-build.toml"),
            "[tasks.build]\ndescription = 'Lower precedence'\nrun = 'echo low'\n",
        )
        .unwrap();
        fs::write(
            conf_d.join("20-build.toml"),
            "[tasks.build]\ndescription = 'Higher precedence'\nrun = 'echo high'\n",
        )
        .unwrap();

        let mut discovered = DiscoveredTasks::default();
        discover_mise_tasks(temp_dir.path(), &mut discovered);

        let build = discovered
            .tasks
            .iter()
            .find(|task| task.name == "build")
            .unwrap();
        assert_eq!(build.description, Some("Higher precedence".to_string()));
        assert_eq!(build.file_path, conf_d.join("20-build.toml"));
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
