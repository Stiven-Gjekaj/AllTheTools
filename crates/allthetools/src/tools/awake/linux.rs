//! The host file of `att awake` on Linux: a lock of logind, which
//! systemd-inhibit holds while a child of it sleeps.

use std::process::{Child, Command, Stdio};
use std::time::Duration;

use anyhow::{Context, Result, bail};

use super::parse;

/// The systemd-inhibit process. The lock ends when the process ends.
pub struct Guard(Child);

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub fn hold(_display: bool) -> Result<Guard> {
    let child = Command::new("systemd-inhibit")
        .args(parse::inhibit_args())
        .args(["sleep", "infinity"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            bail!("this host has no `systemd-inhibit`. att awake needs systemd and logind")
        }
        Err(e) => return Err(e).context("could not start systemd-inhibit"),
    };

    // systemd-inhibit stops at once when logind refuses the lock. A short
    // wait catches that, so att awake does not say that it holds a lock that
    // it does not hold.
    std::thread::sleep(Duration::from_millis(300));
    if let Some(status) = child.try_wait()? {
        let mut message = String::new();
        if let Some(mut stderr) = child.stderr.take() {
            use std::io::Read;
            let _ = stderr.read_to_string(&mut message);
        }
        let first = message.lines().next().unwrap_or("").trim().to_owned();
        bail!("logind refused the lock ({status}): {first}");
    }
    Ok(Guard(child))
}

pub fn alive(pid: u32) -> bool {
    super::unix_alive(pid)
}
