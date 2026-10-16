//! Listing built-in templates.

use std::fmt::Write;

use crate::templates::BUILTIN;

/// Prints each built-in template with its description.
pub fn run() -> anyhow::Result<()> {
    print!("{}", list());
    Ok(())
}

/// Formats each built-in template name and description on its own line, in aligned columns.
pub fn list() -> String {
    let width = BUILTIN
        .iter()
        .map(|template| template.name.len())
        .max()
        .unwrap_or(0);
    let mut text = String::new();
    for template in BUILTIN {
        let _ = writeln!(
            text,
            "{:<width$}   {}",
            template.name,
            template.description()
        );
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    // Each template is listed on its own line with its description in an aligned column.
    #[test]
    fn list_shows_names_and_descriptions() {
        let text = list();
        assert_eq!(text.lines().count(), BUILTIN.len());
        assert!(text.contains("cargo    Rust / Cargo build output\n"));
        assert!(text.contains("python   Python virtualenvs and tool caches\n"));
    }
}
