# dr function to handle task execution
function dr {
    [CmdletBinding()]
    param([Parameter(ValueFromRemainingArguments=$true)][string[]]$Arguments)
    
    $delaBinary = (Get-Command dela -CommandType Application).Source
    $cmd = & $delaBinary get-command -- $Arguments
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
        # First check if the task is allowed (only needs the command name)
        if (-not (& dela allow-command $cmdName)) {
            continue
        }
        
        # If allowed, get and execute the command with all arguments
        $allArgs = @($cmdName) + $_.CategoryInfo.CommandLine.ToString().SubString($cmdName.Length).Trim() -split '\s+'
        $allArgs = $allArgs | Where-Object { $_ -ne "" }
        
        $env:DELA_TASK_RUNNING = 1
        try {
            $cmd = & dela get-command -- $allArgs
            if ($LASTEXITCODE -ne 0) {
                Write-Error "pwsh: command not found: $cmdName"
                continue
            }
            
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