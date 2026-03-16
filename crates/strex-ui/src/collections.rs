//! Collections discovery — scans a directory for `.yaml` files.

use std::path::Path;

/// Scan `dir` for `.yaml` files (non-recursive) and return their filenames as strings.
///
/// Returns an empty vec if the directory cannot be read. Results are sorted alphabetically.
pub(crate) fn scan_yaml_files(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return vec![];
    };
    let mut files: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("yaml"))
        .filter_map(|e| e.file_name().to_str().map(|s| s.to_string()))
        .collect();
    files.sort();
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_yaml_files_sorted() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::File::create(dir.path().join("b.yaml")).unwrap();
        std::fs::File::create(dir.path().join("a.yaml")).unwrap();
        std::fs::File::create(dir.path().join("skip.json")).unwrap();

        let files = scan_yaml_files(dir.path());
        assert_eq!(files, vec!["a.yaml", "b.yaml"]);
    }

    #[test]
    fn returns_empty_for_missing_dir() {
        let files = scan_yaml_files(Path::new("/nonexistent/path/xyz"));
        assert!(files.is_empty());
    }

    #[test]
    fn ignores_non_yaml_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::File::create(dir.path().join("data.csv")).unwrap();
        std::fs::File::create(dir.path().join("README.md")).unwrap();

        let files = scan_yaml_files(dir.path());
        assert!(files.is_empty());
    }
}
