//! Finding and deleting generated files in projects.

use std::io::{self, BufRead, Write};
use std::path::Path;
use std::time::{Duration, SystemTime};

use anyhow::bail;

use crate::confirm::confirm;
use crate::delete::delete;
use crate::report;
use crate::scan::{ScanOptions, scan};

/// The question asked before anything is deleted.
const PROMPT: &str = "Delete permanently? [y/N] ";

/// Cleans every project under `root` with no source changes for at least `older_than`,
/// asking for confirmation on standard input.
pub fn run(root: &Path, older_than: Duration) -> anyhow::Result<()> {
    let options = ScanOptions {
        older_than,
        now: SystemTime::now(),
    };
    clean(
        root,
        &options,
        &mut io::stdin().lock(),
        &mut io::stdout().lock(),
    )
}

/// Scans `root`, writes the deletion plan to `output`, and asks for confirmation on `input`.
/// If confirmed, deletes every target and writes what was freed.
///
/// Fails if `root` cannot be read, `input` or `output` fails, or any target could not be deleted.
pub fn clean(
    root: &Path,
    options: &ScanOptions,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> anyhow::Result<()> {
    let result = scan(root, options)?;
    write!(output, "{}", report::plan(&result, options))?;
    let targets: Vec<_> = report::projects_with_targets(&result)
        .flat_map(|project| &project.targets)
        .collect();
    if targets.is_empty() {
        return Ok(());
    }
    if !confirm(input, output, PROMPT)? {
        writeln!(output, "Nothing deleted.")?;
        return Ok(());
    }
    let summary = delete(targets);
    write!(output, "{}", report::deletion(&summary))?;
    if !summary.failures.is_empty() {
        bail!(
            "failed to delete {} of {} items",
            summary.failures.len(),
            summary.failures.len() + summary.deleted
        );
    }
    Ok(())
}
