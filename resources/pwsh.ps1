# dela function wrapper to handle 'run' command specially
function dela {
    param([Parameter(ValueFromRemainingArguments=$true)]$args)
    if ($args.Count -gt 0 -and $args[0] -eq "run") {
        $cmd = command dela get-command $args[1..($args.Length-1)]
        Invoke-Expression $cmd
    } else {
        command dela $args
    }
}

# Command not found handler to delegate unknown commands to dela
trap [System.Management.Automation.CommandNotFoundException] {
    $cmdName = $_.CategoryInfo.TargetName
    try {
        $cmd = dela get-command $cmdName 2>$null
        if ($cmd) {
            Invoke-Expression $cmd
            continue
        }
    } catch {}
    Write-Error "pwsh: command not found: $cmdName"
    continue
} 