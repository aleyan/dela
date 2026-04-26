# Test Makefile for dela Docker tests

.PHONY: test-task another-task help cd custom-exe print-args test long-running-task stderr-level-task

test-task: ## Test task for basic functionality
	@echo "Test task executed successfully"

test: ## Test task that will conflict with other test tasks
	@echo "Make test task executed successfully"

another-task: ## Another test task
	@echo "Another task executed successfully"

long-running-task: ## Long running task for testing background execution
	@echo "Starting long-running task..."
	@sleep 5
	@echo "Long-running task completed successfully"

stderr-level-task: ## Emit stderr lines for MCP logging level classification
	@printf '%s\n' 'plain stderr line' >&2
	@printf '%s\n' 'warning: this is a warning' >&2
	@printf '%s\n' 'error: this is an error' >&2

print-arg-task: ## Test task for basic functionality with args
	@echo "Argument is: $(ARG)"

print-args: ## Echo all arguments passed to the task
	@echo "Arguments passed to print-args: $(subst ',,$(ARGS))"

cd: ## Task that will be shadowed by shell builtin
	@echo "This task is shadowed by the cd shell builtin"

custom-exe: ## Task that will be shadowed by PATH executable
	@echo "This task is shadowed by a PATH executable"

help: ## Show this help message
	@echo "Available tasks:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}' 
