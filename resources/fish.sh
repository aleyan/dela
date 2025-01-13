# dela function wrapper to handle 'run' command specially
function dela
    if test "$argv[1]" = "run"
        set -l cmd (command dela get-command $argv[2..-1])
        eval $cmd
    else
        command dela $argv
    end
end

# Command not found handler to delegate unknown commands to dela
function fish_command_not_found
    set -l cmd (dela get-command $argv[1] 2>/dev/null)
    if test $status -eq 0
        eval $cmd $argv[2..-1]
        return $status
    end
    echo "fish: Unknown command: $argv[1]" >&2
    return 127
end 