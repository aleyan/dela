use crate::parsers::parse_cmake;
use crate::task_discovery::support::{
    handle_discovery_error, handle_discovery_success, set_definition,
};
use crate::task_discovery::{DiscoveredTasks, TaskDiscovery};
use crate::types::{TaskDefinitionFile, TaskDefinitionType, TaskFileStatus};
use std::path::Path;

pub(crate) struct CmakeDiscovery;

impl TaskDiscovery for CmakeDiscovery {
    fn discover(&self, dir: &Path, discovered: &mut DiscoveredTasks) {
        let _ = discover_cmake_tasks(dir, discovered);
    }
}

fn discover_cmake_tasks(dir: &Path, discovered: &mut DiscoveredTasks) -> anyhow::Result<()> {
    let cmake_path = dir.join("CMakeLists.txt");
    if !cmake_path.exists() {
        set_definition(
            discovered,
            TaskDefinitionFile {
                path: cmake_path,
                definition_type: TaskDefinitionType::CMake,
                status: TaskFileStatus::NotFound,
            },
        );
        return Ok(());
    }

    match parse_cmake::parse(&cmake_path) {
        Ok(tasks) => {
            handle_discovery_success(tasks, cmake_path, TaskDefinitionType::CMake, discovered);
            Ok(())
        }
        Err(error) => {
            handle_discovery_error(error, cmake_path, TaskDefinitionType::CMake, discovered);
            Err(anyhow::anyhow!("Error parsing CMakeLists.txt"))
        }
    }
}
