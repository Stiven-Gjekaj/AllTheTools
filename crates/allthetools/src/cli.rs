//! The shape of the command line.

use clap::{Parser, Subcommand};

use crate::tools;

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
    /// Run a command, and stop it when its time runs out.
    Timeout(tools::timeout::Args),
    /// Wake a computer on the local network with a magic packet.
    Wol(tools::wol::Args),
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
    fn timeout_keeps_the_options_of_the_command() {
        let cli = Cli::try_parse_from(["att", "timeout", "-s", "KILL", "5", "ls", "-la"]).unwrap();
        let super::Command::Timeout(args) = cli.command else {
            panic!("not timeout");
        };
        assert_eq!(args.signal, "KILL");
        assert_eq!(args.duration, "5");
        assert_eq!(args.command, ["ls", "-la"]);
    }

    #[test]
    fn a_command_is_required() {
        assert!(Cli::try_parse_from(["att"]).is_err());
    }
}
