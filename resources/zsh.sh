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
    if cmd=$(dela get-command "$1" "${@:2}"); then
        eval "$cmd"
        return $?
    fi
    echo "zsh: command not found: $1" >&2
    return 127
}
