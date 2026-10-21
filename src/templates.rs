//! The built-in `.winnow` templates.

/// A named `.winnow` file whose first line is a comment describing it.
pub struct Template {
    /// The name used on the command line, for example `cmake`.
    pub name: &'static str,
    /// The text of the template, starting with the description comment.
    pub content: &'static str,
}

impl Template {
    /// Returns the first line of the template, the comment that describes it.
    pub fn description_line(&self) -> &'static str {
        self.content.lines().next().unwrap_or("")
    }

    /// Returns the description without the leading `#` and surrounding spaces.
    pub fn description(&self) -> &'static str {
        self.description_line().trim_start_matches('#').trim()
    }
}

/// Templates embedded in the binary from the `templates/` directory, sorted by name.
pub const BUILTIN: &[Template] = &[
    Template {
        name: "cargo",
        content: include_str!("../templates/cargo.winnow"),
    },
    Template {
        name: "cmake",
        content: include_str!("../templates/cmake.winnow"),
    },
    Template {
        name: "gradle",
        content: include_str!("../templates/gradle.winnow"),
    },
    Template {
        name: "maven",
        content: include_str!("../templates/maven.winnow"),
    },
    Template {
        name: "npm",
        content: include_str!("../templates/npm.winnow"),
    },
    Template {
        name: "python",
        content: include_str!("../templates/python.winnow"),
    },
    Template {
        name: "unity",
        content: include_str!("../templates/unity.winnow"),
    },
];

/// Returns the template called `name`, if there is one.
pub fn find_template(name: &str) -> Option<&'static Template> {
    BUILTIN.iter().find(|template| template.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::winnow_file::Matcher;
    use std::collections::HashSet;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    // Every file in `templates/` is registered in `BUILTIN` under its file name with the same content.
    #[test]
    fn every_template_file_is_registered() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("templates");
        let mut files = 0;
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            assert_eq!(path.extension().unwrap(), "winnow", "{}", path.display());
            let name = path.file_stem().unwrap().to_str().unwrap();
            let template = find_template(name)
                .unwrap_or_else(|| panic!("{} is not in BUILTIN", path.display()));
            assert_eq!(template.content, fs::read_to_string(&path).unwrap());
            files += 1;
        }
        assert_eq!(files, BUILTIN.len());
    }

    // Every built-in template parses as a valid `.winnow` file.
    #[test]
    fn every_template_is_valid() {
        for template in BUILTIN {
            let dir = TempDir::new().unwrap();
            let winnow_file = dir.path().join(".winnow");
            fs::write(&winnow_file, template.content).unwrap();
            if let Err(err) = Matcher::load(&winnow_file) {
                panic!("template {} is invalid: {err:#}", template.name);
            }
        }
    }

    /// Loads the template called `name` into a temporary project.
    fn load(name: &str) -> (TempDir, Matcher) {
        let dir = TempDir::new().unwrap();
        let winnow_file = dir.path().join(".winnow");
        fs::write(&winnow_file, find_template(name).unwrap().content).unwrap();
        let matcher = Matcher::load(&winnow_file).unwrap();
        (dir, matcher)
    }

    // The python template matches the root `venv` but not the stdlib `venv` package sources.
    #[test]
    fn python_matches_only_root_venv() {
        let (dir, matcher) = load("python");
        assert!(matcher.is_match(&dir.path().join("venv"), true));
        assert!(!matcher.is_match(&dir.path().join("Lib/venv"), true));
        assert!(!matcher.is_match(&dir.path().join("pkg/stdlib/venv"), true));
    }

    // The gradle template matches the root `build` but not a `build` package or docs directory.
    #[test]
    fn gradle_matches_only_root_build() {
        let (dir, matcher) = load("gradle");
        assert!(matcher.is_match(&dir.path().join("build"), true));
        let package = dir.path().join("src/main/java/com/example/build");
        assert!(!matcher.is_match(&package, true));
        assert!(!matcher.is_match(&dir.path().join("docs/build"), true));
    }

    // Every built-in template starts with a non-empty description comment.
    #[test]
    fn every_template_starts_with_description() {
        for template in BUILTIN {
            assert!(
                template.description_line().starts_with("# "),
                "template {} has no description comment",
                template.name
            );
            assert!(!template.description().is_empty(), "{}", template.name);
        }
    }

    // Template names are unique.
    #[test]
    fn names_are_unique() {
        let names: HashSet<_> = BUILTIN.iter().map(|template| template.name).collect();
        assert_eq!(names.len(), BUILTIN.len());
    }

    // Template descriptions are unique.
    #[test]
    fn descriptions_are_unique() {
        let descriptions: HashSet<_> = BUILTIN.iter().map(Template::description).collect();
        assert_eq!(descriptions.len(), BUILTIN.len());
    }

    // Templates are sorted by name.
    #[test]
    fn templates_are_sorted() {
        assert!(BUILTIN.is_sorted_by_key(|template| template.name));
    }

    // The description is the first line without `#`.
    #[test]
    fn description_strips_comment_marker() {
        let template = Template {
            name: "x",
            content: "# Some files\nout/\n",
        };
        assert_eq!(template.description_line(), "# Some files");
        assert_eq!(template.description(), "Some files");
    }

    // An unknown name finds no template.
    #[test]
    fn unknown_name_finds_nothing() {
        assert!(find_template("cmake").is_some());
        assert!(find_template("nonexistent").is_none());
    }
}
