# Exit on any error
$ErrorActionPreference = "Stop"

# Default to non-verbose output
if (-not $env:VERBOSE) {
    $env:VERBOSE = "0"
}

# Set up logging functions
function Write-Log {
    param([string]$Message)
    if ($env:VERBOSE -eq "1") {
        Write-Host $Message
    }
}

function Write-Error {
    param([string]$Message)
    [Console]::Error.WriteLine("Error: $Message")
    exit 1
}

Write-Log "=== Testing dela shell integration for PowerShell ==="

Write-Log "1. Verifying test environment..."

# Verify dela binary is installed and accessible
if (-not (Get-Command dela -ErrorAction SilentlyContinue)) {
    Write-Error "dela not found in PATH"
}

# Verify PowerShell profile exists
$profilePath = "$HOME/.config/powershell/Microsoft.PowerShell_profile.ps1"
if (-not (Test-Path $profilePath)) {
    Write-Error "PowerShell profile not found at $profilePath"
}

# Verify Makefile exists
if (-not (Test-Path ~/Makefile)) {
    Write-Error "Makefile not found"
}

# Verify initial command not found handler works
try {
    nonexistent_command
} catch {
    $output = $_.Exception.Message
    if (-not ($output -match "pwsh: command not found: nonexistent_command")) {
        Write-Error "Initial command_not_found_handler not working.`nExpected: 'pwsh: command not found: nonexistent_command'`nGot: '$output'"
    }
}

Write-Log "2. Testing dela initialization..."

# Initialize dela and verify directory creation
dela init
if (-not (Test-Path ~/.dela)) {
    Write-Error "~/.dela directory not created"
}

# Verify shell integration was added
$profileContent = Get-Content $profilePath -Raw
if (-not ($profileContent -match [regex]::Escape('& (dela configure-shell | Out-String)'))) {
    Write-Error "Shell integration not found in PowerShell profile"
}

Write-Log "3. Testing dela shell integration..."

# Source updated profile and check for errors
try {
    . $profilePath
} catch {
    Write-Error "Failed to source PowerShell profile: $_"
}

# Verify shell integration was loaded
try {
    $output = dela configure-shell
} catch {
    Write-Error "dela configure-shell failed with output: $_"
}

# Test dela list command
Write-Log "Testing dela list command..."
if (-not (dela list | Select-String "test-task")) {
    Write-Error "test-task not found in dela list"
}
if (-not (dela list | Select-String "npm-test")) {
    Write-Error "npm-test not found in dela list"
}
if (-not (dela list | Select-String "npm-build")) {
    Write-Error "npm-build not found in dela list"
}

Write-Log "4. Testing task execution..."

# Test dela run command with Makefile task only
Write-Log "Testing dela run command..."
$output = dela run test-task
if (-not ($output -match "Test task executed successfully")) {
    Write-Error "dela run test-task failed. Got: $output"
}

# Verify command not found handler was properly replaced
Write-Log "Testing final command_not_found_handler..."
try {
    nonexistent_command
    Write-Error "Command not found handler didn't throw an error as expected"
} catch {
    $output = $_.Exception.Message
    if ($output -match "pwsh: command not found: nonexistent_command") {
        Write-Error "Command not found handler wasn't properly replaced.`nGot: '$output'"
    }
}

Write-Log "=== All tests passed successfully! ===" 