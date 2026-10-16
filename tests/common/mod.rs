//! Helpers for building fake project trees and running the binary.

use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use assert_cmd::Command;
use filetime::FileTime;
use tempfile::TempDir;
use walkdir::WalkDir;

/// Returns a command that runs the `winnower` binary.
pub fn winnower() -> Command {
    assert_cmd::cargo::cargo_bin_cmd!("winnower")
}

/// Creates a temporary directory with `.winnow` containing `winnow` and the given paths.
pub fn project(paths: &[&str], winnow: &str) -> TempDir {
    let dir = TempDir::new().unwrap();
    write(dir.path(), ".winnow", winnow);
    create(dir.path(), paths);
    dir
}

/// Writes `contents` to `relative` under `root`, creating parent directories.
pub fn write(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

/// Creates each path under `root`: a directory if it ends with `/`, otherwise an empty file.
pub fn create(root: &Path, paths: &[&str]) {
    for path in paths {
        if path.ends_with('/') {
            fs::create_dir_all(root.join(path)).unwrap();
        } else {
            write(root, path, "");
        }
    }
}

/// Sets the modification time of every file under `root` to `days` days ago.
pub fn age(root: &Path, days: u32) {
    let mtime = SystemTime::now() - Duration::from_secs(24 * 60 * 60) * days;
    for entry in WalkDir::new(root) {
        let entry = entry.unwrap();
        if entry.file_type().is_file() {
            filetime::set_file_mtime(entry.path(), FileTime::from_system_time(mtime)).unwrap();
        }
    }
}
