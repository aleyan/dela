# dr function to handle task execution
dr() {
    cmd=$(command dela get-command "$@")
    DELA_TASK_RUNNING=1 eval "$cmd"
}

# Command not found handler to delegate unknown commands to dela
command_not_found_handle() {
    # Skip if we're inside a task execution (DELA_TASK_RUNNING=1)
    if [[ -n "${DELA_TASK_RUNNING}" ]]; then
        echo "bash: command not found: $1" >&2
        return 127
    fi

    # First check if the task exists
    if ! dela get-command "$1" >/dev/null 2>&1; then
        echo "bash: command not found: $1" >&2
        return 127
    fi

    # Then check if it's allowed
    if ! dela allow-command "$1"; then
        return 127
    fi
    
    # If allowed, get and execute the command
    if cmd=$(dela get-command "$1"); then
        shift
        DELA_TASK_RUNNING=1 eval "$cmd $*"
        return $?
    fi
    echo "bash: command not found: $1" >&2
    return 127
} 