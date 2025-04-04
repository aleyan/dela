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
if dela list | grep -q "build" && dela list | grep -q "test" && dela list | grep -q "deploy"; then
    echo "${GREEN}✓ dela list shows GitHub Actions jobs${NC}"
else
    echo "${RED}✗ dela list failed to show GitHub Actions jobs${NC}"
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

# Verify that GitHub Actions workflow file is detected
if dela list | grep -q "Test Workflow"; then
    echo "${GREEN}✓ GitHub Actions workflow file is detected${NC}"
else
    echo "${RED}✗ GitHub Actions workflow file is not detected${NC}"
    exit 1
fi

# Test 18: Verify GitHub Actions job descriptions
echo "\nTest 18: Testing GitHub Actions job descriptions"
if dela list | grep -q "Test Workflow - Build Project" && \
   dela list | grep -q "Test Workflow - Run Tests" && \
   dela list | grep -q "Test Workflow - Deploy to Production"; then
    echo "${GREEN}✓ GitHub Actions job descriptions are correct${NC}"
else
    echo "${RED}✗ GitHub Actions job descriptions are incorrect${NC}"
    dela list
    exit 1
fi

# Test 19: Verify GitHub Actions task discovery
echo "\nTest 19: Verifying GitHub Actions task discovery"

# Output the actual tasks for debugging
dela list | grep "act" || true

# Verify that the GitHub Actions tasks were discovered correctly - look for 'act' tasks
if dela list | grep -q "act" && \
   dela list | grep -q "build (act)" && \
   dela list | grep -q "deploy (act)"; then
    echo "${GREEN}✓ GitHub Actions jobs were discovered correctly${NC}"
else
    echo "${RED}✗ GitHub Actions jobs were not discovered correctly${NC}"
    exit 1
fi

cd /home/testuser/test_project

echo "\n${GREEN}All non-init tests completed successfully!${NC}"
