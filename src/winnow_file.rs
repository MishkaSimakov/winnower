//! Loading `.winnow` files and matching paths against their patterns.

use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use ignore::gitignore::{Gitignore, GitignoreBuilder};

/// The patterns of one `.winnow` file, matched with `.gitignore` semantics
/// relative to the directory that contains the file.
pub struct Matcher {
    root: PathBuf,
    gitignore: Gitignore,
}

impl Matcher {
    /// Reads `winnow_file` and builds a matcher rooted at its parent directory.
    ///
    /// Fails if the file cannot be read or contains an invalid pattern.
    pub fn load(winnow_file: &Path) -> anyhow::Result<Matcher> {
        let root = winnow_file.parent().unwrap_or(Path::new(""));
        let mut builder = GitignoreBuilder::new(root);
        if let Some(err) = builder.add(winnow_file) {
            bail!(err);
        }
        let gitignore = builder
            .build()
            .with_context(|| format!("building matcher for {}", winnow_file.display()))?;
        Ok(Matcher {
            root: root.to_path_buf(),
            gitignore,
        })
    }

    /// Returns whether `path` is matched, either directly or because one of its
    /// parent directories below the project root is matched.
    ///
    /// `path` must start with the project root; other paths and the root itself never match.
    pub fn is_match(&self, path: &Path, is_dir: bool) -> bool {
        let Ok(relative) = path.strip_prefix(&self.root) else {
            return false;
        };
        if relative.as_os_str().is_empty() {
            return false;
        }
        let parent_matched = relative
            .ancestors()
            .skip(1)
            .filter(|parent| !parent.as_os_str().is_empty())
            .any(|parent| self.gitignore.matched(parent, true).is_ignore());
        parent_matched || self.gitignore.matched(relative, is_dir).is_ignore()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Writes `contents` to `.winnow` in a new temporary directory and loads it.
    fn matcher(contents: &str) -> (TempDir, Matcher) {
        let dir = TempDir::new().unwrap();
        let winnow_file = dir.path().join(".winnow");
        fs::write(&winnow_file, contents).unwrap();
        let matcher = Matcher::load(&winnow_file).unwrap();
        (dir, matcher)
    }

    /// Returns whether the file at `relative` inside the project is matched.
    fn file(dir: &TempDir, matcher: &Matcher, relative: &str) -> bool {
        matcher.is_match(&dir.path().join(relative), false)
    }

    /// Returns whether the directory at `relative` inside the project is matched.
    fn dir(dir: &TempDir, matcher: &Matcher, relative: &str) -> bool {
        matcher.is_match(&dir.path().join(relative), true)
    }

    // A pattern without a slash matches a name at any depth.
    #[test]
    fn unanchored_name_matches_at_any_depth() {
        let (d, m) = matcher("build\n");
        assert!(dir(&d, &m, "build"));
        assert!(dir(&d, &m, "a/b/build"));
        assert!(file(&d, &m, "a/build"));
        assert!(!dir(&d, &m, "builds"));
    }

    // A leading slash matches only at the project root.
    #[test]
    fn leading_slash_anchors_to_root() {
        let (d, m) = matcher("/build\n");
        assert!(dir(&d, &m, "build"));
        assert!(!dir(&d, &m, "src/build"));
    }

    // A slash in the middle anchors the pattern to the project root.
    #[test]
    fn middle_slash_anchors_to_root() {
        let (d, m) = matcher("docs/build\n");
        assert!(dir(&d, &m, "docs/build"));
        assert!(!dir(&d, &m, "src/docs/build"));
    }

    // A directory-only pattern does not match a regular file with the same name.
    #[test]
    fn trailing_slash_matches_only_directories() {
        let (d, m) = matcher("build/\n");
        assert!(dir(&d, &m, "build"));
        assert!(!file(&d, &m, "build"));
    }

    // `*` matches any part of a name.
    #[test]
    fn star_matches_within_a_name() {
        let (d, m) = matcher("cmake-build-*/\n");
        assert!(dir(&d, &m, "cmake-build-debug"));
        assert!(dir(&d, &m, "cmake-build-release"));
        assert!(!dir(&d, &m, "cmake-build"));
    }

    // `*` does not match a slash.
    #[test]
    fn star_does_not_cross_directories() {
        let (d, m) = matcher("/docs/*.md\n");
        assert!(file(&d, &m, "docs/a.md"));
        assert!(!file(&d, &m, "docs/sub/a.md"));
    }

    // `?` matches exactly one character.
    #[test]
    fn question_mark_matches_one_character() {
        let (d, m) = matcher("log?.txt\n");
        assert!(file(&d, &m, "log1.txt"));
        assert!(!file(&d, &m, "log.txt"));
        assert!(!file(&d, &m, "log12.txt"));
    }

    // A bracket expression matches one character from the range.
    #[test]
    fn bracket_range_matches_one_character() {
        let (d, m) = matcher("/[Ll]ibrary/\n");
        assert!(dir(&d, &m, "Library"));
        assert!(dir(&d, &m, "library"));
        assert!(!dir(&d, &m, "xibrary"));
    }

    // A leading `**/` matches the name in every directory.
    #[test]
    fn leading_double_star_matches_any_depth() {
        let (d, m) = matcher("**/logs\n");
        assert!(dir(&d, &m, "logs"));
        assert!(dir(&d, &m, "a/b/logs"));
    }

    // A trailing `/**` matches everything inside the directory but not the directory itself.
    #[test]
    fn trailing_double_star_matches_contents() {
        let (d, m) = matcher("logs/**\n");
        assert!(file(&d, &m, "logs/a.txt"));
        assert!(file(&d, &m, "logs/deep/a.txt"));
        assert!(!dir(&d, &m, "logs"));
    }

    // A `/**/` in the middle matches zero or more directories.
    #[test]
    fn middle_double_star_matches_any_directories() {
        let (d, m) = matcher("a/**/b\n");
        assert!(dir(&d, &m, "a/b"));
        assert!(dir(&d, &m, "a/x/y/b"));
        assert!(!dir(&d, &m, "c/a/b"));
    }

    // `!` re-includes a file matched by an earlier pattern.
    #[test]
    fn negation_reincludes_file() {
        let (d, m) = matcher("*.log\n!keep.log\n");
        assert!(file(&d, &m, "debug.log"));
        assert!(!file(&d, &m, "keep.log"));
    }

    // `!build/keep` does not re-include `keep` when `build/` itself is matched.
    #[test]
    fn negation_cannot_reinclude_inside_matched_directory() {
        let (d, m) = matcher("build/\n!build/keep\n");
        assert!(dir(&d, &m, "build"));
        assert!(file(&d, &m, "build/keep"));
    }

    // With `build/*` + `!build/keep`, the contents except `keep` match and `build/` does not.
    #[test]
    fn negation_reincludes_inside_matched_contents() {
        let (d, m) = matcher("build/*\n!build/keep\n");
        assert!(!dir(&d, &m, "build"));
        assert!(!file(&d, &m, "build/keep"));
        assert!(file(&d, &m, "build/other"));
        assert!(dir(&d, &m, "build/sub"));
    }

    // Everything inside a matched directory is matched.
    #[test]
    fn contents_of_matched_directory_match() {
        let (d, m) = matcher("node_modules/\n");
        assert!(file(&d, &m, "node_modules/pkg/index.js"));
        assert!(file(&d, &m, "web/node_modules/pkg/index.js"));
    }

    // Comments and blank lines match nothing.
    #[test]
    fn comments_and_blank_lines_are_ignored() {
        let (d, m) = matcher("# build\n\n   \n");
        assert!(!dir(&d, &m, "build"));
        assert!(!file(&d, &m, "# build"));
    }

    // Escaped `\#` matches a file whose name starts with `#`.
    #[test]
    fn escaped_hash_matches_literal_hash() {
        let (d, m) = matcher("\\#notes\n");
        assert!(file(&d, &m, "#notes"));
    }

    // Escaped `\!` matches a file whose name starts with `!`.
    #[test]
    fn escaped_bang_matches_literal_bang() {
        let (d, m) = matcher("*\n\\!important\n");
        assert!(file(&d, &m, "other"));
        assert!(file(&d, &m, "!important"));
    }

    // Trailing spaces are ignored unless escaped with a backslash.
    #[test]
    fn trailing_spaces_are_trimmed_unless_escaped() {
        let (d, m) = matcher("build   \nout\\ \n");
        assert!(dir(&d, &m, "build"));
        assert!(dir(&d, &m, "out "));
        assert!(!dir(&d, &m, "out"));
    }

    // A pattern can match the `.winnow` file itself.
    #[test]
    fn pattern_can_match_winnow_file() {
        let (d, m) = matcher(".*\n");
        assert!(file(&d, &m, ".winnow"));
    }

    // The project root and paths outside it never match.
    #[test]
    fn root_and_outside_paths_do_not_match() {
        let (d, m) = matcher("*\n");
        assert!(!m.is_match(d.path(), true));
        assert!(!m.is_match(Path::new("/elsewhere/build"), true));
    }

    // An invalid pattern produces an error.
    #[test]
    fn invalid_pattern_is_an_error() {
        let dir = TempDir::new().unwrap();
        let winnow_file = dir.path().join(".winnow");
        fs::write(&winnow_file, "build/\n[z-a]\n").unwrap();
        let err = Matcher::load(&winnow_file).err().unwrap();
        assert!(format!("{err:#}").contains("line 2"));
    }

    // A missing `.winnow` file produces an error.
    #[test]
    fn missing_file_is_an_error() {
        let dir = TempDir::new().unwrap();
        assert!(Matcher::load(&dir.path().join(".winnow")).is_err());
    }
}
