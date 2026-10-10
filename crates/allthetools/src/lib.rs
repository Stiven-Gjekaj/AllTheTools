//! The command line of AllTheTools.
//!
//! The binary is one line that calls [`run`]. Everything else is here, so a
//! test reaches each part of it.

use clap::Parser;

pub mod cli;
pub mod host;
pub mod size;
pub mod table;
pub mod time;
pub mod tools;

use cli::{Cli, Command};

/// Run the command that the person typed, and give the exit code.
///
/// An error prints one line on the error stream and gives exit code 1.
pub fn run() -> i32 {
    let cli = Cli::parse();
    let result: anyhow::Result<i32> = match cli.command {
        Command::Awake(args) => tools::awake::run(args),
        Command::Clip(args) => tools::clip::run(args),
        Command::Ldd(args) => tools::ldd::run(args),
        Command::Lspci(args) => tools::lspci::run(args),
        Command::Lsusb(args) => tools::lsusb::run(args),
        Command::Timeout(args) => tools::timeout::run(args),
        Command::Unquarantine(args) => tools::unquarantine::run(args),
        Command::Wol(args) => tools::wol::run(args),
    };
    match result {
        Ok(code) => code,
        Err(e) => {
            eprintln!("att: {e:#}");
            1
        }
    }
}
