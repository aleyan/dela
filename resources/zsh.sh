# dela function wrapper to handle 'run' command specially
dela() {
    if [[ $1 == "run" ]]; then
        cmd=$(command dela get-command "${@:2}")
        eval "$cmd"
    else
        command dela "$@"
    fi
}

# Command not found handler to delegate unknown commands to dela
command_not_found_handler() {
    # First check if the task is allowed
    if ! dela allow-command "$1" 2>/dev/null; then
        return 127
    fi
    
    # If allowed, get and execute the command
    if cmd=$(dela get-command "$1"); then
        eval "$cmd"
        return $?
    fi
    echo "zsh: command not found: $1" >&2
    return 127
}
