use roxmltree::{Document, Node};
use std::fs;
use std::path::Path;

use crate::types::{Task, TaskDefinitionType, TaskRunner};

/// Parse a Maven pom.xml file and return a list of tasks
pub fn parse(file_path: &Path) -> Result<Vec<Task>, String> {
    // Read the file
    let content =
        fs::read_to_string(file_path).map_err(|e| format!("Error reading pom.xml file: {}", e))?;

    // Parse the XML
    let doc = Document::parse(&content).map_err(|e| format!("Error parsing pom.xml: {}", e))?;

    let root = doc.root_element();

    // Extract tasks from the default Maven goals and profiles
    let mut tasks = Vec::new();

    // Add default Maven goals
    add_default_maven_goals(&mut tasks, file_path);

    // Add custom goals from profiles
    add_profile_tasks(&mut tasks, root, file_path)?;

    // Parse plugins to find custom goals
    add_plugin_tasks(&mut tasks, root, file_path)?;

    Ok(tasks)
}

/// Add default Maven lifecycle goals to the tasks
fn add_default_maven_goals(tasks: &mut Vec<Task>, file_path: &Path) {
    // Maven default lifecycle phases
    let default_goals = [
        "clean", "validate", "compile", "test", "package", "verify", "install", "deploy", "site",
    ];

    for goal in default_goals.iter() {
        tasks.push(Task {
            name: goal.to_string(),
            file_path: file_path.to_path_buf(),
            definition_type: TaskDefinitionType::MavenPom,
            runner: TaskRunner::Maven,
            source_name: goal.to_string(),
            description: Some(format!("Maven {} phase", goal)),
            shadowed_by: None,
            disambiguated_name: None,
        });
    }
}

/// Add tasks from Maven profiles
fn add_profile_tasks(tasks: &mut Vec<Task>, root: Node, file_path: &Path) -> Result<(), String> {
    // Find <profiles> section
    if let Some(profiles_node) = root.children().find(|n| n.has_tag_name("profiles")) {
        // Iterate over each profile
        for profile in profiles_node
            .children()
            .filter(|n| n.has_tag_name("profile"))
        {
            if let Some(id_node) = profile.children().find(|n| n.has_tag_name("id")) {
                let profile_id = id_node.text().unwrap_or("unknown").to_string();

                // Add the profile as a task
                tasks.push(Task {
                    name: format!("profile:{}", profile_id),
                    file_path: file_path.to_path_buf(),
                    definition_type: TaskDefinitionType::MavenPom,
                    runner: TaskRunner::Maven,
                    source_name: profile_id.clone(),
                    description: Some(format!("Maven profile {}", profile_id)),
                    shadowed_by: None,
                    disambiguated_name: None,
                });
            }
        }
    }

    Ok(())
}

/// Add tasks from Maven plugins
fn add_plugin_tasks(tasks: &mut Vec<Task>, root: Node, file_path: &Path) -> Result<(), String> {
    // Find <build> section and then <plugins>
    if let Some(build_node) = root.children().find(|n| n.has_tag_name("build")) {
        if let Some(plugins_node) = build_node.children().find(|n| n.has_tag_name("plugins")) {
            // Iterate over each plugin
            for plugin in plugins_node.children().filter(|n| n.has_tag_name("plugin")) {
                // Get plugin artifact ID
                let artifact_id = plugin
                    .children()
                    .find(|n| n.has_tag_name("artifactId"))
                    .and_then(|n| n.text())
                    .unwrap_or("unknown")
                    .to_string();

                // Find executions
                if let Some(executions_node) =
                    plugin.children().find(|n| n.has_tag_name("executions"))
                {
                    for execution in executions_node
                        .children()
                        .filter(|n| n.has_tag_name("execution"))
                    {
                        // Get execution ID
                        let exec_id = execution
                            .children()
                            .find(|n| n.has_tag_name("id"))
                            .and_then(|n| n.text())
                            .unwrap_or("default")
                            .to_string();

                        // Get goals
                        if let Some(goals_node) =
                            execution.children().find(|n| n.has_tag_name("goals"))
                        {
                            for goal in goals_node.children().filter(|n| n.has_tag_name("goal")) {
                                if let Some(goal_text) = goal.text() {
                                    let task_name = format!("{}:{}", artifact_id, goal_text);
                                    tasks.push(Task {
                                        name: task_name.clone(),
                                        file_path: file_path.to_path_buf(),
                                        definition_type: TaskDefinitionType::MavenPom,
                                        runner: TaskRunner::Maven,
                                        source_name: task_name,
                                        description: Some(format!(
                                            "Maven plugin goal {} (execution: {})",
                                            goal_text, exec_id
                                        )),
                                        shadowed_by: None,
                                        disambiguated_name: None,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
