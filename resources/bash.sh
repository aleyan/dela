# dela function wrapper to handle 'run' command specially
dela() {
    if [[ $1 == "run" ]]; then
        cmd=$(command dela get-command "${@:2}")
        DELA_TASK_RUNNING=1 eval "$cmd"
    else
        command dela "$@"
    fi
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
    if ! dela allow-command "$1" 2>/dev/null; then
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