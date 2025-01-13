# Function wrapper for dela to handle 'run' command specially
function dela {
    param([Parameter(ValueFromRemainingArguments=$true)]$args)
    
    if ($args[0] -eq "run") {
        $cmd = (command dela get-command $args[1..($args.Length-1)])
        Invoke-Expression $cmd
    } else {
        command dela $args
    }
}

# Command not found handler using trap
trap [System.Management.Automation.CommandNotFoundException] {
    $cmdName = $_.CategoryInfo.TargetName
    try {
        $cmd = (dela get-command $cmdName 2>$null)
        if ($cmd) {
            $params = $_.CategoryInfo.TargetObject.CommandElements[1..($_.CategoryInfo.TargetObject.CommandElements.Count-1)]
            Invoke-Expression "$cmd $params"
            continue
        }
    } catch {}
    
    Write-Error "pwsh: command not found: $cmdName"
    continue
} 