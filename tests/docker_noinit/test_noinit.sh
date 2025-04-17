#!/bin/zsh
set -e

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

# Prevent broken pipe errors
exec 2>&1

echo "Starting non-initialized shell integration tests..."

# Test 1: Basic dela list without shell integration
echo "\nTest 1: Testing dela list without shell integration"
if dela list | grep -q "npm-test" && dela list | grep -q "npm-build"; then
    echo "${GREEN}✓ dela list shows npm tasks${NC}"
else
    echo "${RED}✗ dela list failed to show npm tasks${NC}"
    exit 1
fi

# Test 2: Test get-command functionality for npm
echo "\nTest 2: Testing get-command for npm"
output=$(dela get-command npm-test 2>&1)
if echo "$output" | grep -q "npm run npm-test"; then
    echo "${GREEN}✓ get-command returns correct npm command${NC}"
else
    echo "${RED}✗ get-command failed${NC}"
    echo "Full output: $output"
    exit 1
fi

# Test 3: Test allow-command interactive functionality for npm
echo "\nTest 3: Testing allow-command interactive functionality for npm"
echo "Initial allowlist contents:"
cat /home/testuser/.dela/allowlist.toml

# Test interactive allow-command with option 2 (Allow this task)
echo "\nTesting interactive allow-command with 'Allow this task' option:"
echo "2" | dela allow-command npm-build >/dev/null 2>&1

echo "\nAllowlist contents after allow-command:"
cat /home/testuser/.dela/allowlist.toml

# Verify the allowlist was updated with the specific task
if grep -q "npm-build" /home/testuser/.dela/allowlist.toml; then
    echo "${GREEN}✓ npm-build task was added to allowlist via interactive mode${NC}"
else
    echo "${RED}✗ npm-build task was not added to allowlist via interactive mode${NC}"
    exit 1
fi

# Test 4: Basic dela list for Taskfile tasks
echo "\nTest 4: Testing dela list for Taskfile tasks"
if dela list | grep -q "task-test" && dela list | grep -q "task-build" && dela list | grep -q "task-deps"; then
    echo "${GREEN}✓ dela list shows Taskfile tasks${NC}"
else
    echo "${RED}✗ dela list failed to show Taskfile tasks${NC}"
    exit 1
fi

# Test 5: Test get-command functionality for Taskfile
echo "\nTest 5: Testing get-command for Taskfile"
output=$(dela get-command task-test 2>&1)
if echo "$output" | grep -q "task task-test"; then
    echo "${GREEN}✓ get-command returns correct task command${NC}"
else
    echo "${RED}✗ get-command failed for Taskfile task${NC}"
    echo "Full output: $output"
    exit 1
fi

# Test 6: Test allow-command interactive functionality for Taskfile
echo "\nTest 6: Testing allow-command interactive functionality for Taskfile"
echo "Initial allowlist contents:"
cat /home/testuser/.dela/allowlist.toml

# Test interactive allow-command with option 2 (Allow this task)
echo "\nTesting interactive allow-command with 'Allow this task' option:"
echo "2" | dela allow-command task-build >/dev/null 2>&1 || {
    echo "${RED}✗ allow-command failed for Taskfile task${NC}"
    exit 1
}

echo "\nAllowlist contents after allow-command:"
cat /home/testuser/.dela/allowlist.toml

# Verify the allowlist was updated with the specific task
if grep -q "task-build" /home/testuser/.dela/allowlist.toml; then
    echo "${GREEN}✓ task-build task was added to allowlist via interactive mode${NC}"
else
    echo "${RED}✗ task-build task was not added to allowlist via interactive mode${NC}"
    exit 1
fi

# Verify the task was added with Task scope (not File or Directory)
if grep -q "scope = \"Task\"" /home/testuser/.dela/allowlist.toml; then
    echo "${GREEN}✓ Task scope was correctly set via interactive mode${NC}"
else
    echo "${RED}✗ Task scope was not correctly set via interactive mode${NC}"
    exit 1
fi

# Test 7: Basic dela list for Maven tasks
echo "\nTest 7: Testing dela list for Maven tasks"
if dela list | grep -q "clean" && dela list | grep -q "compile" && dela list | grep -q "profile:dev"; then
    echo "${GREEN}✓ dela list shows Maven tasks${NC}"
else
    echo "${RED}✗ dela list failed to show Maven tasks${NC}"
    exit 1
fi

# Test 8: Test get-command functionality for Maven
echo "\nTest 8: Testing get-command for Maven"
output=$(dela get-command compile 2>&1)
if echo "$output" | grep -q "mvn compile"; then
    echo "${GREEN}✓ get-command returns correct Maven command${NC}"
else
    echo "${RED}✗ get-command failed for Maven task${NC}"
    echo "Full output: $output"
    exit 1
fi

# Test 9: Test Maven plugin goal
echo "\nTest 9: Testing Maven plugin goal"
if dela list | grep -q "maven-compiler-plugin:compile"; then
    echo "${GREEN}✓ dela list shows Maven plugin goals${NC}"
else
    echo "${RED}✗ dela list failed to show Maven plugin goals${NC}"
    exit 1
fi

# Test 10: Test allow-command interactive functionality for Maven
echo "\nTest 10: Testing allow-command interactive functionality for Maven"
echo "Initial allowlist contents:"
cat /home/testuser/.dela/allowlist.toml

# Test interactive allow-command with option 2 (Allow this task)
echo "\nTesting interactive allow-command with 'Allow this task' option:"
echo "Running: dela allow-command profile:dev"
echo "2" | dela allow-command profile:dev
RESULT=$?
if [ $RESULT -ne 0 ]; then
    echo "${RED}✗ allow-command failed for Maven task with exit code: $RESULT${NC}"
    echo "Available tasks:"
    dela list | grep -i profile || true
    exit 1
fi

echo "\nAllowlist contents after allow-command:"
cat /home/testuser/.dela/allowlist.toml

# Verify the allowlist was updated with the specific task
if grep -q "profile:dev" /home/testuser/.dela/allowlist.toml; then
    echo "${GREEN}✓ Maven profile:dev task was added to allowlist via interactive mode${NC}"
else
    echo "${RED}✗ Maven profile:dev task was not added to allowlist via interactive mode${NC}"
    exit 1
fi

# Test 11: Basic dela list for Gradle tasks (Groovy DSL)
echo "\nTest 11: Testing dela list for Gradle tasks (Groovy DSL)"
if dela list | grep -q "gradleTest" && dela list | grep -q "gradleBuild" && dela list | grep -q "build"; then
    echo "${GREEN}✓ dela list shows Gradle tasks from build.gradle${NC}"
else
    echo "${RED}✗ dela list failed to show Gradle tasks from build.gradle${NC}"
    exit 1
fi

# Test 12: Test get-command functionality for Gradle (Groovy DSL)
echo "\nTest 12: Testing get-command for Gradle (Groovy DSL)"
output=$(dela get-command gradleTest 2>&1)
if echo "$output" | grep -q "gradle gradleTest"; then
    echo "${GREEN}✓ get-command returns correct Gradle command${NC}"
else
    echo "${RED}✗ get-command failed for Gradle task${NC}"
    echo "Full output: $output"
    exit 1
fi

# Test 13: Basic dela list for Gradle tasks (Kotlin DSL)
echo "\nTest 13: Testing dela list for Gradle tasks (Kotlin DSL)"
echo "Checking available tasks from build.gradle.kts:"
dela list

if dela list | grep -q "compileKotlin"; then
    echo "${GREEN}✓ dela list shows Gradle tasks from Kotlin sources${NC}"
else
    echo "${RED}✗ dela list failed to show Gradle tasks from Kotlin sources${NC}"
    echo "Looking for any Kotlin tasks:"
    dela list | grep -i kotlin || true
    exit 1
fi

# Test 14: Test get-command functionality for Gradle (Kotlin DSL)
echo "\nTest 14: Testing get-command for Gradle (Kotlin DSL)"
output=$(dela get-command compileKotlin 2>&1)
if echo "$output" | grep -q "gradle compileKotlin"; then
    echo "${GREEN}✓ get-command returns correct Gradle command for Kotlin DSL task${NC}"
else
    echo "${RED}✗ get-command failed for Gradle Kotlin DSL task${NC}"
    echo "Full output: $output"
    exit 1
fi

# Test 15: Test allow-command interactive functionality for Gradle
echo "\nTest 15: Testing allow-command interactive functionality for Gradle"
echo "Initial allowlist contents:"
cat /home/testuser/.dela/allowlist.toml

# Test interactive allow-command with option 2 (Allow this task)
echo "\nTesting interactive allow-command with 'Allow this task' option:"
echo "2" | dela allow-command gradleBuild >/dev/null 2>&1

echo "\nAllowlist contents after allow-command:"
cat /home/testuser/.dela/allowlist.toml

# Verify the allowlist was updated with the specific task
if grep -q "gradleBuild" /home/testuser/.dela/allowlist.toml; then
    echo "${GREEN}✓ gradleBuild task was added to allowlist via interactive mode${NC}"
else
    echo "${RED}✗ gradleBuild task was not added to allowlist via interactive mode${NC}"
    exit 1
fi

# Test 16: Basic dela list for GitHub Actions workflow jobs
echo "\nTest 16: Testing dela list for GitHub Actions workflow jobs"
cd /home/testuser/test_project
if dela list | grep -q "test"; then
    echo "${GREEN}✓ dela list shows GitHub Actions workflows${NC}"
else
    echo "${RED}✗ dela list failed to show GitHub Actions workflows${NC}"
    exit 1
fi

# Test 17: Test GitHub Actions runner detection
echo "\nTest 17: Testing GitHub Actions runner detection"

# Verify that act is available
if which act > /dev/null 2>&1; then
    echo "${GREEN}✓ act command is available${NC}"
else
    echo "${RED}✗ act command is not available${NC}"
    exit 1
fi

# Verify that GitHub Actions workflow file is detected - use regex to match any workflow with act runner
if dela list | grep -q "test.*act"; then
    echo "${GREEN}✓ GitHub Actions workflow file is detected${NC}"
else
    echo "${RED}✗ GitHub Actions workflow file is not detected${NC}"
    dela list
    exit 1
fi

# Test 18: Verify GitHub Actions workflow descriptions
echo "\nTest 18: Testing GitHub Actions workflow descriptions"
# Look for test workflows with act runner and Test Workflow description
if dela list | grep -q "test.*act.*Test Workflow"; then
    echo "${GREEN}✓ GitHub Actions workflow descriptions are correct${NC}"
else
    echo "${RED}✗ GitHub Actions workflow descriptions are incorrect${NC}"
    dela list
    exit 1
fi

# Test 19: Verify GitHub Actions task discovery
echo "\nTest 19: Verifying GitHub Actions task discovery"

# Output the actual tasks for debugging
dela list | grep "act" || true

# Verify that the GitHub Actions tasks were discovered correctly - look for 'act' tasks
# First check for any act runners, then check for test workflow specifically
if dela list | grep -q "act" && \
   dela list | grep -q "test.*act"; then
    echo "${GREEN}✓ GitHub Actions workflows were discovered correctly${NC}"
else
    echo "${RED}✗ GitHub Actions workflows were not discovered correctly${NC}"
    dela list
    exit 1
fi

# Test 20: Test single argument passing with print-arg-task
echo "\nTest 20: Testing single argument passing with print-arg-task"

# Test with print-arg-task and a single argument
output=$(dela get-command print-arg-task ARG=value1 2>&1)
if echo "$output" | grep -q "make.*print-arg-task.*ARG=value1"; then
    echo "${GREEN}✓ Single argument is passed through get-command${NC}"
else
    echo "${RED}✗ Single argument is not passed through get-command${NC}"
    echo "Expected: make print-arg-task ARG=value1"
    echo "Got: $output"
    exit 1
fi

# Test 21: Verify arguments are properly passed through get-command
echo "\nTest 21: Testing argument passing through get-command"

# Test with a makefile task and simple arguments (no quotes/spaces in values)
output=$(dela get-command print-args ARGS="--flag1 --flag2=value positional" 2>&1)
if echo "$output" | grep -q "make.*print-args.*ARGS=.*--flag1.*--flag2=value.*positional"; then
    echo "${GREEN}✓ Simple arguments are passed through get-command (make task)${NC}"
else
    echo "${RED}✗ Simple arguments are not passed through get-command (make task)${NC}"
    echo "Expected: make print-args ARGS=\"--flag1 --flag2=value positional\""
    echo "Got: $output"
    exit 1
fi

# Test with npm task (simple arguments only)
output=$(dela get-command npm-test --verbose --no-color 2>&1)
if echo "$output" | grep -q "npm run npm-test.*--verbose.*--no-color"; then
    echo "${GREEN}✓ Arguments are passed through get-command for npm task${NC}"
else
    echo "${RED}✗ Arguments are not passed through get-command for npm task${NC}"
    echo "Expected: npm run npm-test --verbose --no-color"
    echo "Got: $output"
    exit 1
fi

echo "${GREEN}✓ All get-command argument passing tests passed successfully${NC}"

# Test 22: Simulate dr command to verify argument passing
echo "\nTest 22: Testing argument passing with dr function simulation"

# Create a temporary dr function similar to what's in shell integrations
function temp_dr() {
    local cmd=$(dela get-command "$@")
    echo "COMMAND WOULD EXECUTE: $cmd"
}

# Test with simple arguments (avoid quotes/spaces in arguments until fixed in Docker)
result=$(temp_dr print-args --arg1 --arg2=value positional)
if echo "$result" | grep -q "print-args.*--arg1.*--arg2=value.*positional"; then
    echo "${GREEN}✓ Arguments are passed through dr function${NC}"
else
    echo "${RED}✗ Arguments are not passed through dr function${NC}"
    echo "Expected: COMMAND WOULD EXECUTE: make print-args --arg1 --arg2=value positional"
    echo "Got: $result"
    exit 1
fi

# Clean up
unset -f temp_dr

cd /home/testuser/test_project

# Test 23: Verify ambiguous task detection in dela list
echo "\nTest 23: Testing ambiguous task detection and disambiguation in dela list"

# Create a Makefile with the 'test' task
cat > duplicate_test.mk << EOF
test: ## Test task in Makefile
	echo "Another test implementation"
EOF

# Create a package.json with a 'test' task
cat > duplicate_test.json << EOF
{
  "name": "duplicate-test",
  "version": "1.0.0",
  "scripts": {
    "test": "echo \"Duplicate test task\""
  }
}
EOF

# First, run 'dela list' to see what disambiguated names are being used
dela list > list_output.txt
DISAMBIGUATION_SECTION=$(grep -A 20 "Duplicate task names (‖)" list_output.txt | grep "Use")

# Extract the exact suffixes for each runner type
MAKE_SUFFIX=$(echo "$DISAMBIGUATION_SECTION" | grep -o "'test-[^']*' for make version" | grep -o "test-[^']*" || echo "")
MAVEN_SUFFIX=$(echo "$DISAMBIGUATION_SECTION" | grep -o "'test-[^']*' for mvn version" | grep -o "test-[^']*" || echo "")
GRADLE_SUFFIX=$(echo "$DISAMBIGUATION_SECTION" | grep -o "'test-[^']*' for gradle version" | grep -o "test-[^']*" || echo "" | head -1)
NPM_SUFFIX=$(echo "$DISAMBIGUATION_SECTION" | grep -o "'test-[^']*' for npm version" | grep -o "test-[^']*" || echo "")
ACT_SUFFIX=$(echo "$DISAMBIGUATION_SECTION" | grep -o "'test-[^']*' for act version" | grep -o "test-[^']*" || echo "")

echo "Detected disambiguated task names:"
echo "- Make: $MAKE_SUFFIX"
echo "- Maven: $MAVEN_SUFFIX"
echo "- Gradle: $GRADLE_SUFFIX"
echo "- npm: $NPM_SUFFIX"
echo "- act: $ACT_SUFFIX"

# Verify that 'dela list' shows the disambiguation info
if grep -q "Duplicate task names (‖)" list_output.txt && grep -q "has multiple implementations" list_output.txt; then
    echo "${GREEN}✓ dela list shows the disambiguation information for conflicting tasks${NC}"
else
    echo "${RED}✗ dela list failed to show disambiguation information${NC}"
    echo "List output:"
    cat list_output.txt
    exit 1
fi

# Verify that 'test' appears as ambiguous
if grep -q "test.*‖" list_output.txt; then
    echo "${GREEN}✓ Ambiguous 'test' task is marked with ‖ symbol${NC}"
else
    echo "${RED}✗ Ambiguous 'test' task is not marked correctly${NC}"
    echo "List output for 'test':"
    grep "test" list_output.txt || true
    exit 1
fi

# Verify that at least one disambiguated name is displayed
if grep -q "Use 'test-" list_output.txt; then
    echo "${GREEN}✓ Disambiguated task names are displayed correctly${NC}"
else
    echo "${RED}✗ Disambiguated task names are not displayed correctly${NC}"
    echo "Disambiguation section:"
    grep -A 10 "Duplicate task names" list_output.txt || true
    exit 1
fi

# Test 24: Verify get-command with disambiguated task names
echo "\nTest 24: Testing get-command with disambiguated task names"

# Test make variant (if found)
if [ ! -z "$MAKE_SUFFIX" ]; then
    echo "Testing make variant with suffix: $MAKE_SUFFIX"
    output=$(dela get-command "$MAKE_SUFFIX" --verbose 2>&1 || echo "COMMAND_FAILED")
    
    echo "Command output for make variant:"
    echo "$output"
    
    # Check if command failed
    if [[ "$output" == "COMMAND_FAILED" ]]; then
        echo "${RED}✗ get-command execution failed for '$MAKE_SUFFIX'${NC}"
        exit 1
    fi
    
    # We're only checking if the right runner is used for the disambiguated task name
    if echo "$output" | grep -q "make"; then
        echo "${GREEN}✓ get-command correctly uses make runner for '$MAKE_SUFFIX'${NC}"
    else
        echo "${RED}✗ get-command does not use make runner for '$MAKE_SUFFIX'${NC}"
        echo "Got: $output"
        exit 1
    fi
else
    echo "${YELLOW}⚠ Make suffix not detected, skipping test${NC}"
fi

# Test maven variant (if found)
if [ ! -z "$MAVEN_SUFFIX" ]; then
    echo "Testing maven variant with suffix: $MAVEN_SUFFIX"
    output=$(dela get-command "$MAVEN_SUFFIX" --verbose 2>&1 || echo "COMMAND_FAILED")
    
    echo "Command output for maven variant:"
    echo "$output"
    
    # Check if command failed
    if [[ "$output" == "COMMAND_FAILED" ]]; then
        echo "${RED}✗ get-command execution failed for '$MAVEN_SUFFIX'${NC}"
        exit 1
    fi
    
    # We're only checking if the right runner is used for the disambiguated task name
    if echo "$output" | grep -q "mvn"; then
        echo "${GREEN}✓ get-command correctly uses maven runner for '$MAVEN_SUFFIX'${NC}"
    else
        echo "${RED}✗ get-command does not use maven runner for '$MAVEN_SUFFIX'${NC}"
        echo "Got: $output"
        exit 1
    fi
else
    echo "${YELLOW}⚠ Maven suffix not detected, skipping test${NC}"
fi

# Test npm variant (if found)
if [ ! -z "$NPM_SUFFIX" ]; then
    echo "Testing npm variant with suffix: $NPM_SUFFIX"
    output=$(dela get-command "$NPM_SUFFIX" --ci --watch 2>&1 || echo "COMMAND_FAILED")
    
    echo "Command output for npm variant:"
    echo "$output"
    
    # Check if command failed
    if [[ "$output" == "COMMAND_FAILED" ]]; then
        echo "${RED}✗ get-command execution failed for '$NPM_SUFFIX'${NC}"
        exit 1
    fi
    
    # We're only checking if the right runner is used for the disambiguated task name
    if echo "$output" | grep -q "npm"; then
        echo "${GREEN}✓ get-command correctly uses npm runner for '$NPM_SUFFIX'${NC}"
    else
        echo "${RED}✗ get-command does not use npm runner for '$NPM_SUFFIX'${NC}"
        echo "Got: $output"
        exit 1
    fi
else
    echo "${YELLOW}⚠ npm suffix not detected, skipping test${NC}"
fi

# Test act variant (if found)
if [ ! -z "$ACT_SUFFIX" ]; then
    echo "Testing act variant with suffix: $ACT_SUFFIX"
    output=$(dela get-command "$ACT_SUFFIX" --job=build 2>&1 || echo "COMMAND_FAILED")
    
    echo "Command output for act variant:"
    echo "$output"
    
    # Check if command failed
    if [[ "$output" == "COMMAND_FAILED" ]]; then
        echo "${RED}✗ get-command execution failed for '$ACT_SUFFIX'${NC}"
        exit 1
    fi
    
    # We're only checking if the right runner is used for the disambiguated task name
    if echo "$output" | grep -q "act"; then
        echo "${GREEN}✓ get-command correctly uses act runner for '$ACT_SUFFIX'${NC}"
    else
        echo "${RED}✗ get-command does not use act runner for '$ACT_SUFFIX'${NC}"
        echo "Got: $output"
        exit 1
    fi
else
    echo "${YELLOW}⚠ Act suffix not detected, skipping test${NC}"
fi

# Define yellow color for warnings
YELLOW='\033[1;33m'

# Test 25: Test allow-command with disambiguated task names
echo "\nTest 25: Testing allow-command with disambiguated task names"

# Test allowing the make variant with arguments (if detected)
if [ ! -z "$MAKE_SUFFIX" ]; then
    echo "Testing allow-command with make variant: $MAKE_SUFFIX"
    dela allow-command "$MAKE_SUFFIX" --allow 2

    # Verify the allowlist was updated with the original task name "test"
    # and with the Makefile path (not checking for the disambiguated name)
    if grep -q "path.*Makefile" /home/testuser/.dela/allowlist.toml && \
       grep -q 'tasks = \["test"\]' /home/testuser/.dela/allowlist.toml; then
        echo "${GREEN}✓ Make test task was added to allowlist${NC}"
    else
        echo "${RED}✗ Make test task was not added to allowlist${NC}"
        echo "Allowlist contents:"
        cat /home/testuser/.dela/allowlist.toml
        exit 1
    fi
else
    echo "${YELLOW}⚠ Make suffix not detected, skipping allow-command test${NC}"
fi

# Test allowing npm variant (if detected)
if [ ! -z "$NPM_SUFFIX" ]; then
    echo "Testing allow-command with npm variant: $NPM_SUFFIX"
    dela allow-command "$NPM_SUFFIX" --allow 2
    
    # Verify the allowlist was updated with the original task name "test"
    # and with the package.json path
    if grep -q "path.*package.json" /home/testuser/.dela/allowlist.toml && \
       grep -q 'tasks = \["test"\]' /home/testuser/.dela/allowlist.toml; then
        echo "${GREEN}✓ NPM test task was added to allowlist${NC}"
    else
        echo "${RED}✗ NPM test task was not added to allowlist${NC}"
        echo "Allowlist contents:"
        cat /home/testuser/.dela/allowlist.toml
        exit 1
    fi
else
    echo "${YELLOW}⚠ npm suffix not detected, skipping allow-command test${NC}"
fi

# Clean up
rm -f duplicate_test.json duplicate_test.mk list_output.txt

echo "\n${GREEN}All non-init tests completed successfully!${NC}"
