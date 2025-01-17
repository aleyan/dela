# Basic PowerShell profile for testing

# Set a basic prompt
function prompt {
    "$($PWD.Path)> "
}

# Set up basic path
$env:PATH = "/usr/local/bin:/usr/bin:/bin:$env:PATH"

# Basic command not found handler (will be replaced by dela)
trap [System.Management.Automation.CommandNotFoundException] {
    Write-Error "pwsh: command not found: $($_.CategoryInfo.TargetName)"
    continue
} 