use crate::parsers::parse_justfile;
use crate::task_discovery::support::{handle_discovery_error, handle_discovery_success};
use crate::task_discovery::{DiscoveredTasks, TaskDiscovery};
use crate::types::{TaskDefinitionFile, TaskDefinitionType, TaskFileStatus};
use std::path::Path;

pub(crate) struct JustfileDiscovery;

impl TaskDiscovery for JustfileDiscovery {
    fn discover(&self, dir: &Path, discovered: &mut DiscoveredTasks) {
        let _ = discover_justfile_tasks(dir, discovered);
    }
}

fn discover_justfile_tasks(dir: &Path, discovered: &mut DiscoveredTasks) -> anyhow::Result<()> {
    let possible_justfiles = ["Justfile", "justfile", ".justfile"];
    let justfile_path = possible_justfiles
        .iter()
        .map(|filename| dir.join(filename))
        .find(|path| path.exists());

    let default_path = dir.join("Justfile");
    if let Some(justfile_path) = justfile_path {
        match parse_justfile::parse(&justfile_path) {
            Ok(tasks) => {
                handle_discovery_success(
                    tasks,
                    justfile_path,
                    TaskDefinitionType::Justfile,
                    discovered,
                );
            }
            Err(error) => {
                handle_discovery_error(
                    error,
                    justfile_path,
                    TaskDefinitionType::Justfile,
                    discovered,
                );
            }
        }
    } else {
        discovered.definitions.justfile = Some(TaskDefinitionFile {
            path: default_path,
            definition_type: TaskDefinitionType::Justfile,
            status: TaskFileStatus::NotFound,
        });
    }

    Ok(())
}
