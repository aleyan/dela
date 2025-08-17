## Unit Testing

### Running tests
   - Run with: `cargo test --lib`

### Mocking
When logic depends on the presence of a task runner, always mock it, otherwise
you will write a environment dependent test.
When writing tests that mock files, always set them to `#[serial]`.


### Case-sensitivity
When writing tests that create files with different case variations (e.g., "Justfile" vs "justfile"), be aware that filesystem case sensitivity varies between operating systems:
- macOS: Case-insensitive filesystem (default)
- Linux: Case-sensitive filesystem
- Windows: Case-insensitive filesystem (default)

Tests should account for both case-sensitive and case-insensitive filesystems by checking for the actual path that was found rather than assuming a specific path. Use assertions like:
```rust
assert!(path == expected_case_sensitive || path == expected_case_insensitive);
```

## Integration-tests

### Running Tests

The project uses a combination of unit tests and integration tests. Unit tests can be run with `cargo test --lib`, while integration tests require Docker and can be run with `make tests`. The test suite includes:

   - Run with: `make tests`
   - Uses Docker containers to test different shell environments:
     - Bash (`tests/docker_bash/`)
     - Fish (`tests/docker_fish/`)
     - Zsh (`tests/docker_zsh/`)
     - PowerShell (`tests/docker_pwsh/`)
     - No-init shell (`tests/docker_noinit/`)

### Shell Types and Docker Integration

Each shell type (bash, fish, zsh, pwsh) has its own Dockerfile in the `tests/docker_*/` directories. These Dockerfiles:

1. **Base Environment**: All use `alpine:3.21` as the base image
2. **Package Installation**: Install shell-specific packages and common tools:
   - Shell-specific package (bash/fish/zsh/powershell)
   - Build tools (make)
   - Python environment (python3, uv, poetry)
   - Task runner from community repo
   
3. **User Setup**: Create a test user with appropriate shell configuration:
   - Shell-specific config files (`.bashrc`, `config.fish`, `.zshrc`, etc.)
   - Proper permissions and environment variables
   - Dela configuration directory (`~/.dela`)

4. **Test Files**: Mount test definitions and scripts for verification:
   - Task definitions from various build systems
   - Shell-specific test scripts
   - Proper file permissions and ownership

The `docker_noinit` container provides a clean environment for testing dela without any shell initialization, ensuring dela works correctly even without shell-specific configurations.
