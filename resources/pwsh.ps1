# dela function wrapper to handle 'run' command specially
function dela {
    [CmdletBinding()]
    param([Parameter(ValueFromRemainingArguments=$true)][string[]]$Arguments)
    
    Write-Host "Debug: ===== Starting dela function ====="
    Write-Host "Debug: Arguments type: $($Arguments.GetType().FullName)"
    Write-Host "Debug: Arguments raw value: $Arguments"
    Write-Host "Debug: Arguments count: $($Arguments.Count)"
    if ($Arguments.Count -gt 0) {
        Write-Host "Debug: First arg type: $($Arguments[0].GetType().FullName)"
        Write-Host "Debug: First arg value: $($Arguments[0])"
    }
    
    $delaBinary = (Get-Command dela -CommandType Application).Source
    Write-Host "Debug: Using binary at: $delaBinary"
    Write-Host "Debug: Binary exists: $(Test-Path $delaBinary)"
    Write-Host "Debug: Binary type: $((Get-Item $delaBinary).GetType().FullName)"
    
    if ($Arguments.Count -gt 0 -and $Arguments[0] -eq "run") {
        Write-Host "Debug: Handling 'run' command"
        $taskArgs = $Arguments[1..($Arguments.Count-1)]
        Write-Host "Debug: Task args: $taskArgs"
        Write-Host "Debug: Executing get-command with args: $($taskArgs -join ' ')"
        $cmd = & $delaBinary get-command ($taskArgs -join ' ')
        Write-Host "Debug: Got command result: $cmd"
        Write-Host "Debug: Executing command via Invoke-Expression"
        Invoke-Expression $cmd
    } else {
        Write-Host "Debug: Passing through to dela command"
        if ($Arguments.Count -gt 0 -and $Arguments[0] -eq "configure-shell") {
            Write-Host "Debug: Handling configure-shell command"
            Write-Host "Debug: Executing binary directly with configure-shell"
            try {
                $result = & $delaBinary "configure-shell" 2>&1
                Write-Host "Debug: Command result type: $($result.GetType().FullName)"
                Write-Host "Debug: Command raw output: $result"
                if ($LASTEXITCODE -ne 0) {
                    Write-Host "Debug: Command failed with exit code: $LASTEXITCODE"
                }
                # Join array output into a string if needed
                if ($result -is [array]) {
                    $result = $result -join "`n"
                }
                $result
            } catch {
                Write-Host "Debug: Command threw exception: $($_.Exception.Message)"
                throw
            }
        } else {
            Write-Host "Debug: Handling regular command"
            Write-Host "Debug: Passing arguments: $Arguments"
            & $delaBinary $Arguments
        }
    }
    Write-Host "Debug: ===== Ending dela function ====="
}

# Command not found handler to delegate unknown commands to dela
trap [System.Management.Automation.CommandNotFoundException] {
    $cmdName = $_.CategoryInfo.TargetName
    Write-Host "Debug: Command not found handler triggered for: $cmdName"
    try {
        Write-Host "Debug: Attempting to get command from dela"
        $cmd = & dela get-command $cmdName 2>$null
        if ($cmd) {
            Write-Host "Debug: Got command: $cmd"
            Invoke-Expression $cmd
            continue
        }
    } catch {
        Write-Host "Debug: Error getting command: $($_.Exception.Message)"
    }
    Write-Error "pwsh: command not found: $cmdName"
    continue
} 