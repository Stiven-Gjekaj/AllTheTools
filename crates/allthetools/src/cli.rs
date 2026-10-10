//! The shape of the command line.

use clap::{Parser, Subcommand};

/// The same small tools on Windows, on macOS and on Linux.
#[derive(Debug, Parser)]
#[command(name = "att", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

/// One tool of AllTheTools.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Print the version of AllTheTools.
    Version,
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::{CommandFactory, Parser};

    // clap checks the shape of the command only when it runs. This test runs
    // the same checks, so a bad option fails here and not in the hands of a
    // person.
    #[test]
    fn the_command_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn the_version_flag_is_accepted() {
        let error = Cli::try_parse_from(["att", "--version"]).unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::DisplayVersion);
    }

    #[test]
    fn a_command_is_required() {
        assert!(Cli::try_parse_from(["att"]).is_err());
    }
}
