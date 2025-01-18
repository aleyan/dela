# dela function wrapper to handle 'run' command specially
function dela {
    [CmdletBinding()]
    param([Parameter(ValueFromRemainingArguments=$true)][string[]]$Arguments)
    
    $delaBinary = (Get-Command dela -CommandType Application).Source
    
    if ($Arguments.Count -gt 0 -and $Arguments[0] -eq "run") {
        $taskArgs = $Arguments[1..($Arguments.Count-1)]
        $cmd = & $delaBinary get-command ($taskArgs -join ' ')
        Invoke-Expression $cmd
    } else {
        if ($Arguments.Count -gt 0 -and $Arguments[0] -eq "configure-shell") {
            try {
                $result = & $delaBinary "configure-shell" 2>&1
                if ($LASTEXITCODE -ne 0) {
                    Write-Host "Error: dela configure-shell failed with exit code: $LASTEXITCODE"
                }
                # Join array output into a string if needed
                if ($result -is [array]) {
                    $result = $result -join "`n"
                }
                $result
            } catch {
                Write-Host "Error: dela configure-shell failed: $($_.Exception.Message)"
                throw
            }
        } else {
            & $delaBinary $Arguments
        }
    }
}

# Command not found handler to delegate unknown commands to dela
trap [System.Management.Automation.CommandNotFoundException] {
    $cmdName = $_.CategoryInfo.TargetName
    try {
        $cmd = & dela get-command $cmdName 2>$null
        if ($cmd) {
            Invoke-Expression $cmd
            continue
        }
    } catch {
        Write-Host "Error: Failed to handle command '$cmdName': $($_.Exception.Message)"
    }
    Write-Error "pwsh: command not found: $cmdName"
    continue
} 