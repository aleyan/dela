use std::path::{Path, PathBuf};

use crate::types::{DiscoveredTasks, Task, TaskRunner, TaskDefinitionFile, TaskFileStatus};

/// Discovers tasks in the given directory
pub fn discover_tasks(dir: &Path) -> DiscoveredTasks {
    let mut discovered = DiscoveredTasks::default();
    
    // TODO(DTKT-4): Implement Makefile parser
    if let Err(e) = discover_makefile_tasks(dir, &mut discovered) {
        discovered.errors.push(format!("Error parsing Makefile: {}", e));
        if let Some(makefile) = &mut discovered.definitions.makefile {
            makefile.status = TaskFileStatus::ParseError(e);
        }
    }

    // TODO(DTKT-5): Implement package.json parser
    if let Err(e) = discover_npm_tasks(dir, &mut discovered) {
        discovered.errors.push(format!("Error parsing package.json: {}", e));
        if let Some(package_json) = &mut discovered.definitions.package_json {
            package_json.status = TaskFileStatus::ParseError(e);
        }
    }

    // TODO(DTKT-6): Implement pyproject.toml parser
    if let Err(e) = discover_python_tasks(dir, &mut discovered) {
        discovered.errors.push(format!("Error parsing pyproject.toml: {}", e));
        if let Some(pyproject) = &mut discovered.definitions.pyproject_toml {
            pyproject.status = TaskFileStatus::ParseError(e);
        }
    }

    discovered
}

fn discover_makefile_tasks(dir: &Path, discovered: &mut DiscoveredTasks) -> Result<(), String> {
    let makefile_path = dir.join("Makefile");
    
    if !makefile_path.exists() {
        discovered.definitions.makefile = Some(TaskDefinitionFile {
            path: makefile_path,
            runner: TaskRunner::Make,
            status: TaskFileStatus::NotFound,
        });
        return Ok(());
    }

    discovered.definitions.makefile = Some(TaskDefinitionFile {
        path: makefile_path.clone(),
        runner: TaskRunner::Make,
        status: TaskFileStatus::NotImplemented,
    });

    // Placeholder until DTKT-4 is implemented
    discovered.tasks.push(Task {
        name: "build".to_string(),
        file_path: makefile_path,
        runner: TaskRunner::Make,
        source_name: "build".to_string(),
        description: None,
    });

    Ok(())
}

fn discover_npm_tasks(dir: &Path, discovered: &mut DiscoveredTasks) -> Result<(), String> {
    let package_json = dir.join("package.json");
    
    if !package_json.exists() {
        discovered.definitions.package_json = Some(TaskDefinitionFile {
            path: package_json,
            runner: TaskRunner::Npm,
            status: TaskFileStatus::NotFound,
        });
        return Ok(());
    }

    discovered.definitions.package_json = Some(TaskDefinitionFile {
        path: package_json,
        runner: TaskRunner::Npm,
        status: TaskFileStatus::NotImplemented,
    });

    // Placeholder until DTKT-5 is implemented
    Ok(())
}

fn discover_python_tasks(dir: &Path, discovered: &mut DiscoveredTasks) -> Result<(), String> {
    let pyproject_toml = dir.join("pyproject.toml");
    
    if !pyproject_toml.exists() {
        discovered.definitions.pyproject_toml = Some(TaskDefinitionFile {
            path: pyproject_toml,
            runner: TaskRunner::Python,
            status: TaskFileStatus::NotFound,
        });
        return Ok(());
    }

    discovered.definitions.pyproject_toml = Some(TaskDefinitionFile {
        path: pyproject_toml,
        runner: TaskRunner::Python,
        status: TaskFileStatus::NotImplemented,
    });

    // Placeholder until DTKT-6 is implemented
    Ok(())
}

// TODO(DTKT-52): Add trait for plugin-based task discovery 