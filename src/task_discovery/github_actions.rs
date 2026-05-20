use crate::composed_paths::ComposedDefinitionSource;
use crate::parsers::parse_github_actions;
use crate::task_discovery::{DiscoveredTasks, TaskDiscovery};
use crate::task_shadowing::check_shadowing;
use crate::types::{TaskDefinitionFile, TaskDefinitionType, TaskFileStatus};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) struct GithubActionsDiscovery;

impl TaskDiscovery for GithubActionsDiscovery {
    fn discover(&self, dir: &Path, discovered: &mut DiscoveredTasks) {
        let _ = discover_github_actions_tasks(dir, discovered);
    }
}

fn discover_github_actions_tasks(
    dir: &Path,
    discovered: &mut DiscoveredTasks,
) -> anyhow::Result<()> {
    let mut workflow_files = Vec::new();

    let workflows_dir = dir.join(".github").join("workflows");
    if workflows_dir.exists() && workflows_dir.is_dir() {
        match fs::read_dir(&workflows_dir) {
            Ok(entries) => {
                let files: Vec<PathBuf> = entries
                    .filter_map(Result::ok)
                    .map(|entry| entry.path())
                    .filter(|path| {
                        path.extension()
                            .is_some_and(|ext| ext == "yml" || ext == "yaml")
                    })
                    .collect();
                workflow_files.extend(files);
            }
            Err(error) => {
                discovered.errors.push(format!(
                    "Failed to read .github/workflows directory: {}",
                    error
                ));
            }
        }
    }

    for filename in &[
        "workflow.yml",
        "workflow.yaml",
        ".github/workflow.yml",
        ".github/workflow.yaml",
    ] {
        let file_path = dir.join(filename);
        if file_path.exists() && file_path.is_file() {
            workflow_files.push(file_path);
        }
    }

    for custom_dir in &["workflows", "custom/workflows", ".gitlab/workflows"] {
        let custom_path = dir.join(custom_dir);
        if custom_path.exists()
            && custom_path.is_dir()
            && let Ok(entries) = fs::read_dir(&custom_path)
        {
            let files: Vec<PathBuf> = entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| {
                    path.extension()
                        .is_some_and(|ext| ext == "yml" || ext == "yaml")
                })
                .collect();
            workflow_files.extend(files);
        }
    }

    if workflow_files.is_empty() {
        return Ok(());
    }

    let mut all_tasks = Vec::new();
    let mut errors = Vec::new();
    let workflows_parent = dir.join(".github").join("workflows");

    for file_path in workflow_files {
        match parse_github_actions(&file_path) {
            Ok(mut tasks) => {
                let source =
                    ComposedDefinitionSource::composed(workflows_parent.clone(), file_path);
                for task in &mut tasks {
                    source.apply_to_task(task);
                    task.shadowed_by = check_shadowing(&task.name);
                }
                all_tasks.extend(tasks);
            }
            Err(error) => errors.push(format!(
                "Failed to parse workflow file {:?}: {}",
                file_path, error
            )),
        }
    }

    if !errors.is_empty() {
        discovered.errors.extend(errors);
    }

    if !all_tasks.is_empty() {
        discovered.definitions.insert(TaskDefinitionFile {
            path: workflows_parent,
            definition_type: TaskDefinitionType::GitHubActions,
            status: TaskFileStatus::Parsed,
        });
        discovered.tasks.extend(all_tasks);
    }

    Ok(())
}
