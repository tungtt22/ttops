use std::fs;

use tempfile::TempDir;
use ttops_core::fsutil::{last_modified_ms, path_size};

#[test]
fn measures_regular_files_without_following_symlinks() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("root");
    fs::create_dir_all(root.join("nested")).unwrap();
    fs::write(root.join("a"), b"1234").unwrap();
    fs::write(root.join("nested/b"), b"123456").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(temp.path(), root.join("nested/link-out")).unwrap();

    assert_eq!(path_size(&root).unwrap(), 10);
}

#[test]
fn reports_last_modified_and_missing_paths() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("file");
    fs::write(&file, b"x").unwrap();
    assert!(last_modified_ms(&file).unwrap().is_some());
    assert!(path_size(&temp.path().join("missing")).is_err());
}
