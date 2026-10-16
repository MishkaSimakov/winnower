//! End-to-end tests of `winnower init` and `winnower templates`.

mod common;

use common::{age, create, winnower};
use predicates::prelude::*;
use predicates::str::contains;
use tempfile::TempDir;
use winnower::templates::BUILTIN;

// `winnower init npm` creates a `.winnow` that `clean` then uses.
#[test]
fn init_creates_file_used_by_clean() {
    let d = TempDir::new().unwrap();
    winnower()
        .args(["init", "npm", "--dir"])
        .arg(d.path())
        .assert()
        .success()
        .stdout(contains("Added npm to "));
    create(d.path(), &["src/index.js", "node_modules/pkg/index.js"]);
    winnower()
        .args(["clean", "--older-than", "0"])
        .arg(d.path())
        .write_stdin("y\n")
        .assert()
        .success()
        .stdout(contains("node_modules/").and(contains("Deleted 1 item")));
    assert!(!d.path().join("node_modules").exists());
    assert!(d.path().join("src/index.js").exists());
}

// Running `init` in a stale project does not stop the default `clean` from cleaning it.
#[test]
fn init_keeps_stale_project_inactive() {
    let d = TempDir::new().unwrap();
    create(d.path(), &["src/index.js", "node_modules/pkg/index.js"]);
    age(d.path(), 90);
    winnower()
        .args(["init", "npm", "--dir"])
        .arg(d.path())
        .assert()
        .success();
    winnower()
        .arg("clean")
        .arg(d.path())
        .write_stdin("y\n")
        .assert()
        .success()
        .stdout(contains("Deleted 1 item"));
    assert!(!d.path().join("node_modules").exists());
}

// An unknown template name exits with 1, names the available templates, and creates no file.
#[test]
fn init_unknown_template_fails() {
    let d = TempDir::new().unwrap();
    winnower()
        .args(["init", "nonexistent", "--dir"])
        .arg(d.path())
        .assert()
        .code(1)
        .stderr(contains("unknown template 'nonexistent'").and(contains("cmake")));
    assert!(!d.path().join(".winnow").exists());
}

// `winnower templates` lists every built-in template with its description.
#[test]
fn templates_lists_every_builtin() {
    let assert = winnower().arg("templates").assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    for template in BUILTIN {
        assert!(
            stdout
                .lines()
                .any(|line| line.starts_with(template.name)
                    && line.ends_with(template.description())),
            "{} is not listed:\n{stdout}",
            template.name
        );
    }
}
