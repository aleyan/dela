#[cfg(test)]
mod tests {
    #[test]
    fn test_execute_task_with_disambiguated_name() {
        let mut task = Task {
            name: "test".to_string(),
            source: PathBuf::from("/path/to/Makefile"),
            runner: TaskRunner::Make,
            args: vec![],
            shadowed_by: Some(ShadowType::PathExecutable("/bin/test".to_string())),
            disambiguated_name: Some("test-m".to_string()),
        };

        // Mock the executor
        let mut mock_executor = MockTaskExecutor::new();
        
        // Expect execution with the original task name, not the disambiguated one
        mock_executor.expect_execute().times(1).returning(|task| {
            assert_eq!(task.name, "test"); // We still execute with the original name
            assert_eq!(task.disambiguated_name, Some("test-m".to_string())); // But it has a disambiguated name
            assert!(task.shadowed_by.is_some()); // And it is shadowed
            Ok(())
        });

        let executor = CommandExecutor::new(mock_executor);
        
        // Execute using the disambiguated name
        let result = executor.execute_task_by_name(&mut DiscoveredTasks::new(), "test-m", &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_task_by_either_name() {
        let mut discovered_tasks = DiscoveredTasks::new();
        
        // Add a shadowed task with a disambiguated name
        let task = Task {
            name: "grep".to_string(),
            source: PathBuf::from("/path/to/Makefile"),
            runner: TaskRunner::Make,
            args: vec![],
            shadowed_by: Some(ShadowType::PathExecutable("/bin/grep".to_string())),
            disambiguated_name: Some("grep-m".to_string()),
        };
        
        discovered_tasks.add_task(task);
        
        // Mock the executor
        let mut mock_executor = MockTaskExecutor::new();
        
        // Expect two executions - one by original name, one by disambiguated name
        mock_executor.expect_execute().times(2).returning(|task| {
            assert_eq!(task.name, "grep"); // Original name used for execution
            Ok(())
        });

        let executor = CommandExecutor::new(mock_executor);
        
        // Execute using the original name
        let result1 = executor.execute_task_by_name(&mut discovered_tasks, "grep", &[]);
        assert!(result1.is_ok());
        
        // Execute using the disambiguated name
        let result2 = executor.execute_task_by_name(&mut discovered_tasks, "grep-m", &[]);
        assert!(result2.is_ok());
    }

    #[test]
    fn test_execute_task_ambiguous_and_shadowed() {
        let mut discovered_tasks = DiscoveredTasks::new();
        
        // Add two tasks with the same name but from different sources
        let task1 = Task {
            name: "test".to_string(),
            source: PathBuf::from("/path/to/Makefile"),
            runner: TaskRunner::Make,
            args: vec![],
            shadowed_by: Some(ShadowType::PathExecutable("/bin/test".to_string())),
            disambiguated_name: Some("test-m".to_string()),
        };
        
        let task2 = Task {
            name: "test".to_string(),
            source: PathBuf::from("/path/to/package.json"),
            runner: TaskRunner::Npm,
            args: vec![],
            shadowed_by: None,
            disambiguated_name: Some("test-npm".to_string()),
        };
        
        discovered_tasks.add_task(task1);
        discovered_tasks.add_task(task2);
        
        // Mock the executor
        let mut mock_executor = MockTaskExecutor::new();
        
        // Expect execution with the specific task
        mock_executor.expect_execute().times(2).returning(|task| {
            if task.runner == TaskRunner::Make {
                assert_eq!(task.disambiguated_name, Some("test-m".to_string()));
            } else if task.runner == TaskRunner::Npm {
                assert_eq!(task.disambiguated_name, Some("test-npm".to_string()));
            } else {
                panic!("Unexpected task runner");
            }
            Ok(())
        });

        let executor = CommandExecutor::new(mock_executor);
        
        // Execute using the disambiguated names
        let result1 = executor.execute_task_by_name(&mut discovered_tasks, "test-m", &[]);
        assert!(result1.is_ok());
        
        let result2 = executor.execute_task_by_name(&mut discovered_tasks, "test-npm", &[]);
        assert!(result2.is_ok());
        
        // Executing by the original name should fail due to ambiguity
        let result3 = executor.execute_task_by_name(&mut discovered_tasks, "test", &[]);
        assert!(result3.is_err());
        assert!(format!("{}", result3.unwrap_err()).contains("ambiguous"));
    }
} 