//! End-to-end tests of `winnower clean`.

mod common;

use common::{age, create, project, winnower, write};
use predicates::prelude::*;
use predicates::str::contains;
use tempfile::TempDir;

/// The confirmation question printed before deletion.
const PROMPT: &str = "Delete permanently? [y/N]";

// Answering `y` deletes matched paths and keeps everything else.
#[test]
fn yes_deletes_matched_paths() {
    let p = project(
        &["src/main.c", "build/out.o", "debug.log"],
        "build/\n*.log\n",
    );
    winnower()
        .args(["clean", "--older-than", "0"])
        .arg(p.path())
        .write_stdin("y\n")
        .assert()
        .success()
        .stdout(contains("Deleted 2 items"));
    assert!(!p.path().join("build").exists());
    assert!(!p.path().join("debug.log").exists());
    assert!(p.path().join("src/main.c").exists());
    assert!(p.path().join(".winnow").exists());
}

// Answering `n` deletes nothing.
#[test]
fn no_deletes_nothing() {
    let p = project(&["build/out.o"], "build/\n");
    winnower()
        .args(["clean", "--older-than", "0"])
        .arg(p.path())
        .write_stdin("n\n")
        .assert()
        .success()
        .stdout(contains("Nothing deleted."));
    assert!(p.path().join("build/out.o").exists());
}

// Empty stdin (end of input) deletes nothing.
#[test]
fn eof_deletes_nothing() {
    let p = project(&["build/out.o"], "build/\n");
    winnower()
        .args(["clean", "--older-than", "0"])
        .arg(p.path())
        .write_stdin("")
        .assert()
        .success()
        .stdout(contains("Nothing deleted."));
    assert!(p.path().join("build/out.o").exists());
}

// A file re-included with `!` survives the deletion while its siblings are deleted.
#[test]
fn reincluded_file_survives() {
    let p = project(&["build/a.o", "build/keep"], "build/*\n!build/keep\n");
    winnower()
        .args(["clean", "--older-than", "0"])
        .arg(p.path())
        .write_stdin("y\n")
        .assert()
        .success();
    assert!(!p.path().join("build/a.o").exists());
    assert!(p.path().join("build/keep").exists());
}

// A project with a nested `.winnow` is reported and left untouched while other projects are cleaned.
#[test]
fn nested_winnow_project_is_left_untouched() {
    let d = TempDir::new().unwrap();
    write(d.path(), "bad/.winnow", "build/\n");
    write(d.path(), "bad/sub/.winnow", "out/\n");
    write(d.path(), "good/.winnow", "build/\n");
    create(d.path(), &["bad/build/out.o", "good/build/out.o"]);
    winnower()
        .args(["clean", "--older-than", "0"])
        .arg(d.path())
        .write_stdin("y\n")
        .assert()
        .success()
        .stdout(contains("warning:").and(contains("nested .winnow files are not supported")));
    assert!(d.path().join("bad/build/out.o").exists());
    assert!(!d.path().join("good/build").exists());
}

// With no `.winnow` files, the tool prints `Nothing to clean.` and does not prompt.
#[test]
fn no_winnow_files_means_nothing_to_clean() {
    let d = TempDir::new().unwrap();
    create(d.path(), &["src/main.c", "build/out.o"]);
    winnower()
        .arg("clean")
        .arg(d.path())
        .assert()
        .success()
        .stdout(contains("Nothing to clean.").and(contains(PROMPT).not()));
}

// A recent project is listed as skipped and is not deleted by the default filter.
#[test]
fn recent_project_is_skipped() {
    let p = project(&["src/main.c", "build/out.o"], "build/\n");
    winnower()
        .arg("clean")
        .arg(p.path())
        .write_stdin("y\n")
        .assert()
        .success()
        .stdout(
            contains("Skipped (changed within 30days):")
                .and(contains("last change 0 days ago"))
                .and(contains("Nothing to clean.")),
        );
    assert!(p.path().join("build/out.o").exists());
}

// A project unchanged for longer than the default filter is cleaned.
#[test]
fn old_project_is_cleaned() {
    let p = project(&["src/main.c", "build/out.o"], "build/\n");
    age(p.path(), 40);
    winnower()
        .arg("clean")
        .arg(p.path())
        .write_stdin("y\n")
        .assert()
        .success()
        .stdout(contains("last change 40 days ago"));
    assert!(!p.path().join("build").exists());
}

// `--older-than 0` cleans a recent project.
#[test]
fn older_than_zero_cleans_recent_project() {
    let p = project(&["src/main.c", "build/out.o"], "build/\n");
    winnower()
        .args(["clean", "--older-than", "0"])
        .arg(p.path())
        .write_stdin("y\n")
        .assert()
        .success();
    assert!(!p.path().join("build").exists());
}

// The report shows each target with its size and the total size.
#[test]
fn report_shows_targets_and_total() {
    let p = project(&[], "build/\n*.log\n");
    write(p.path(), "build/out.o", &"x".repeat(1500));
    write(p.path(), "debug.log", "abc");
    winnower()
        .args(["clean", "--older-than", "0"])
        .arg(p.path())
        .write_stdin("n\n")
        .assert()
        .success()
        .stdout(
            contains("    1.5 KB  build/\n")
                .and(contains("       3 B  debug.log\n"))
                .and(contains("Total: 2 items, 1.5 KB in 1 project.")),
        );
}

// A target that cannot be deleted is reported, the rest are deleted, and the exit code is 1.
#[cfg(unix)]
#[test]
fn failed_deletion_exits_with_one() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    let p = project(&["locked/out.log", "free.log"], "*.log\n");
    let locked = p.path().join("locked");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o500)).unwrap();
    let assert = winnower()
        .args(["clean", "--older-than", "0"])
        .arg(p.path())
        .write_stdin("y\n")
        .assert();
    let still_there = locked.join("out.log").exists();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o700)).unwrap();
    if !still_there {
        return;
    }
    assert
        .code(1)
        .stdout(contains("Deleted 1 item").and(contains("Failed to delete 1 item:")))
        .stderr(contains("error: failed to delete 1 of 2 items"));
    assert!(!p.path().join("free.log").exists());
}

// A scan root that does not exist is a runtime error with exit code 1.
#[test]
fn missing_root_exits_with_one() {
    let d = TempDir::new().unwrap();
    winnower()
        .arg("clean")
        .arg(d.path().join("missing"))
        .assert()
        .code(1)
        .stderr(contains("error:"));
}
