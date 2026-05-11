use crate::parsers::parse_cmake;
use crate::task_discovery::support::{handle_discovery_error, handle_discovery_success};
use crate::task_discovery::{DiscoveredTasks, TaskDiscovery};
use crate::types::TaskDefinitionType;
use std::path::Path;

pub(crate) struct CmakeDiscovery;

impl TaskDiscovery for CmakeDiscovery {
    fn discover(&self, dir: &Path, discovered: &mut DiscoveredTasks) {
        let _ = discover_cmake_tasks(dir, discovered);
    }
}

fn discover_cmake_tasks(dir: &Path, discovered: &mut DiscoveredTasks) -> Result<(), String> {
    let cmake_path = dir.join("CMakeLists.txt");
    if !cmake_path.exists() {
        return Ok(());
    }

    match parse_cmake::parse(&cmake_path) {
        Ok(tasks) => {
            handle_discovery_success(tasks, cmake_path, TaskDefinitionType::CMake, discovered);
            Ok(())
        }
        Err(error) => {
            handle_discovery_error(error, cmake_path, TaskDefinitionType::CMake, discovered);
            Err("Error parsing CMakeLists.txt".to_string())
        }
    }
}
