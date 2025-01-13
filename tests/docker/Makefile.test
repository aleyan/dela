# Test Makefile for dela Docker tests

.PHONY: test-task another-task help

test-task: ## Test task for basic functionality
	@echo "Test task executed successfully"

another-task: ## Another test task
	@echo "Another task executed successfully"

help: ## Show this help message
	@echo "Available tasks:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-20s\033[0m %s\n", $$1, $$2}' 