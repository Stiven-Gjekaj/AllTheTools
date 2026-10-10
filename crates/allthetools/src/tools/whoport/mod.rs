//! `att whoport`: show which process uses a port, and stop it when asked.
//!
//! Each host has its own command and its own output: `lsof -i` on macOS,
//! `ss -p` on Linux, and `netstat -ano` on Windows, which gives a number and
//! no name. This tool gives the same table on each host.

use std::io::{BufRead, IsTerminal, Write};

use anyhow::{Result, bail};

pub mod parse;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as host;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as host;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as host;

use parse::{Proto, Socket};

/// Show which process listens on a port, or on each port.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// The ports. With no port, show each port that a process listens on.
    pub ports: Vec<u16>,

    /// Show connected sockets too, and not only the ones that listen.
    #[arg(short, long)]
    pub all: bool,

    /// Show TCP only.
    #[arg(long, conflicts_with = "udp")]
    pub tcp: bool,

    /// Show UDP only.
    #[arg(long)]
    pub udp: bool,

    /// Stop each process that listens on the ports. att asks first.
    #[arg(long, requires = "ports")]
    pub kill: bool,

    /// Stop the processes and do not ask. Use this in a script.
    #[arg(long, requires = "kill")]
    pub yes: bool,

    /// Print JSON for a script, and not a table for a person.
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) -> Result<i32> {
    let mut sockets: Vec<Socket> = host::sockets()?
        .into_iter()
        .filter(|s| args.all || s.listens())
        .filter(|s| args.ports.is_empty() || args.ports.contains(&s.port))
        .filter(|s| !args.tcp || s.proto == Proto::Tcp)
        .filter(|s| !args.udp || s.proto == Proto::Udp)
        .collect();
    sockets.sort_by(|a, b| {
        (a.port, a.proto as u8, &a.address).cmp(&(b.port, b.proto as u8, &b.address))
    });
    sockets.dedup();

    if args.json {
        println!("{}", serde_json::to_string(&sockets)?);
    } else if sockets.is_empty() {
        match args.ports.as_slice() {
            [] => println!("No process that this user can see listens on a port."),
            ports => println!(
                "No process that this user can see uses port {}.",
                list(ports)
            ),
        }
    } else {
        print(&sockets, args.all);
    }

    let hidden = sockets.iter().filter(|s| s.pid.is_none()).count();
    if hidden > 0 && !args.json {
        eprintln!(
            "att whoport: {hidden} sockets belong to processes of another user. \
             Run as root or as Administrator to see them."
        );
    }

    if args.kill {
        return kill(&sockets, args.yes);
    }
    Ok(if !args.ports.is_empty() && sockets.is_empty() {
        1
    } else {
        0
    })
}

fn list(ports: &[u16]) -> String {
    ports
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn print(sockets: &[Socket], all: bool) {
    let dash = || "-".to_owned();
    let rows: Vec<Vec<String>> = sockets
        .iter()
        .map(|s| {
            let mut row = vec![
                match s.proto {
                    Proto::Tcp => "tcp".to_owned(),
                    Proto::Udp => "udp".to_owned(),
                },
                s.local(),
            ];
            if all {
                row.push(s.remote.clone().unwrap_or_else(dash));
                row.push(s.state.clone().unwrap_or_else(dash));
            }
            row.push(s.pid.map_or_else(dash, |p| p.to_string()));
            row.push(s.process.clone().unwrap_or_else(dash));
            row
        })
        .collect();
    if all {
        crate::table::print(
            &["PROTO", "LOCAL", "REMOTE", "STATE", "PID", "PROCESS"],
            &rows,
        );
    } else {
        crate::table::print(&["PROTO", "LOCAL", "PID", "PROCESS"], &rows);
    }
}

/// Stop each process of the sockets one time, after the person says yes.
fn kill(sockets: &[Socket], yes: bool) -> Result<i32> {
    let mut targets: Vec<(u32, String, String)> = Vec::new();
    for socket in sockets {
        if let Some(pid) = socket.pid
            && !targets.iter().any(|(p, _, _)| *p == pid)
        {
            let name = socket.process.clone().unwrap_or_else(|| "?".to_owned());
            targets.push((pid, name, socket.local()));
        }
    }
    if targets.is_empty() {
        bail!("no process to stop that this user can see");
    }
    if !yes && !std::io::stdin().is_terminal() {
        bail!("the input is not a terminal, so att cannot ask. Add --yes to stop the processes");
    }

    let mut failed = false;
    for (pid, name, local) in targets {
        if !yes {
            eprint!("Stop process {pid} ({name}), which uses {local}? Type yes to stop it: ");
            std::io::stderr().flush()?;
            let mut answer = String::new();
            std::io::stdin().lock().read_line(&mut answer)?;
            if answer.trim() != "yes" {
                eprintln!("Process {pid} still runs.");
                continue;
            }
        }
        match host::stop(pid) {
            Ok(()) => println!("Stopped process {pid} ({name})."),
            Err(e) => {
                eprintln!("att whoport: could not stop process {pid} ({name}): {e}");
                failed = true;
            }
        }
    }
    Ok(if failed { 1 } else { 0 })
}

/// Send SIGTERM, which lets the process close its files first.
#[cfg(unix)]
fn unix_stop(pid: u32) -> std::io::Result<()> {
    let pid = libc::pid_t::try_from(pid)
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    // SAFETY: kill has no precondition.
    #[allow(unsafe_code)]
    let result = unsafe { libc::kill(pid, libc::SIGTERM) };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}
