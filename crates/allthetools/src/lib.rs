//! The command line of AllTheTools.
//!
//! The binary is one line that calls [`run`]. Everything else is here, so a
//! test reaches each part of it.

use clap::Parser;

pub mod alias;
pub mod cli;
pub mod disks;
pub mod host;
pub mod progress;
pub mod rawdev;
pub mod size;
#[cfg(target_os = "macos")]
pub mod sysctl;
pub mod table;
pub mod time;
pub mod tools;

use cli::{Cli, Command};

/// Run the command that the person typed, and give the exit code.
///
/// A start under a familiar name, such as lsusb, runs that tool. An error
/// prints one line on the error stream and gives exit code 1.
pub fn run() -> i32 {
    let cli = Cli::parse_from(alias::expand(std::env::args_os().collect()));
    let result: anyhow::Result<i32> = match cli.command {
        Command::Awake(args) => tools::awake::run(args),
        Command::Battery(args) => tools::battery::run(args),
        Command::Clip(args) => tools::clip::run(args),
        Command::Ddc(args) => tools::ddc::run(args),
        Command::Df(args) => tools::df::run(args),
        Command::Du(args) => tools::du::run(args),
        Command::Edid(args) => tools::edid::run(args),
        Command::Eject(args) => tools::eject::run(args),
        Command::Flash(args) => tools::flash::run(args),
        Command::Flushdns(args) => tools::flushdns::run(args),
        Command::Free(args) => tools::free::run(args),
        Command::Hash(args) => tools::hash::run(args),
        Command::Hexdump(args) => tools::hexdump::run(args),
        Command::Ip(args) => tools::ip::run(args),
        Command::Ldd(args) => tools::ldd::run(args),
        Command::Link(args) => tools::link::run(args),
        Command::Lsblk(args) => tools::lsblk::run(args),
        Command::Lscpu(args) => tools::lscpu::run(args),
        Command::Lspci(args) => tools::lspci::run(args),
        Command::Lsusb(args) => tools::lsusb::run(args),
        Command::Mount(args) => tools::mount::run(args),
        Command::Netquality(args) => tools::netquality::run(args),
        Command::Notify(args) => tools::notify::run(args),
        Command::Probe(args) => tools::probe::run(args),
        Command::Sensors(args) => tools::sensors::run(args),
        Command::Surface(args) => tools::surface::run(args),
        Command::Timeout(args) => tools::timeout::run(args),
        Command::Trash(args) => tools::trash::run(args),
        Command::Unquarantine(args) => tools::unquarantine::run(args),
        Command::Watch(args) => tools::watch::run(args),
        Command::Whoholds(args) => tools::whoholds::run(args),
        Command::Whoport(args) => tools::whoport::run(args),
        Command::Wifi(args) => tools::wifi::run(args),
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
