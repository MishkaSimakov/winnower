# winnower

> His winnowing fork is in his hand, and he will clear his threshing floor and gather his wheat
> into the barn, but the chaff he will burn with unquenchable fire.
> — Matthew 3:12 (ESV)

**winnower frees disk space by deleting generated files (`node_modules`, `cmake-build-*`,
`target`, …) across all your projects, using a per-project list of what is safe to delete.**

Each project lists its disposable paths in a `.winnow` file, written in `.gitignore` syntax.
`winnower clean` walks a directory tree, finds every `.winnow` file, shows what would be deleted
and how much space it takes, asks for confirmation, and deletes.

![winnower clean deleting build directories from two old projects and skipping a recent one](docs/demo.gif)

> [!WARNING]
> **Deletion is permanent.** Files are not moved to the Trash, and there is no undo. The
> confirmation prompt is the only safeguard: read the list before answering `y`.

## Install

**Shell installer** (macOS and Linux, no Rust toolchain needed):

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/MishkaSimakov/winnower/releases/latest/download/winnower-installer.sh | sh
```

**Manual download:** get the archive for your platform from the
[releases page](https://github.com/MishkaSimakov/winnower/releases), unpack it, and move
`winnower` to a directory on your `PATH` (for example `/usr/local/bin`).

**Cargo:**

```sh
cargo install winnower          # build from source
cargo binstall winnower         # download a prebuilt binary
```

## Quick start

Create a `.winnow` file in a project from a built-in template:

```console
$ cd ~/projects/raytracer
$ winnower init cmake
Added cmake to ./.winnow
$ cat .winnow
# CMake and CLion build directories
cmake-build-*/
```

Then clean every project under a directory:

```sh
winnower clean ~/projects
```

Answering anything other than `y` or `yes` deletes nothing, so `n` works as a dry run.

## Usage

### `winnower clean [PATH] [--older-than <DURATION>]`

Finds every `.winnow` file under `PATH` (default: the current directory), lists the matched
paths with their sizes, asks for confirmation, and deletes them.

`--older-than` (default `30d`) only cleans projects you have not touched for that long, so the
build directory of the project you are working on today is left alone. A project's last change
is the newest modification time of its files that are **not** matched by `.winnow`, not
counting `.winnow` itself. Durations
such as `12h`, `2w`, or `6months` are accepted. `--older-than 0` cleans every project.

Exit code `0` means success, nothing to do, or the deletion was declined. `1` means a runtime
error, for example some paths could not be deleted.

### `winnower init <TEMPLATE>... [--dir <PATH>]`

Writes the named templates into `.winnow` in `--dir` (default: the current directory). Several
templates can be combined:

```sh
winnower init npm python
```

If `.winnow` already exists, the templates are appended. A template that is already in the file
is skipped, so running `init` again is safe.

### `winnower templates`

Lists the built-in templates:

```console
$ winnower templates
cargo    Rust / Cargo build output
cmake    CMake and CLion build directories
gradle   Gradle caches and build output
maven    Maven build output
npm      Node.js dependencies and framework caches
python   Python virtualenvs and tool caches
unity    Unity generated folders
```

Templates are conservative: they only list paths that tools always regenerate. The full
contents are in [`templates/`](templates).

## The `.winnow` format

A `.winnow` file uses `.gitignore` syntax, with patterns relative to the directory that contains
it. A matched directory is deleted as a whole.

```gitignore
# CLion build directories
cmake-build-*/

# Node dependencies, only at project root
/node_modules/

# Keep the downloaded model, delete other data
/data/*
!/data/model.bin
```

See [docs/winnow-format.md](docs/winnow-format.md) for the full rules, including how negation
works, how symlinks are handled, and why nested `.winnow` files are rejected.

## Comparison

|                                 | winnower                  | `git clean -X`            | [kondo]                        | [npkill]                                    |
| ------------------------------- | ------------------------- | ------------------------- | ------------------------------ | ------------------------------------------- |
| What gets deleted               | Paths listed in `.winnow` | Everything git ignores    | Built-in list per project type | Directories by name (`node_modules` by default) |
| Works across many projects      | Yes                       | No, one repository        | Yes                            | Yes                                         |
| Needs a git repository          | No                        | Yes                       | No                             | No                                          |
| Customizable per project        | Yes                       | Only through `.gitignore` | No                             | No, one global list of names                |
| Skips recently changed projects | Yes, `--older-than`       | No                        | Yes, `--older`                 | No                                          |

`.gitignore` usually lists more than build output: `.env` files, local configuration, IDE
settings, downloaded data. `git clean -X` deletes all of it. A `.winnow` file is a separate,
explicit list of what is safe to delete, and it works for any kind of project, including ones
that are not git repositories.

[kondo]: https://github.com/tbillington/kondo
[npkill]: https://github.com/voidcosmos/npkill

## License

[MIT](LICENSE)
