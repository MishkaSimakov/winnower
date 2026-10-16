//! Creating `.winnow` files from templates.

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use anyhow::Context;

use crate::templates::{BUILTIN, Template, find_template};

/// Adds the named templates to `.winnow` in `dir`, reporting on standard output.
pub fn run(dir: &Path, names: &[String]) -> anyhow::Result<()> {
    init(dir, names, &mut io::stdout().lock())
}

/// Adds the named templates to `.winnow` in `dir`, creating the file if it does not exist,
/// and writes what was added and skipped to `output`.
///
/// Each template is appended after a blank line. A template whose description line
/// is already in the file is skipped.
///
/// Fails without writing anything if a name is not a built-in template.
pub fn init(dir: &Path, names: &[String], output: &mut impl Write) -> anyhow::Result<()> {
    let templates = names
        .iter()
        .map(|name| lookup(name))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let path = dir.join(".winnow");
    let mut contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err).with_context(|| format!("reading {}", path.display())),
    };
    let mut added = Vec::new();
    for template in templates {
        if contains_line(&contents, template.description_line()) {
            writeln!(
                output,
                "Skipped {}: already in {}",
                template.name,
                path.display()
            )?;
            continue;
        }
        append(&mut contents, template.content);
        added.push(template.name);
    }
    if added.is_empty() {
        return Ok(());
    }
    fs::write(&path, &contents).with_context(|| format!("writing {}", path.display()))?;
    writeln!(output, "Added {} to {}", added.join(", "), path.display())?;
    Ok(())
}

/// Returns the built-in template called `name`, or an error listing the available names.
fn lookup(name: &str) -> anyhow::Result<&'static Template> {
    find_template(name).with_context(|| {
        let available: Vec<_> = BUILTIN.iter().map(|template| template.name).collect();
        format!(
            "unknown template '{name}' (available: {})",
            available.join(", ")
        )
    })
}

/// Returns whether `text` has a line equal to `line`, ignoring trailing whitespace.
fn contains_line(text: &str, line: &str) -> bool {
    text.lines().any(|existing| existing.trim_end() == line)
}

/// Appends `section` to `contents`, separated from existing text by a blank line.
fn append(contents: &mut String, section: &str) {
    if !contents.is_empty() {
        if !contents.ends_with('\n') {
            contents.push('\n');
        }
        if !contents.ends_with("\n\n") {
            contents.push('\n');
        }
    }
    contents.push_str(section.trim_end());
    contents.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Runs `init` in `dir` with `names` and returns everything written to the output.
    fn run_init(dir: &TempDir, names: &[&str]) -> anyhow::Result<String> {
        let names: Vec<String> = names.iter().map(|name| name.to_string()).collect();
        let mut output = Vec::new();
        init(dir.path(), &names, &mut output)?;
        Ok(String::from_utf8(output).unwrap())
    }

    /// Returns the contents of `.winnow` in `dir`.
    fn winnow(dir: &TempDir) -> String {
        fs::read_to_string(dir.path().join(".winnow")).unwrap()
    }

    /// Returns the content of the built-in template called `name`.
    fn content(name: &str) -> &'static str {
        find_template(name).unwrap().content
    }

    // `init` creates `.winnow` with the chosen template section.
    #[test]
    fn creates_winnow_file() {
        let dir = TempDir::new().unwrap();
        let output = run_init(&dir, &["cmake"]).unwrap();
        assert_eq!(winnow(&dir), content("cmake"));
        assert!(output.starts_with("Added cmake to "));
    }

    // `init` with two templates writes both sections in order, separated by a blank line.
    #[test]
    fn writes_several_templates_in_order() {
        let dir = TempDir::new().unwrap();
        let output = run_init(&dir, &["python", "cmake"]).unwrap();
        let expected = format!("{}\n{}", content("python"), content("cmake"));
        assert_eq!(winnow(&dir), expected);
        assert!(output.starts_with("Added python, cmake to "));
    }

    // `init` appends to an existing `.winnow` and keeps the existing content.
    #[test]
    fn appends_to_existing_file() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".winnow"), "/data/").unwrap();
        run_init(&dir, &["cmake"]).unwrap();
        assert_eq!(winnow(&dir), format!("/data/\n\n{}", content("cmake")));
    }

    // `init` skips a template whose description line is already present.
    #[test]
    fn skips_template_already_present() {
        let dir = TempDir::new().unwrap();
        run_init(&dir, &["cmake"]).unwrap();
        let output = run_init(&dir, &["cmake", "python"]).unwrap();
        let expected = format!("{}\n{}", content("cmake"), content("python"));
        assert_eq!(winnow(&dir), expected);
        assert!(output.starts_with("Skipped cmake: already in "));
        assert!(output.contains("Added python to "));
    }

    // A template named twice in one call is written once.
    #[test]
    fn repeated_name_is_written_once() {
        let dir = TempDir::new().unwrap();
        run_init(&dir, &["npm", "npm"]).unwrap();
        assert_eq!(winnow(&dir), content("npm"));
    }

    // When every template is already present, the file is left unchanged.
    #[test]
    fn nothing_new_leaves_file_unchanged() {
        let dir = TempDir::new().unwrap();
        let existing = format!("{}\n\n\n", content("cargo"));
        fs::write(dir.path().join(".winnow"), &existing).unwrap();
        let output = run_init(&dir, &["cargo"]).unwrap();
        assert_eq!(winnow(&dir), existing);
        assert!(!output.contains("Added"));
    }

    // An unknown template name writes nothing and returns an error listing the available names.
    #[test]
    fn unknown_template_writes_nothing() {
        let dir = TempDir::new().unwrap();
        let err = run_init(&dir, &["cmake", "nonexistent"]).unwrap_err();
        let message = format!("{err:#}");
        assert!(message.contains("unknown template 'nonexistent'"));
        assert!(message.contains("cargo, cmake,"));
        assert!(!dir.path().join(".winnow").exists());
    }

    // A section is separated from existing text by exactly one blank line.
    #[test]
    fn append_adds_one_blank_line() {
        for existing in ["a", "a\n", "a\n\n"] {
            let mut contents = existing.to_string();
            append(&mut contents, "b\n");
            assert_eq!(contents, "a\n\nb\n", "{existing:?}");
        }
        let mut contents = String::new();
        append(&mut contents, "b");
        assert_eq!(contents, "b\n");
    }
}
