//! The checks before a tool writes to a drive, and the question that the
//! person answers.
//!
//! The rules come from Burnout:
//!
//! - Nothing allows a write to the drive that the system starts from.
//! - A drive that the host does not mark as removable needs `--force`.
//! - The person types the name of the drive, and not one key. A question
//!   that takes one key is one that people learn to answer without reading.

use std::io::BufRead;

use anyhow::{Result, bail};

use super::Target;

/// Refuse the system drive always, and a fixed drive with no `force`.
pub fn check(target: &Target, force: bool) -> Result<()> {
    let Target::Drive(disk) = target else {
        return Ok(());
    };
    if disk.system {
        bail!(
            "{} holds the running system. Nothing allows a write to it",
            disk.id
        );
    }
    if disk.read_only {
        bail!(
            "{} is read only. A lock switch or the host stops each write",
            disk.id
        );
    }
    if !disk.removable && !force {
        bail!(
            "{} is not a removable drive. Add --force when you are sure that it is the right drive",
            disk.id
        );
    }
    Ok(())
}

/// The answer that the person types: the name of the drive, or the name of
/// the file.
pub fn answer(target: &Target) -> String {
    match target {
        Target::Drive(disk) => disk.id.clone(),
        Target::File(path) => path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
    }
}

/// Say what the write does, and wait for the person to type the answer. The
/// answer can come from a pipe, so a script can give it.
pub fn ask(target: &Target, effect: &str) -> Result<()> {
    let expected = answer(target);
    eprintln!("{effect}");
    eprintln!();
    eprintln!("    target  {}", target.describe());
    if let Target::Drive(disk) = target {
        eprintln!(
            "    size    {} ({} bytes)",
            crate::size::decimal(disk.size),
            crate::size::grouped(disk.size)
        );
        if let Some(bus) = &disk.bus {
            eprintln!(
                "    bus     {bus}{}",
                if disk.removable { ", removable" } else { "" }
            );
        }
    }
    eprintln!();
    eprint!("Type {expected} to go on: ");
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    // An answer from a pipe shows nothing, so the next text needs its own
    // line.
    if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        eprintln!();
    }
    if line.trim() != expected {
        bail!("the answer was not {expected}, so nothing changed");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disks::Disk;

    fn drive(system: bool, removable: bool) -> Target {
        Target::Drive(Disk {
            id: "sdb".to_owned(),
            system,
            removable,
            ..Disk::default()
        })
    }

    #[test]
    fn the_system_drive_is_refused_even_with_force() {
        assert!(check(&drive(true, true), true).is_err());
    }

    #[test]
    fn a_fixed_drive_needs_force() {
        assert!(check(&drive(false, false), false).is_err());
        assert!(check(&drive(false, false), true).is_ok());
        assert!(check(&drive(false, true), false).is_ok());
    }

    #[test]
    fn a_read_only_drive_is_refused() {
        let Target::Drive(mut disk) = drive(false, true) else {
            unreachable!()
        };
        disk.read_only = true;
        assert!(check(&Target::Drive(disk), true).is_err());
    }

    #[test]
    fn the_answer_is_the_name_of_the_drive_or_of_the_file() {
        assert_eq!(answer(&drive(false, true)), "sdb");
        let file = Target::File(std::path::PathBuf::from("/tmp/stick.img"));
        assert_eq!(answer(&file), "stick.img");
        assert!(check(&file, false).is_ok());
    }
}
