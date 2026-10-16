//! Finding projects, the paths their `.winnow` files match, and how recently they changed.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{Context, anyhow};
use walkdir::{DirEntry, WalkDir};

use crate::winnow_file::Matcher;

/// Name of the file that marks a project root.
const WINNOW_FILE: &str = ".winnow";

/// Name of the git directory, which is never matched or entered.
const GIT_DIR: &str = ".git";

/// A directory that contains a `.winnow` file.
#[derive(Debug)]
pub struct Project {
    /// The directory that contains the `.winnow` file.
    pub root: PathBuf,
    /// The newest modification time of an unmatched file in the project other than
    /// its `.winnow` file, or the Unix epoch if there is no such file.
    pub last_activity: SystemTime,
    /// The matched paths, in walk order.
    pub targets: Vec<Target>,
}

/// A path matched by a `.winnow` pattern, scheduled for deletion.
#[derive(Debug)]
pub struct Target {
    /// The matched path.
    pub path: PathBuf,
    /// What kind of filesystem entry the path is.
    pub kind: TargetKind,
    /// The apparent size in bytes: the length of a file or symlink, or the sum of the
    /// lengths of every file and symlink inside a directory.
    pub size: u64,
}

/// The kind of filesystem entry a target is. Symlinks are never followed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetKind {
    /// A directory, deleted with everything inside it.
    Dir,
    /// A regular file or another non-directory entry.
    File,
    /// A symbolic link, deleted without touching what it points to.
    Symlink,
}

/// Settings for [`scan`].
#[derive(Debug)]
pub struct ScanOptions {
    /// Projects with activity more recent than this are skipped. Zero selects every project.
    pub older_than: Duration,
    /// The current time, used to compute project ages.
    pub now: SystemTime,
}

/// The projects found by [`scan`].
#[derive(Debug)]
pub struct ScanResult {
    /// Projects with no activity for at least `older_than`.
    pub to_clean: Vec<Project>,
    /// Projects with activity within `older_than`.
    pub skipped_recent: Vec<Project>,
    /// Problems that caused a project or directory to be skipped.
    pub warnings: Vec<String>,
}

/// Finds every project under `root`, collects its targets and last activity,
/// and splits the projects by `options.older_than`.
///
/// A project with an invalid or nested `.winnow` file, or with an I/O error while it is
/// scanned, is left out of both lists and produces a warning. An unreadable directory
/// outside any project also produces a warning. Fails only if `root` cannot be read.
pub fn scan(root: &Path, options: &ScanOptions) -> anyhow::Result<ScanResult> {
    let (project_roots, mut warnings) = discover(root)?;
    let mut to_clean = Vec::new();
    let mut skipped_recent = Vec::new();
    for project_root in project_roots {
        match scan_project(&project_root) {
            Ok(project) if is_inactive(&project, options) => to_clean.push(project),
            Ok(project) => skipped_recent.push(project),
            Err(err) => warnings.push(format!(
                "skipping project {}: {err:#}",
                project_root.display()
            )),
        }
    }
    Ok(ScanResult {
        to_clean,
        skipped_recent,
        warnings,
    })
}

/// Returns whether `project` had no activity for at least `options.older_than`.
fn is_inactive(project: &Project, options: &ScanOptions) -> bool {
    let age = options
        .now
        .duration_since(project.last_activity)
        .unwrap_or(Duration::ZERO);
    age >= options.older_than
}

/// Returns a walker over `root` that does not follow symlinks or cross filesystems
/// and visits entries in name order.
fn walker(root: &Path) -> WalkDir {
    WalkDir::new(root)
        .follow_links(false)
        .same_file_system(true)
        .sort_by_file_name()
}

/// Returns the directories under `root` that contain a `.winnow` file, without looking
/// inside them, and a warning for each directory that could not be read.
fn discover(root: &Path) -> anyhow::Result<(Vec<PathBuf>, Vec<String>)> {
    let mut project_roots = Vec::new();
    let mut warnings = Vec::new();
    let mut entries = walker(root).into_iter();
    while let Some(entry) = entries.next() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) if err.depth() == 0 => {
                return Err(err).with_context(|| format!("reading {}", root.display()));
            }
            Err(err) => {
                warnings.push(format!("skipping directory: {err}"));
                continue;
            }
        };
        if !entry.file_type().is_dir() {
            continue;
        }
        if entry.file_name() == GIT_DIR {
            entries.skip_current_dir();
        } else if has_winnow_file(entry.path()) {
            project_roots.push(entry.into_path());
            entries.skip_current_dir();
        }
    }
    Ok((project_roots, warnings))
}

/// Returns whether `dir` directly contains a `.winnow` entry that is not a directory.
fn has_winnow_file(dir: &Path) -> bool {
    fs::symlink_metadata(dir.join(WINNOW_FILE)).is_ok_and(|metadata| !metadata.is_dir())
}

/// Returns whether `entry` is a `.winnow` entry that is not a directory.
fn is_winnow_file(entry: &DirEntry) -> bool {
    entry.file_name() == WINNOW_FILE && !entry.file_type().is_dir()
}

/// Returns the error for a `.winnow` file at `nested` inside the project at `project_root`.
fn nested_winnow_error(nested: &Path, project_root: &Path) -> anyhow::Error {
    anyhow!(
        "{} is inside the project of {}, and nested .winnow files are not supported",
        nested.display(),
        project_root.join(WINNOW_FILE).display()
    )
}

/// Loads the `.winnow` file of the project at `root` and walks the project,
/// collecting targets and the newest modification time of unmatched files
/// other than the project's `.winnow` file.
///
/// Fails if the `.winnow` file is invalid, another `.winnow` file exists below `root`,
/// or any entry cannot be read.
fn scan_project(root: &Path) -> anyhow::Result<Project> {
    let matcher = Matcher::load(&root.join(WINNOW_FILE))?;
    let mut last_activity = SystemTime::UNIX_EPOCH;
    let mut targets = Vec::new();
    let mut entries = walker(root).min_depth(1).into_iter();
    while let Some(entry) = entries.next() {
        let entry = entry?;
        let file_type = entry.file_type();
        if entry.file_name() == GIT_DIR {
            if file_type.is_dir() {
                entries.skip_current_dir();
            }
            continue;
        }
        if entry.depth() > 1 && is_winnow_file(&entry) {
            return Err(nested_winnow_error(entry.path(), root));
        }
        if matcher.is_match(entry.path(), file_type.is_dir()) {
            if file_type.is_dir() {
                entries.skip_current_dir();
            }
            targets.push(target(entry, root)?);
        } else if file_type.is_file() && !is_winnow_file(&entry) {
            let modified = entry
                .metadata()?
                .modified()
                .with_context(|| format!("reading mtime of {}", entry.path().display()))?;
            last_activity = last_activity.max(modified);
        }
    }
    Ok(Project {
        root: root.to_path_buf(),
        last_activity,
        targets,
    })
}

/// Builds the target for a matched `entry` of the project at `project_root`.
fn target(entry: DirEntry, project_root: &Path) -> anyhow::Result<Target> {
    let file_type = entry.file_type();
    let (kind, size) = if file_type.is_dir() {
        (TargetKind::Dir, dir_size(entry.path(), project_root)?)
    } else if file_type.is_symlink() {
        (TargetKind::Symlink, entry.metadata()?.len())
    } else {
        (TargetKind::File, entry.metadata()?.len())
    };
    Ok(Target {
        path: entry.into_path(),
        kind,
        size,
    })
}

/// Returns the sum of the lengths of every file and symlink inside `dir`.
///
/// Fails if a `.winnow` file is found inside `dir` or any entry cannot be read.
fn dir_size(dir: &Path, project_root: &Path) -> anyhow::Result<u64> {
    let mut size = 0;
    for entry in walker(dir).min_depth(1) {
        let entry = entry?;
        if is_winnow_file(&entry) {
            return Err(nested_winnow_error(entry.path(), project_root));
        }
        if !entry.file_type().is_dir() {
            size += entry.metadata()?.len();
        }
    }
    Ok(size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use filetime::FileTime;
    use tempfile::TempDir;

    const DAY: Duration = Duration::from_secs(24 * 60 * 60);

    /// Writes `contents` to `relative` under `root`, creating parent directories.
    fn write(root: &Path, relative: &str, contents: &str) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    /// Creates each path under `root`: a directory if it ends with `/`, otherwise an empty file.
    fn create(root: &Path, paths: &[&str]) {
        for path in paths {
            if path.ends_with('/') {
                fs::create_dir_all(root.join(path)).unwrap();
            } else {
                write(root, path, "");
            }
        }
    }

    /// Sets the modification time of `relative` under `root` to `days` days ago.
    fn age(root: &Path, relative: &str, days: u64) {
        let mtime = FileTime::from_system_time(SystemTime::now() - DAY * days as u32);
        filetime::set_file_mtime(root.join(relative), mtime).unwrap();
    }

    /// Scans `root` with the given filter and the current time.
    fn scan_with(root: &Path, older_than: Duration) -> ScanResult {
        let options = ScanOptions {
            older_than,
            now: SystemTime::now(),
        };
        scan(root, &options).unwrap()
    }

    /// Scans `root` without an age filter.
    fn scan_all(root: &Path) -> ScanResult {
        scan_with(root, Duration::ZERO)
    }

    /// Returns the roots of the projects in `projects`, relative to `root`.
    fn roots(projects: &[Project], root: &Path) -> Vec<String> {
        projects
            .iter()
            .map(|project| relative(&project.root, root))
            .collect()
    }

    /// Returns the target paths of `project`, relative to the project root.
    fn target_paths(project: &Project) -> Vec<String> {
        project
            .targets
            .iter()
            .map(|target| relative(&target.path, &project.root))
            .collect()
    }

    /// Returns `path` relative to `root` as a string.
    fn relative(path: &Path, root: &Path) -> String {
        path.strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .into_owned()
    }

    /// Removes all permissions from the directory at `path` and returns whether it became unreadable.
    #[cfg(unix)]
    fn lock(path: &Path) -> bool {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o000)).unwrap();
        fs::read_dir(path).is_err()
    }

    /// Restores owner permissions on the directory at `path`.
    #[cfg(unix)]
    fn unlock(path: &Path) {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }

    // A `.winnow` file at the scan root makes the scan root a project.
    #[test]
    fn project_at_scan_root_is_found() {
        let d = TempDir::new().unwrap();
        write(d.path(), ".winnow", "build/\n");
        let result = scan_all(d.path());
        assert_eq!(roots(&result.to_clean, d.path()), [""]);
        assert!(result.warnings.is_empty());
    }

    // Projects in nested subdirectories of the scan root are found.
    #[test]
    fn projects_in_subdirectories_are_found() {
        let d = TempDir::new().unwrap();
        write(d.path(), "a/.winnow", "build/\n");
        write(d.path(), "b/c/.winnow", "build/\n");
        let result = scan_all(d.path());
        assert_eq!(roots(&result.to_clean, d.path()), ["a", "b/c"]);
    }

    // Directories without a `.winnow` file produce no projects.
    #[test]
    fn directories_without_winnow_produce_no_projects() {
        let d = TempDir::new().unwrap();
        create(d.path(), &["src/main.rs", "build/"]);
        let result = scan_all(d.path());
        assert!(result.to_clean.is_empty());
        assert!(result.skipped_recent.is_empty());
        assert!(result.warnings.is_empty());
    }

    // A `.winnow` file in a subdirectory of a project skips the project and adds a warning naming both files.
    #[test]
    fn nested_winnow_skips_project() {
        let d = TempDir::new().unwrap();
        write(d.path(), "p/.winnow", "build/\n");
        write(d.path(), "p/sub/.winnow", "out/\n");
        let result = scan_all(d.path());
        assert!(result.to_clean.is_empty());
        assert_eq!(result.warnings.len(), 1);
        let warning = &result.warnings[0];
        assert!(warning.contains(&d.path().join("p/.winnow").display().to_string()));
        assert!(warning.contains(&d.path().join("p/sub/.winnow").display().to_string()));
    }

    // A `.winnow` file inside a matched directory skips the project and adds a warning.
    #[test]
    fn winnow_inside_matched_directory_skips_project() {
        let d = TempDir::new().unwrap();
        write(d.path(), ".winnow", "build/\n");
        create(d.path(), &["build/deep/.winnow"]);
        let result = scan_all(d.path());
        assert!(result.to_clean.is_empty());
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("build/deep/.winnow"));
    }

    // A matched directory is one target and its contents are not listed.
    #[test]
    fn matched_directory_is_one_target() {
        let d = TempDir::new().unwrap();
        write(d.path(), ".winnow", "build/\n");
        create(d.path(), &["build/a.o", "build/sub/b.o", "src/main.c"]);
        let result = scan_all(d.path());
        let project = &result.to_clean[0];
        assert_eq!(target_paths(project), ["build"]);
        assert_eq!(project.targets[0].kind, TargetKind::Dir);
    }

    // A matched regular file is a file target with its length as size.
    #[test]
    fn matched_file_is_file_target() {
        let d = TempDir::new().unwrap();
        write(d.path(), ".winnow", "*.log\n");
        write(d.path(), "logs/debug.log", "1234");
        let result = scan_all(d.path());
        let project = &result.to_clean[0];
        assert_eq!(target_paths(project), ["logs/debug.log"]);
        assert_eq!(project.targets[0].kind, TargetKind::File);
        assert_eq!(project.targets[0].size, 4);
    }

    // With `build/*` + `!build/keep`, every entry of `build/` except `keep` is a target and `build/` is not.
    #[test]
    fn negated_entry_inside_matched_contents_is_kept() {
        let d = TempDir::new().unwrap();
        write(d.path(), ".winnow", "build/*\n!build/keep\n");
        create(d.path(), &["build/a.o", "build/keep", "build/sub/b.o"]);
        let result = scan_all(d.path());
        assert_eq!(
            target_paths(&result.to_clean[0]),
            ["build/a.o", "build/sub"]
        );
    }

    // `.git` inside a project is never matched or scanned, even by a pattern that matches it.
    #[test]
    fn git_directory_is_not_matched_or_scanned() {
        let d = TempDir::new().unwrap();
        write(d.path(), ".winnow", ".*\n");
        create(d.path(), &[".git/HEAD", ".git/sub/.winnow", ".cache/x"]);
        let result = scan_all(d.path());
        assert!(result.warnings.is_empty());
        assert_eq!(target_paths(&result.to_clean[0]), [".cache", ".winnow"]);
    }

    // A `.winnow` file inside a `.git` directory outside any project does not create a project.
    #[test]
    fn discovery_skips_git_directory() {
        let d = TempDir::new().unwrap();
        write(d.path(), ".git/hooks/.winnow", "build/\n");
        let result = scan_all(d.path());
        assert!(result.to_clean.is_empty());
        assert!(result.warnings.is_empty());
    }

    // A symlink to a directory outside the project is not followed.
    #[cfg(unix)]
    #[test]
    fn symlink_to_outside_directory_is_not_followed() {
        let outside = TempDir::new().unwrap();
        create(outside.path(), &["build/a.o", "sub/.winnow"]);
        let d = TempDir::new().unwrap();
        write(d.path(), ".winnow", "build/\n");
        std::os::unix::fs::symlink(outside.path(), d.path().join("link")).unwrap();
        let result = scan_all(d.path());
        assert!(result.warnings.is_empty());
        assert!(result.to_clean[0].targets.is_empty());
    }

    // A matched symlink is a symlink target, even when it points to a directory.
    #[cfg(unix)]
    #[test]
    fn matched_symlink_is_symlink_target() {
        let outside = TempDir::new().unwrap();
        create(outside.path(), &["big.bin"]);
        let d = TempDir::new().unwrap();
        write(d.path(), ".winnow", "link\n");
        std::os::unix::fs::symlink(outside.path(), d.path().join("link")).unwrap();
        let result = scan_all(d.path());
        let project = &result.to_clean[0];
        assert_eq!(target_paths(project), ["link"]);
        assert_eq!(project.targets[0].kind, TargetKind::Symlink);
    }

    // A directory target's size is the sum of the file sizes inside it.
    #[test]
    fn directory_size_is_sum_of_file_sizes() {
        let d = TempDir::new().unwrap();
        write(d.path(), ".winnow", "build/\n");
        write(d.path(), "build/a", "hello");
        write(d.path(), "build/sub/b", "abc");
        create(d.path(), &["build/empty/"]);
        let result = scan_all(d.path());
        assert_eq!(result.to_clean[0].targets[0].size, 8);
    }

    // Activity uses only unmatched files, so a fresh build directory does not make an old project look active.
    #[test]
    fn activity_ignores_matched_files() {
        let d = TempDir::new().unwrap();
        write(d.path(), ".winnow", "build/\n");
        create(d.path(), &["src/main.c", "build/out.o"]);
        age(d.path(), ".winnow", 100);
        age(d.path(), "src/main.c", 90);
        let result = scan_with(d.path(), 30 * DAY);
        let project = &result.to_clean[0];
        let source_mtime = fs::metadata(d.path().join("src/main.c"))
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(project.last_activity, source_mtime);
    }

    // A fresh `.winnow` file does not make an old project look active.
    #[test]
    fn activity_ignores_winnow_file() {
        let d = TempDir::new().unwrap();
        write(d.path(), ".winnow", "build/\n");
        create(d.path(), &["src/main.c", "build/out.o"]);
        age(d.path(), "src/main.c", 90);
        let result = scan_with(d.path(), 30 * DAY);
        assert!(result.skipped_recent.is_empty());
        assert_eq!(target_paths(&result.to_clean[0]), ["build"]);
    }

    // A project changed recently is skipped by a 30-day filter.
    #[test]
    fn recent_project_is_skipped() {
        let d = TempDir::new().unwrap();
        write(d.path(), ".winnow", "build/\n");
        create(d.path(), &["src/main.c", "build/out.o"]);
        let result = scan_with(d.path(), 30 * DAY);
        assert!(result.to_clean.is_empty());
        assert_eq!(roots(&result.skipped_recent, d.path()), [""]);
    }

    // A project with no changes for longer than the filter is selected for cleaning.
    #[test]
    fn old_project_is_selected() {
        let d = TempDir::new().unwrap();
        write(d.path(), ".winnow", "build/\n");
        create(d.path(), &["src/main.c", "build/out.o"]);
        age(d.path(), ".winnow", 40);
        age(d.path(), "src/main.c", 40);
        let result = scan_with(d.path(), 30 * DAY);
        assert!(result.skipped_recent.is_empty());
        assert_eq!(target_paths(&result.to_clean[0]), ["build"]);
    }

    // `older_than = 0` selects a project changed just now.
    #[test]
    fn zero_older_than_selects_every_project() {
        let d = TempDir::new().unwrap();
        write(d.path(), ".winnow", "build/\n");
        create(d.path(), &["src/main.c"]);
        let result = scan_all(d.path());
        assert_eq!(result.to_clean.len(), 1);
        assert!(result.skipped_recent.is_empty());
    }

    // A project whose files are all matched has no activity and is selected by a 30-day filter.
    #[test]
    fn project_with_only_matched_files_is_inactive() {
        let d = TempDir::new().unwrap();
        write(d.path(), ".winnow", "*\n");
        create(d.path(), &["out.o"]);
        let result = scan_with(d.path(), 30 * DAY);
        let project = &result.to_clean[0];
        assert_eq!(project.last_activity, SystemTime::UNIX_EPOCH);
        assert_eq!(target_paths(project), [".winnow", "out.o"]);
    }

    // A `.winnow` file with an invalid pattern skips the project and adds a warning.
    #[test]
    fn invalid_pattern_skips_project() {
        let d = TempDir::new().unwrap();
        write(d.path(), ".winnow", "[z-a]\n");
        let result = scan_all(d.path());
        assert!(result.to_clean.is_empty());
        assert!(result.skipped_recent.is_empty());
        assert_eq!(result.warnings.len(), 1);
    }

    // An unreadable directory inside a project skips the project and adds a warning.
    #[cfg(unix)]
    #[test]
    fn io_error_inside_project_skips_project() {
        let d = TempDir::new().unwrap();
        write(d.path(), ".winnow", "build/\n");
        create(d.path(), &["locked/"]);
        let locked = d.path().join("locked");
        if !lock(&locked) {
            unlock(&locked);
            return;
        }
        let result = scan_all(d.path());
        unlock(&locked);
        assert!(result.to_clean.is_empty());
        assert_eq!(result.warnings.len(), 1);
    }

    // An unreadable directory outside any project adds a warning and other projects are still found.
    #[cfg(unix)]
    #[test]
    fn io_error_outside_projects_is_a_warning() {
        let d = TempDir::new().unwrap();
        write(d.path(), "p/.winnow", "build/\n");
        create(d.path(), &["locked/"]);
        let locked = d.path().join("locked");
        if !lock(&locked) {
            unlock(&locked);
            return;
        }
        let result = scan_all(d.path());
        unlock(&locked);
        assert_eq!(roots(&result.to_clean, d.path()), ["p"]);
        assert_eq!(result.warnings.len(), 1);
    }

    // A scan root that exists but cannot be read is an error.
    #[cfg(unix)]
    #[test]
    fn unreadable_scan_root_is_an_error() {
        let d = TempDir::new().unwrap();
        create(d.path(), &["root/"]);
        let root = d.path().join("root");
        if !lock(&root) {
            unlock(&root);
            return;
        }
        let options = ScanOptions {
            older_than: Duration::ZERO,
            now: SystemTime::now(),
        };
        let result = scan(&root, &options);
        unlock(&root);
        assert!(result.is_err());
    }

    // A scan root that does not exist is an error.
    #[test]
    fn missing_scan_root_is_an_error() {
        let d = TempDir::new().unwrap();
        let options = ScanOptions {
            older_than: Duration::ZERO,
            now: SystemTime::now(),
        };
        assert!(scan(&d.path().join("missing"), &options).is_err());
    }
}
