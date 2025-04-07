# dr function to handle task execution
function dr
    set -l cmd (command dela get-command -- $argv)
    if test $status -eq 0
        set -x DELA_TASK_RUNNING 1
        eval $cmd
        set -e DELA_TASK_RUNNING
        return $status
    end
end

# Command not found handler to delegate unknown commands to dela
function fish_command_not_found
    # Skip if we're already running a task
    if set -q DELA_TASK_RUNNING
        echo "fish: Unknown command: $argv[1]" >&2
        return 127
    end

    # Check if this is a dela task
    set -l cmd (dela get-command -- $argv 2>/dev/null)
    if test $status -eq 0
        # Check if task is allowed
        if not dela allow-command $argv[1]
            return 127
        end
        # Execute the task
        set -x DELA_TASK_RUNNING 1
        eval $cmd
        set -e DELA_TASK_RUNNING
        return $status
    end
    echo "fish: Unknown command: $argv[1]" >&2
    return 127
end 