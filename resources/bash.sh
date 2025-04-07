# dr function to handle task execution
dr() {
    local cmd=$(command dela get-command -- "$@")
    eval "$cmd"
}

# Command not found handler for bash
command_not_found_handle() {
    # Skip if we're already running a task to avoid infinite recursion
    if [ -n "${DELA_TASK_RUNNING}" ]; then
        echo "bash: command not found: $1" >&2
        return 127
    fi

    # First check if the task is allowed
    if ! dela allow-command "$1"; then
        return 127
    fi
    
    # If allowed, get and execute the command
    if cmd=$(dela get-command -- "$@"); then
        export DELA_TASK_RUNNING=1
        eval "$cmd"
        local status=$?
        unset DELA_TASK_RUNNING
        return $status
    fi
    echo "bash: command not found: $1" >&2
    return 127
} 