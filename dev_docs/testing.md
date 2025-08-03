# Testing
When writing tests that mock files, always set them to `#[serial]`.

## Case-sensitivity
When writing tests that create files with different case variations (e.g., "Justfile" vs "justfile"), be aware that filesystem case sensitivity varies between operating systems:
- macOS: Case-insensitive filesystem (default)
- Linux: Case-sensitive filesystem
- Windows: Case-insensitive filesystem (default)

Tests should account for both case-sensitive and case-insensitive filesystems by checking for the actual path that was found rather than assuming a specific path. Use assertions like:
```rust
assert!(path == expected_case_sensitive || path == expected_case_insensitive);
```
