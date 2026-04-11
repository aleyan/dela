use std::path::{Path, PathBuf};

pub fn find_ancestor<F>(start: &Path, predicate: F) -> Option<PathBuf>
where
    F: Fn(&Path) -> bool,
{
    let mut current = if start.is_dir() {
        start.to_path_buf()
    } else {
        start.parent()?.to_path_buf()
    };

    loop {
        if predicate(&current) {
            return Some(current);
        }

        if !current.pop() {
            return None;
        }
    }
}

pub fn find_git_repo_root(start: &Path) -> Option<PathBuf> {
    find_ancestor(start, |dir| dir.join(".git").exists())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_find_git_repo_root_from_nested_directory() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        std::fs::create_dir_all(repo_root.join(".git")).unwrap();
        let nested = repo_root.join("apps").join("web").join("src");
        std::fs::create_dir_all(&nested).unwrap();

        assert_eq!(find_git_repo_root(&nested), Some(repo_root.to_path_buf()));
    }

    #[test]
    fn test_find_git_repo_root_from_file_path() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        std::fs::create_dir_all(repo_root.join(".git")).unwrap();
        let file_path = repo_root.join("apps").join("web").join("package.json");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(&file_path, "{}").unwrap();

        assert_eq!(
            find_git_repo_root(&file_path),
            Some(repo_root.to_path_buf())
        );
    }

    #[test]
    fn test_find_git_repo_root_returns_none_outside_git_repo() {
        let temp_dir = TempDir::new().unwrap();
        let nested = temp_dir.path().join("apps").join("web");
        std::fs::create_dir_all(&nested).unwrap();

        assert_eq!(find_git_repo_root(&nested), None);
    }
}
