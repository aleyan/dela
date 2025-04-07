# dr function to handle task execution
dr() {
    cmd=$(command dela get-command "$@")
    eval "$cmd"
}

# Command not found handler to delegate unknown commands to dela
command_not_found_handler() {
    # First check if the task is allowed
    if ! dela allow-command "$1"; then
        return 127
    fi
    
    # If allowed, get and execute the command
    if cmd=$(dela get-command "$@"); then
        eval "$cmd"
        return $?
    fi
    echo "zsh: command not found: $1" >&2
    return 127
}
