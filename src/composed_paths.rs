use crate::types::Task;
use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

/// Shared path metadata for tasks discovered through composed definitions such
/// as includes or inherited configs.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)] // DTKT-198/200/201 will consume the full recursive source API.
pub struct ComposedDefinitionSource {
    runner_path: PathBuf,
    definition_path: PathBuf,
}

#[allow(dead_code)] // DTKT-198/200/201 will consume the full recursive source API.
impl ComposedDefinitionSource {
    /// Create a source where the runner path and defining file are the same.
    pub fn direct(path: impl Into<PathBuf>) -> Self {
        let path = normalize_path(path.into());
        Self {
            runner_path: path.clone(),
            definition_path: path,
        }
    }

    /// Create a source where the runner path differs from the defining file.
    pub fn composed(
        runner_path: impl Into<PathBuf>,
        definition_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            runner_path: normalize_path(runner_path.into()),
            definition_path: normalize_path(definition_path.into()),
        }
    }

    pub fn runner_path(&self) -> &Path {
        &self.runner_path
    }

    pub fn definition_path(&self) -> &Path {
        &self.definition_path
    }

    /// Resolve a referenced child definition relative to the current defining file.
    pub fn resolve_child(&self, referenced_path: impl AsRef<Path>) -> PathBuf {
        resolve_nested_definition_path(&self.definition_path, referenced_path.as_ref())
    }

    /// Apply the source metadata to a discovered task.
    pub fn apply_to_task(&self, task: &mut Task) {
        task.file_path = self.runner_path.clone();
        if self.runner_path == self.definition_path {
            task.definition_path = None;
        } else {
            task.definition_path = Some(self.definition_path.clone());
        }
    }
}

/// Track visited definitions for recursive discovery so include cycles can be
/// handled consistently across runners.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)] // DTKT-198/200/201 will use recursive traversal state directly.
pub struct RecursiveDiscoveryState {
    visited: HashSet<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)] // DTKT-198/200/201 will use visit outcomes for include traversal.
pub enum VisitState {
    New(PathBuf),
    AlreadyVisited(PathBuf),
}

#[allow(dead_code)] // DTKT-198/200/201 will use recursive traversal state directly.
impl RecursiveDiscoveryState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mark_visited(&mut self, path: impl AsRef<Path>) -> VisitState {
        let normalized = normalize_path(path.as_ref());
        if self.visited.insert(normalized.clone()) {
            VisitState::New(normalized)
        } else {
            VisitState::AlreadyVisited(normalized)
        }
    }
}

/// Resolve a referenced definition path relative to the file that included it.
#[allow(dead_code)] // DTKT-198/200/201 will reuse this for include resolution.
pub fn resolve_nested_definition_path(
    current_definition_path: &Path,
    referenced_path: &Path,
) -> PathBuf {
    if referenced_path.is_absolute() {
        return normalize_path(referenced_path);
    }

    let base_dir = current_definition_path.parent().unwrap_or(Path::new("."));
    normalize_path(base_dir.join(referenced_path))
}

fn normalize_path(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    let mut normalized_parts: Vec<OsString> = Vec::new();
    let is_absolute = path.is_absolute();

    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::CurDir => {}
            Component::ParentDir => {
                if let Some(last) = normalized_parts.last() {
                    if last != ".." {
                        normalized_parts.pop();
                    } else if !is_absolute {
                        normalized_parts.push(OsString::from(".."));
                    }
                } else if !is_absolute {
                    normalized_parts.push(OsString::from(".."));
                }
            }
            Component::Normal(part) => normalized_parts.push(part.to_os_string()),
            Component::Prefix(prefix) => normalized_parts.push(prefix.as_os_str().to_os_string()),
        }
    }

    let mut normalized = if is_absolute {
        PathBuf::from(std::path::MAIN_SEPARATOR.to_string())
    } else {
        PathBuf::new()
    };

    for part in normalized_parts {
        normalized.push(part);
    }

    if normalized.as_os_str().is_empty() && !is_absolute {
        PathBuf::from(".")
    } else {
        normalized
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Task, TaskDefinitionType, TaskRunner};
    use std::path::PathBuf;

    fn sample_task() -> Task {
        Task {
            name: "build".to_string(),
            file_path: PathBuf::from("/tmp/runner"),
            definition_path: None,
            definition_type: TaskDefinitionType::Makefile,
            runner: TaskRunner::Make,
            source_name: "build".to_string(),
            description: None,
            shadowed_by: None,
            disambiguated_name: None,
        }
    }

    #[test]
    fn test_direct_source_uses_same_path_for_runner_and_definition() {
        let source = ComposedDefinitionSource::direct("/repo/Makefile");

        assert_eq!(source.runner_path(), Path::new("/repo/Makefile"));
        assert_eq!(source.definition_path(), Path::new("/repo/Makefile"));
    }

    #[test]
    fn test_composed_source_applies_runner_and_definition_paths_to_task() {
        let source = ComposedDefinitionSource::composed(
            "/repo/.github/workflows",
            "/repo/.github/workflows/ci.yml",
        );
        let mut task = sample_task();

        source.apply_to_task(&mut task);

        assert_eq!(task.file_path, PathBuf::from("/repo/.github/workflows"));
        assert_eq!(
            task.definition_path(),
            Path::new("/repo/.github/workflows/ci.yml")
        );
        assert_eq!(
            task.allowlist_path(),
            Path::new("/repo/.github/workflows/ci.yml")
        );
    }

    #[test]
    fn test_resolve_nested_definition_path_normalizes_relative_segments() {
        let resolved = resolve_nested_definition_path(
            Path::new("/repo/make/Makefile"),
            Path::new("../shared/tasks.mk"),
        );

        assert_eq!(resolved, PathBuf::from("/repo/shared/tasks.mk"));
    }

    #[test]
    fn test_recursive_discovery_state_detects_duplicate_paths_after_normalization() {
        let mut state = RecursiveDiscoveryState::new();

        assert_eq!(
            state.mark_visited("/repo/make/../shared/tasks.mk"),
            VisitState::New(PathBuf::from("/repo/shared/tasks.mk"))
        );
        assert_eq!(
            state.mark_visited("/repo/shared/tasks.mk"),
            VisitState::AlreadyVisited(PathBuf::from("/repo/shared/tasks.mk"))
        );
    }

    #[test]
    fn test_composed_source_resolves_children_relative_to_definition_file() {
        let source = ComposedDefinitionSource::composed(
            "/repo/Makefile",
            "/repo/includes/common/tasks.mk",
        );

        assert_eq!(
            source.resolve_child("../../other.mk"),
            PathBuf::from("/repo/other.mk")
        );
    }
}
