use crate::parsers::parse_pom_xml;
use crate::task_discovery::support::{handle_discovery_error, handle_discovery_success};
use crate::task_discovery::{DiscoveredTasks, TaskDiscovery};
use crate::types::TaskDefinitionType;
use std::path::Path;

pub(crate) struct MavenDiscovery;

impl TaskDiscovery for MavenDiscovery {
    fn discover(&self, dir: &Path, discovered: &mut DiscoveredTasks) {
        let _ = discover_maven_tasks(dir, discovered);
    }
}

fn discover_maven_tasks(dir: &Path, discovered: &mut DiscoveredTasks) -> anyhow::Result<()> {
    let pom_path = dir.join("pom.xml");
    if !pom_path.exists() {
        return Ok(());
    }

    match parse_pom_xml(&pom_path) {
        Ok(tasks) => {
            handle_discovery_success(tasks, pom_path, TaskDefinitionType::MavenPom, discovered);
            Ok(())
        }
        Err(error) => {
            handle_discovery_error(&error, pom_path.clone(), TaskDefinitionType::MavenPom, discovered);
            Err(anyhow::Error::new(error).context(format!("Failed to parse {}", pom_path.display())))
        }
    }
}
