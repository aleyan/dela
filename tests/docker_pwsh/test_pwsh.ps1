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
    if (-not ($output -match "The term 'nonexistent_command' is not recognized")) {
        Write-Error "Initial command_not_found_handler not working.`nExpected PowerShell error message for unrecognized command`nGot: '$output'"
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
if (-not ($profileContent -match [regex]::Escape('Invoke-Expression (dela configure-shell | Out-String)'))) {
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
    Invoke-Expression $output
} catch {
    Write-Error "dela configure-shell failed with output: $_"
}

# Test dela list command
Write-Log "Testing dela list command..."
$listOutput = dela list
Write-Host "Debug - dela list output:"
Write-Host $listOutput
Write-Host "Debug - End of dela list output"
if (-not ($listOutput -match "test-task")) {
    Write-Error "test-task not found in dela list"
}
if (-not ($listOutput -match "npm-test")) {
    Write-Error "npm-test not found in dela list"
}
if (-not ($listOutput -match "npm-build")) {
    Write-Error "npm-build not found in dela list"
}

if (!(dela list | Select-String -Quiet "poetry-build")) {
    Write-Error "poetry-build not found in dela list"
    exit 1
}

Write-Log "Testing task shadowing detection..."

# Create a custom executable in PATH
Write-Log "Creating custom executable..."
$localBinPath = Join-Path $HOME ".local" "bin"
if (-not (Test-Path $localBinPath)) {
    New-Item -ItemType Directory -Path $localBinPath -Force | Out-Null
}

# Create a custom executable
$customExePath = Join-Path $localBinPath "custom-exe"
Set-Content -Path $customExePath -Value @"
#!/bin/sh
echo "I am a custom executable"
"@

# Make the file executable using chmod (since we're in a Linux container)
& chmod +x $customExePath

# Add ~/.local/bin to PATH if not already present
$localBinPath = (Resolve-Path $localBinPath).Path
if (-not ($env:PATH -split ':' -contains $localBinPath)) {
    $env:PATH = "${localBinPath}:$env:PATH"
}

# Verify the executable exists and is executable
if (-not (Test-Path $customExePath)) {
    Write-Error "Failed to create custom executable at $customExePath"
}

Write-Log "Testing if custom-exe is in PATH..."
Write-Log "Current PATH: $env:PATH"
Write-Log "Executable path: $customExePath"
$customExeExists = Get-Command custom-exe -ErrorAction SilentlyContinue
if (-not $customExeExists) {
    Write-Error "custom-exe not found in PATH"
}

# Test that dela list shows shadowing symbols
Write-Log "Testing shadow detection in dela list..."
$output = dela list | Out-String

Write-Log "Debug - dela list output:"
Write-Host $output
Write-Log "Debug - End of dela list output"

# Check for shell builtin shadowing (cd)
if (!(Select-String -InputObject $output -Pattern "cd †" -Quiet)) {
    Write-Error "Shell builtin shadowing symbol not found for 'cd' task"
    Write-Error "Got output: $output"
    exit 1
}

if (!(Select-String -InputObject $output -Pattern "† task 'cd' shadowed by pwsh shell builtin" -Quiet)) {
    Write-Error "Shell builtin shadow info not found for 'cd' task"
    Write-Error "Got output: $output"
    exit 1
}

# Check for PATH executable shadowing (custom-exe)
if (!(Select-String -InputObject $output -Pattern "custom-exe ‡" -Quiet)) {
    Write-Error "PATH executable shadowing symbol not found for 'custom-exe' task"
    Write-Error "Got output: $output"
    exit 1
}

if (!(Select-String -InputObject $output -Pattern "‡ task 'custom-exe' shadowed by executable at" -Quiet)) {
    Write-Error "PATH executable shadow info not found for 'custom-exe' task"
    Write-Error "Got output: $output"
    exit 1
}

Write-Log "4. Testing allowlist functionality..."

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
    if (-not ($output -match "The term 'nonexistent_command' is not recognized")) {
        Write-Error "Command not found handler wasn't properly replaced.`nGot: '$output'"
    }
}

Write-Log "=== All tests passed successfully! ===" 