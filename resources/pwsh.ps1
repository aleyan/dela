# dela function wrapper to handle 'run' command specially
function dela {
    param([Parameter(ValueFromRemainingArguments=$true)]$args)
    Write-Host "Debug: dela function called with args: $args"
    Write-Host "Debug: args count: $($args.Count)"
    if ($args.Count -gt 0) {
        Write-Host "Debug: first arg: $($args[0])"
    }
    
    if ($args.Count -gt 0 -and $args[0] -eq "run") {
        Write-Host "Debug: handling 'run' command"
        $cmd = & command dela get-command ($args | Select-Object -Skip 1)
        Write-Host "Debug: got command: $cmd"
        Invoke-Expression $cmd
    } else {
        Write-Host "Debug: passing through to dela command"
        Write-Host "Debug: passing args directly: $args"
        & command dela $args
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
    } catch {}
    Write-Error "pwsh: command not found: $cmdName"
    continue
} 