use crate::task_discovery::DiscoveredTasks;
use crate::types::{Task, TaskRunner};
use std::collections::{HashMap, HashSet};

const MIN_PREFIX_LEN: usize = 3;

pub fn process_task_disambiguation(discovered: &mut DiscoveredTasks) {
    let mut task_name_counts: HashMap<String, usize> = HashMap::new();
    let mut tasks_by_name: HashMap<String, Vec<usize>> = HashMap::new();

    for (index, task) in discovered.tasks.iter().enumerate() {
        *task_name_counts.entry(task.name.clone()).or_insert(0) += 1;
        tasks_by_name
            .entry(task.name.clone())
            .or_default()
            .push(index);
    }

    discovered.task_name_counts = task_name_counts.clone();

    for (name, count) in &task_name_counts {
        if *count <= 1 {
            continue;
        }

        let task_indices = tasks_by_name
            .get(name)
            .expect("task collision indexes should exist");
        let mut used_prefixes = HashSet::new();

        for &index in task_indices {
            let task = &mut discovered.tasks[index];
            let runner_prefix = generate_runner_prefix(&task.runner, &used_prefixes);
            used_prefixes.insert(runner_prefix.clone());
            task.disambiguated_name = Some(format!("{}-{}", task.name, runner_prefix));
        }
    }

    for task in &mut discovered.tasks {
        if task.disambiguated_name.is_some() {
            continue;
        }

        if task.shadowed_by.is_some() {
            let runner_prefix = generate_runner_prefix(&task.runner, &HashSet::new());
            task.disambiguated_name = Some(format!("{}-{}", task.name, runner_prefix));
        }
    }
}

fn generate_runner_prefix(runner: &TaskRunner, used_prefixes: &HashSet<String>) -> String {
    let short_name = runner.short_name().to_lowercase();
    generate_prefix_from_short_name(&short_name, used_prefixes)
}

fn generate_prefix_from_short_name(short_name: &str, used_prefixes: &HashSet<String>) -> String {
    let single_char = short_name
        .chars()
        .next()
        .expect("runner short names are never empty")
        .to_string();
    if !used_prefixes.contains(&single_char) {
        return single_char;
    }

    let short_name_len = short_name.chars().count();
    let prefix_length = std::cmp::min(MIN_PREFIX_LEN, short_name_len);
    let mut prefix = short_name.chars().take(prefix_length).collect::<String>();
    if !used_prefixes.contains(&prefix) {
        return prefix;
    }

    for length in (prefix_length + 1)..=short_name_len {
        prefix = short_name.chars().take(length).collect::<String>();
        if !used_prefixes.contains(&prefix) {
            return prefix;
        }
    }

    let mut index = 1;
    loop {
        let numbered_prefix = format!("{}{}", short_name, index);
        if !used_prefixes.contains(&numbered_prefix) {
            return numbered_prefix;
        }
        index += 1;
    }
}

pub fn is_task_ambiguous(discovered: &DiscoveredTasks, task_name: &str) -> bool {
    discovered
        .task_name_counts
        .get(task_name)
        .is_some_and(|&count| count > 1)
}

#[allow(dead_code)]
pub fn get_disambiguated_task_names(discovered: &DiscoveredTasks, task_name: &str) -> Vec<String> {
    discovered
        .tasks
        .iter()
        .filter(|task| task.name == task_name)
        .filter_map(|task| task.disambiguated_name.clone())
        .collect()
}

pub fn get_matching_tasks<'a>(discovered: &'a DiscoveredTasks, task_name: &str) -> Vec<&'a Task> {
    discovered
        .tasks
        .iter()
        .filter(|task| {
            task.name == task_name
                || task
                    .disambiguated_name
                    .as_ref()
                    .is_some_and(|name| name == task_name)
        })
        .collect()
}

pub fn format_ambiguous_task_error(task_name: &str, matching_tasks: &[&Task]) -> String {
    let mut message = format!("Multiple tasks named '{}' found. Use one of:\n", task_name);

    for task in matching_tasks {
        let display_name = task.disambiguated_name.as_deref().unwrap_or(&task.name);
        message.push_str(&format!(
            "  • {} ({} from {})\n",
            display_name,
            task.runner.short_name(),
            task.definition_path().display()
        ));
    }

    message.push_str("Please use the specific task name with its suffix to disambiguate.");
    message
}

#[cfg(test)]
mod tests {
    use super::{format_ambiguous_task_error, generate_prefix_from_short_name};
    use crate::types::{Task, TaskDefinitionType, TaskRunner};
    use std::collections::HashSet;
    use std::path::PathBuf;

    #[test]
    fn generate_prefix_handles_multibyte_runner_names() {
        let used_prefixes = HashSet::from(["å".to_string(), "ång".to_string(), "ångs".to_string()]);

        assert_eq!(
            generate_prefix_from_short_name("ångström", &used_prefixes),
            "ångst".to_string()
        );
    }

    #[test]
    fn format_ambiguous_task_error_includes_tasks_without_disambiguated_names() {
        let make_task = Task {
            name: "test".to_string(),
            file_path: PathBuf::from("/tmp/Makefile"),
            definition_path: None,
            definition_type: TaskDefinitionType::Makefile,
            runner: TaskRunner::Make,
            source_name: "test".to_string(),
            description: None,
            shadowed_by: None,
            disambiguated_name: None,
        };
        let npm_task = Task {
            name: "test".to_string(),
            file_path: PathBuf::from("/tmp/package.json"),
            definition_path: None,
            definition_type: TaskDefinitionType::PackageJson,
            runner: TaskRunner::NodeNpm,
            source_name: "test".to_string(),
            description: None,
            shadowed_by: None,
            disambiguated_name: Some("test-npm".to_string()),
        };

        let error = format_ambiguous_task_error("test", &[&make_task, &npm_task]);

        assert!(error.contains("  • test (make from /tmp/Makefile)"));
        assert!(error.contains("  • test-npm (npm from /tmp/package.json)"));
    }
}
