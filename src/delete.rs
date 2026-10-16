//! Removing targets from the filesystem.

use std::fs;
use std::io;
use std::path::PathBuf;

use crate::scan::{Target, TargetKind};

/// The outcome of [`delete`].
#[derive(Debug, Default)]
pub struct DeleteSummary {
    /// The number of targets that were removed.
    pub deleted: usize,
    /// The sum of the sizes of the removed targets.
    pub freed: u64,
    /// The targets that could not be removed.
    pub failures: Vec<Failure>,
}

/// A target that could not be removed.
#[derive(Debug)]
pub struct Failure {
    /// The path of the target.
    pub path: PathBuf,
    /// The error returned while removing it.
    pub error: io::Error,
}

/// Removes every target: directories with everything inside them, files and symlinks
/// with a single unlink, so symlinks are never followed. A failure is recorded and
/// the remaining targets are still removed.
pub fn delete<'a>(targets: impl IntoIterator<Item = &'a Target>) -> DeleteSummary {
    let mut summary = DeleteSummary::default();
    for target in targets {
        let result = match target.kind {
            TargetKind::Dir => fs::remove_dir_all(&target.path),
            TargetKind::File | TargetKind::Symlink => fs::remove_file(&target.path),
        };
        match result {
            Ok(()) => {
                summary.deleted += 1;
                summary.freed += target.size;
            }
            Err(error) => summary.failures.push(Failure {
                path: target.path.clone(),
                error,
            }),
        }
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::TempDir;

    /// Returns a target for `path` with the given kind and size.
    fn target(path: &Path, kind: TargetKind, size: u64) -> Target {
        Target {
            path: path.to_path_buf(),
            kind,
            size,
        }
    }

    // A directory target is removed with everything inside it.
    #[test]
    fn directory_is_removed_recursively() {
        let d = TempDir::new().unwrap();
        let build = d.path().join("build");
        fs::create_dir_all(build.join("sub")).unwrap();
        fs::write(build.join("sub/a.o"), "abc").unwrap();
        let summary = delete(&[target(&build, TargetKind::Dir, 3)]);
        assert!(!build.exists());
        assert_eq!(summary.deleted, 1);
        assert_eq!(summary.freed, 3);
        assert!(summary.failures.is_empty());
    }

    // A file target is removed.
    #[test]
    fn file_is_removed() {
        let d = TempDir::new().unwrap();
        let file = d.path().join("debug.log");
        fs::write(&file, "1234").unwrap();
        let summary = delete(&[target(&file, TargetKind::File, 4)]);
        assert!(!file.exists());
        assert_eq!(summary.freed, 4);
    }

    // A symlink target removes the link and leaves the linked directory intact.
    #[cfg(unix)]
    #[test]
    fn symlink_is_removed_without_its_target() {
        let outside = TempDir::new().unwrap();
        fs::write(outside.path().join("big.bin"), "data").unwrap();
        let d = TempDir::new().unwrap();
        let link = d.path().join("link");
        std::os::unix::fs::symlink(outside.path(), &link).unwrap();
        let summary = delete(&[target(&link, TargetKind::Symlink, 0)]);
        assert!(fs::symlink_metadata(&link).is_err());
        assert!(outside.path().join("big.bin").exists());
        assert_eq!(summary.deleted, 1);
    }

    // A missing target is reported as a failure, and the remaining targets are still removed.
    #[test]
    fn missing_target_is_a_failure_and_deletion_continues() {
        let d = TempDir::new().unwrap();
        let missing = d.path().join("missing");
        let file = d.path().join("out.o");
        fs::write(&file, "ab").unwrap();
        let summary = delete(&[
            target(&missing, TargetKind::Dir, 10),
            target(&file, TargetKind::File, 2),
        ]);
        assert!(!file.exists());
        assert_eq!(summary.deleted, 1);
        assert_eq!(summary.freed, 2);
        assert_eq!(summary.failures.len(), 1);
        assert_eq!(summary.failures[0].path, missing);
    }
}
