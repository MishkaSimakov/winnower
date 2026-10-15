//! Command-line interface definitions.

use std::path::PathBuf;
use std::time::Duration;

use clap::{Args, Parser, Subcommand};

/// Top-level command-line arguments.
#[derive(Debug, Parser)]
#[command(version, about)]
pub struct Cli {
    /// The subcommand to run.
    #[command(subcommand)]
    pub command: Command,
}

/// Available subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Find generated files in projects, confirm, and delete them.
    Clean(CleanArgs),
    /// Create or extend a `.winnow` file from built-in templates.
    Init(InitArgs),
    /// List built-in templates.
    Templates,
}

/// Arguments of `winnower clean`.
#[derive(Debug, Args)]
pub struct CleanArgs {
    /// Directory to search for `.winnow` files.
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Only clean projects with no source changes for at least this long (`0` disables the filter).
    #[arg(long, value_name = "DURATION", default_value = "30d", value_parser = humantime::parse_duration)]
    pub older_than: Duration,
}

/// Arguments of `winnower init`.
#[derive(Debug, Args)]
pub struct InitArgs {
    /// Names of the templates to add.
    #[arg(required = true, value_name = "TEMPLATE")]
    pub templates: Vec<String>,

    /// Directory in which to write `.winnow`.
    #[arg(long, value_name = "PATH", default_value = ".")]
    pub dir: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    // The clap definitions are internally consistent.
    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    // `clean` without arguments uses the current directory and a 30-day filter.
    #[test]
    fn clean_defaults() {
        let cli = Cli::parse_from(["winnower", "clean"]);
        let Command::Clean(args) = cli.command else {
            panic!("expected the clean subcommand");
        };
        assert_eq!(args.path, PathBuf::from("."));
        assert_eq!(args.older_than, Duration::from_secs(30 * 24 * 60 * 60));
    }

    // `--older-than 0` disables the age filter.
    #[test]
    fn older_than_zero() {
        let cli = Cli::parse_from(["winnower", "clean", "--older-than", "0"]);
        let Command::Clean(args) = cli.command else {
            panic!("expected the clean subcommand");
        };
        assert_eq!(args.older_than, Duration::ZERO);
    }

    // An unparsable duration is a usage error.
    #[test]
    fn older_than_rejects_garbage() {
        assert!(Cli::try_parse_from(["winnower", "clean", "--older-than", "soon"]).is_err());
    }

    // `init` accepts several template names.
    #[test]
    fn init_accepts_several_templates() {
        let cli = Cli::parse_from(["winnower", "init", "cmake", "python"]);
        let Command::Init(args) = cli.command else {
            panic!("expected the init subcommand");
        };
        assert_eq!(args.templates, ["cmake", "python"]);
        assert_eq!(args.dir, PathBuf::from("."));
    }

    // `init` without template names is a usage error.
    #[test]
    fn init_requires_a_template() {
        assert!(Cli::try_parse_from(["winnower", "init"]).is_err());
    }
}
