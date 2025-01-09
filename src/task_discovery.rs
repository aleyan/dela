use std::path::Path;
use crate::types::{DiscoveredTasks, TaskFileStatus, TaskDefinitionFile, TaskRunner};

use crate::parse_makefile;

/// Discovers tasks in the given directory
pub fn discover_tasks(dir: &Path) -> DiscoveredTasks {
    let mut discovered = DiscoveredTasks::default();
    
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
        discovered.definitions.makefile = Some(parse_makefile::create_definition(
            &makefile_path,
            TaskFileStatus::NotFound,
        ));
        return Ok(());
    }

    match parse_makefile::parse(&makefile_path) {
        Ok(tasks) => {
            discovered.definitions.makefile = Some(parse_makefile::create_definition(
                &makefile_path,
                TaskFileStatus::Parsed,
            ));
            discovered.tasks.extend(tasks);
            Ok(())
        }
        Err(e) => {
            discovered.definitions.makefile = Some(parse_makefile::create_definition(
                &makefile_path,
                TaskFileStatus::ParseError(e.clone()),
            ));
            Err(e)
        }
    }
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

    // TODO(DTKT-5): Implement package.json parser
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

    // TODO(DTKT-6): Implement pyproject.toml parser
    Ok(())
}

// TODO(DTKT-52): Add trait for plugin-based task discovery 