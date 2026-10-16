//! Text shown to the user before and after deletion.

use std::fmt::Write;
use std::path::Path;
use std::time::SystemTime;

use crate::delete::DeleteSummary;
use crate::scan::{Project, ScanOptions, ScanResult, TargetKind};
use crate::units::{format_days_ago, format_size};

/// Formats the deletion plan: warnings, each project with targets and its targets,
/// the projects skipped as recently changed, and a final line with the totals,
/// or `Nothing to clean.` if there are no targets.
pub fn plan(result: &ScanResult, options: &ScanOptions) -> String {
    let mut text = String::new();
    for warning in &result.warnings {
        let _ = writeln!(text, "warning: {warning}");
    }
    let projects: Vec<&Project> = projects_with_targets(result).collect();
    let width = root_column_width(&projects, &result.skipped_recent);
    for project in &projects {
        let root = project.root.display().to_string();
        let _ = writeln!(text, "{root:<width$}  {}", activity(project, options.now));
        for target in &project.targets {
            let relative = target
                .path
                .strip_prefix(&project.root)
                .unwrap_or(&target.path);
            let slash = if target.kind == TargetKind::Dir {
                "/"
            } else {
                ""
            };
            let size = format_size(target.size);
            let _ = writeln!(text, "{size:>10}  {}{slash}", relative.display());
        }
    }
    if !result.skipped_recent.is_empty() {
        let within = humantime::format_duration(options.older_than);
        let _ = writeln!(text, "\nSkipped (changed within {within}):");
        for project in &result.skipped_recent {
            let root = format!("  {}", project.root.display());
            let _ = writeln!(text, "{root:<width$}  {}", activity(project, options.now));
        }
    }
    if projects.is_empty() {
        let _ = writeln!(text, "\nNothing to clean.");
    } else {
        let items = projects.iter().map(|project| project.targets.len()).sum();
        let size = projects
            .iter()
            .flat_map(|project| &project.targets)
            .map(|target| target.size)
            .sum();
        let _ = writeln!(
            text,
            "\nTotal: {}, {} in {}.",
            count(items, "item"),
            format_size(size),
            count(projects.len(), "project")
        );
    }
    text.trim_start().to_string()
}

/// Formats the result of deletion: what was removed and freed, and each failure.
pub fn deletion(summary: &DeleteSummary) -> String {
    let mut text = format!(
        "Deleted {}, freed {}.\n",
        count(summary.deleted, "item"),
        format_size(summary.freed)
    );
    if !summary.failures.is_empty() {
        let _ = writeln!(
            text,
            "Failed to delete {}:",
            count(summary.failures.len(), "item")
        );
        for failure in &summary.failures {
            let _ = writeln!(text, "  {}: {}", failure.path.display(), failure.error);
        }
    }
    text
}

/// Returns the projects to clean that have at least one target.
pub fn projects_with_targets(result: &ScanResult) -> impl Iterator<Item = &Project> {
    result
        .to_clean
        .iter()
        .filter(|project| !project.targets.is_empty())
}

/// Returns the width of the column of project roots: the longest root to clean,
/// or the longest skipped root plus its two-space indent.
fn root_column_width(to_clean: &[&Project], skipped: &[Project]) -> usize {
    let to_clean = to_clean.iter().map(|project| display_len(&project.root));
    let skipped = skipped.iter().map(|project| display_len(&project.root) + 2);
    to_clean.chain(skipped).max().unwrap_or(0)
}

/// Returns the number of characters in the displayed form of `path`.
fn display_len(path: &Path) -> usize {
    path.display().to_string().chars().count()
}

/// Describes the last activity of `project`, for example `last change 94 days ago`.
fn activity(project: &Project, now: SystemTime) -> String {
    if project.last_activity == SystemTime::UNIX_EPOCH {
        "no unmatched files".to_string()
    } else {
        format!(
            "last change {}",
            format_days_ago(project.last_activity, now)
        )
    }
}

/// Formats `n` followed by `noun`, adding `s` unless `n` is 1.
fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::delete::Failure;
    use crate::scan::Target;
    use std::io;
    use std::path::PathBuf;
    use std::time::Duration;

    const DAY: Duration = Duration::from_secs(24 * 60 * 60);

    /// Returns a project at `root`, last changed `days` days before `now`,
    /// with a target for each `(relative path, kind, size)`.
    fn project(
        now: SystemTime,
        root: &str,
        days: u32,
        targets: &[(&str, TargetKind, u64)],
    ) -> Project {
        Project {
            root: PathBuf::from(root),
            last_activity: now - DAY * days,
            targets: targets
                .iter()
                .map(|&(path, kind, size)| Target {
                    path: Path::new(root).join(path),
                    kind,
                    size,
                })
                .collect(),
        }
    }

    /// Returns options with a 30-day filter at `now`.
    fn options(now: SystemTime) -> ScanOptions {
        ScanOptions {
            older_than: 30 * DAY,
            now,
        }
    }

    // The plan lists each project with its targets, the skipped projects, and the totals, in aligned columns.
    #[test]
    fn plan_lists_projects_targets_and_totals() {
        let now = SystemTime::now();
        let result = ScanResult {
            to_clean: vec![
                project(
                    now,
                    "/p/game",
                    94,
                    &[
                        ("Library", TargetKind::Dir, 1_200_000_000),
                        ("Temp", TargetKind::Dir, 310_000_000),
                    ],
                ),
                project(
                    now,
                    "/p/raytracer",
                    41,
                    &[("out.log", TargetKind::File, 12)],
                ),
            ],
            skipped_recent: vec![project(now, "/p/winnower", 0, &[])],
            warnings: vec![],
        };
        let expected = "\
/p/game        last change 94 days ago
    1.2 GB  Library/
  310.0 MB  Temp/
/p/raytracer   last change 41 days ago
      12 B  out.log

Skipped (changed within 30days):
  /p/winnower  last change 0 days ago

Total: 3 items, 1.5 GB in 2 projects.
";
        assert_eq!(plan(&result, &options(now)), expected);
    }

    // Warnings come first, and a plan without targets ends with `Nothing to clean.`.
    #[test]
    fn plan_without_targets_says_nothing_to_clean() {
        let now = SystemTime::now();
        let result = ScanResult {
            to_clean: vec![project(now, "/p/empty", 40, &[])],
            skipped_recent: vec![],
            warnings: vec!["skipping project /p/bad: invalid".to_string()],
        };
        let expected = "\
warning: skipping project /p/bad: invalid

Nothing to clean.
";
        assert_eq!(plan(&result, &options(now)), expected);
    }

    // A plan with no warnings, projects, or skipped projects is only `Nothing to clean.`.
    #[test]
    fn empty_plan_is_nothing_to_clean() {
        let now = SystemTime::now();
        let result = ScanResult {
            to_clean: vec![],
            skipped_recent: vec![],
            warnings: vec![],
        };
        assert_eq!(plan(&result, &options(now)), "Nothing to clean.\n");
    }

    // A project with no unmatched files is described as such, and one target is counted in the singular.
    #[test]
    fn project_without_unmatched_files() {
        let now = SystemTime::now();
        let mut inactive = project(now, "/p/a", 0, &[("out", TargetKind::File, 1)]);
        inactive.last_activity = SystemTime::UNIX_EPOCH;
        let result = ScanResult {
            to_clean: vec![inactive],
            skipped_recent: vec![],
            warnings: vec![],
        };
        let text = plan(&result, &options(now));
        assert!(text.starts_with("/p/a  no unmatched files\n"));
        assert!(text.ends_with("Total: 1 item, 1 B in 1 project.\n"));
    }

    // The deletion summary lists the freed size and each failure.
    #[test]
    fn deletion_lists_failures() {
        let summary = DeleteSummary {
            deleted: 3,
            freed: 2_000_000,
            failures: vec![Failure {
                path: PathBuf::from("/p/build"),
                error: io::Error::other("denied"),
            }],
        };
        let expected = "\
Deleted 3 items, freed 2.0 MB.
Failed to delete 1 item:
  /p/build: denied
";
        assert_eq!(deletion(&summary), expected);
    }
}
