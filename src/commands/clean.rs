//! Finding and deleting generated files in projects.

use std::path::Path;
use std::time::Duration;

/// Cleans every project under `root` with no source changes for at least `older_than`. Does nothing yet.
pub fn run(_root: &Path, _older_than: Duration) -> anyhow::Result<()> {
    Ok(())
}
