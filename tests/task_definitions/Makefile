# Test Makefile for dela Docker tests

.PHONY: test-task another-task help cd custom-exe print-args test

test-task: ## Test task for basic functionality
	@echo "Test task executed successfully"

test: ## Test task that will conflict with other test tasks
	@echo "Make test task executed successfully"

another-task: ## Another test task
	@echo "Another task executed successfully"

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