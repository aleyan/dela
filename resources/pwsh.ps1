# dr function to handle task execution
function dr {
    [CmdletBinding()]
    param([Parameter(ValueFromRemainingArguments=$true)][string[]]$Arguments)
    
    $delaBinary = (Get-Command dela -CommandType Application).Source
    $cmd = & $delaBinary get-command ($Arguments -join ' ')
    if ($LASTEXITCODE -eq 0) {
        $env:DELA_TASK_RUNNING = 1
        try {
            if ($cmd -is [array]) {
                $cmd = $cmd -join "`n"
            }
            Invoke-Expression $cmd
        } finally {
            Remove-Item Env:\DELA_TASK_RUNNING -ErrorAction SilentlyContinue
        }
    }
}

# Command not found handler to delegate unknown commands to dela
trap [System.Management.Automation.CommandNotFoundException] {
    $cmdName = $_.CategoryInfo.TargetName
    
    # Skip if we're already running a task
    if ($env:DELA_TASK_RUNNING) {
        Write-Error "pwsh: command not found: $cmdName"
        continue
    }
    
    try {
        # First check if the task exists
        $cmd = & dela get-command $cmdName 2>$null
        if ($LASTEXITCODE -ne 0) {
            Write-Error "pwsh: command not found: $cmdName"
            continue
        }
        
        # Then check if it's allowed
        if (-not (& dela allow-command $cmdName)) {
            continue
        }
        
        # If allowed, get and execute the command
        $env:DELA_TASK_RUNNING = 1
        try {
            if ($cmd -is [array]) {
                $cmd = $cmd -join "`n"
            }
            Invoke-Expression $cmd
        } finally {
            Remove-Item Env:\DELA_TASK_RUNNING -ErrorAction SilentlyContinue
        }
        continue
    } catch {
        Write-Error "pwsh: command not found: $cmdName"
    }
    continue
} 