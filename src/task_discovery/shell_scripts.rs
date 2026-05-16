use crate::task_discovery::{DiscoveredTasks, TaskDiscovery};
use crate::task_shadowing::check_shadowing;
use crate::types::{Task, TaskDefinitionType, TaskRunner};
use std::fs;
use std::path::Path;

pub(crate) struct ShellScriptDiscovery;

impl TaskDiscovery for ShellScriptDiscovery {
    fn discover(&self, dir: &Path, discovered: &mut DiscoveredTasks) {
        discover_shell_script_tasks(dir, discovered);
    }
}

fn discover_shell_script_tasks(dir: &Path, discovered: &mut DiscoveredTasks) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file()
                && let Some(extension) = path.extension()
                && extension == "sh"
            {
                let name = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let source_name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                discovered.tasks.push(Task {
                    name: name.clone(),
                    file_path: path,
                    definition_path: None,
                    definition_type: TaskDefinitionType::ShellScript,
                    runner: TaskRunner::ShellScript,
                    source_name,
                    description: None,
                    shadowed_by: check_shadowing(&name),
                    disambiguated_name: None,
                });
            }
        }
    }
}
