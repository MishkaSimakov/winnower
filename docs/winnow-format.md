# The `.winnow` file format

A `.winnow` file lists the paths in a project that winnower may delete. **The format is the
[`.gitignore` format](https://git-scm.com/docs/gitignore)**, with one extension: brace
alternation (see [rule 8](#8-difference-from-git-brace-alternation)). Matching is done by the
[`ignore`](https://crates.io/crates/ignore) crate, the one ripgrep uses, which implements git's
rules.

Every path a pattern matches is deleted.

## Rules (same as `.gitignore`)

| Pattern                        | Meaning                                                                                                                         |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------- |
| `# text`                       | Comment.                                                                                                                        |
| blank line                     | Ignored.                                                                                                                        |
| `node_modules/`                | Trailing `/` matches **directories only**.                                                                                      |
| `build`                        | No slash except a trailing one, so it matches a file or directory named `build` **at any depth** under the `.winnow` directory. |
| `/build`                       | Leading `/` anchors to the directory that contains `.winnow`, so only `<project>/build`.                                        |
| `docs/build`                   | A slash in the middle also anchors the pattern relative to the `.winnow` directory.                                             |
| `cmake-build-*`                | `*` matches anything except `/`. `?` matches one character. `[a-z]` matches a range.                                            |
| `**/logs`, `logs/**`, `a/**/b` | `**` matches any number of directories.                                                                                         |
| `!pattern`                     | Negation: re-includes a path that an earlier pattern matched.                                                                   |
| `\#file`, `\!file`             | Backslash escapes a leading `#` or `!`.                                                                                         |
| trailing spaces                | Ignored unless escaped with `\`.                                                                                                |

## Winnower-specific behavior

These all follow from "matched ⇒ delete" plus git's own semantics.

### 1. Patterns are relative to the project root

The directory that contains the `.winnow` file is the **project root**. Patterns are matched
against paths relative to it.

### 2. A matched directory is deleted as a whole

As in git, `!` cannot re-include a path whose parent directory is already matched. This file
deletes all of `build/`, including `keep`:

```gitignore
build/
!build/keep
```

To keep one file, match the directory's **contents** instead of the directory:

```gitignore
build/*
!build/keep
```

Here `build/` itself is not matched, so winnower looks inside it, deletes every entry except
`keep`, and leaves `build/` in place.

### 3. Nested `.winnow` files are not allowed

When winnower finds a `.winnow` file in a directory, the whole subtree belongs to that project.
If another `.winnow` file exists anywhere inside that subtree (including inside matched
directories), winnower reports an error naming both files and skips the project. Nothing in it
is deleted. Other projects are cleaned as usual.

### 4. `.git` is never touched

`.git` directories are never matched or entered, the same as in git.

### 5. A pattern can match `.winnow` itself

A pattern such as `.*` also matches the `.winnow` file. In that case `.winnow` is deleted like
any other match.

### 6. Symbolic links are not followed

If a symlink matches, the link itself is deleted, not its target. Symlinks that do not match are
not entered either.

### 7. Filesystem boundaries are not crossed

Mounted volumes inside a project are skipped.

### 8. Difference from git: brace alternation

The `ignore` crate supports `{a,b}`, which matches either `a` or `b`. Git treats braces as
literal characters. So `out{1,2}` matches `out1` and `out2` in winnower, but only a file
literally named `out{1,2}` in git.

An unbalanced brace (`a{b`) is an invalid pattern. To match a literal brace, write `[{]` or
`[}]`.

## Invalid files

If a `.winnow` file contains an invalid pattern, winnower prints a warning and skips the whole
project. Nothing in that project is deleted.

## Project activity

`winnower clean --older-than <DURATION>` (default `30d`) only cleans projects with no activity
for at least that long. A project's activity is the newest modification time of any file in it
that is **not** matched by `.winnow`: source files, not build output. The `.winnow` file itself
and `.git` contents are not counted, so adding a `.winnow` file to a stale project does not make
it look active. A project with no other unmatched files counts as inactive.

## Example

```gitignore
# CLion build directories
cmake-build-*/

# Node dependencies, only at project root
/node_modules/

# Python caches anywhere in the tree
__pycache__/
.pytest_cache/

# Keep the downloaded model, delete other data
/data/*
!/data/model.bin
```
