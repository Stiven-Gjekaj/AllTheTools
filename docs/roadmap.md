<div align="center">
  <a href="../README.md"><b>AllTheTools</b></a>
</div>

# The roadmap

The thirty-five tools are built, and CI runs them on the three hosts on each
change.
This file says what comes next, in order.

## The way to 1.0

Version 1.0 is a promise: the name of each tool, its options, its exit codes
and the fields of its JSON stay the same for each 1.x release. A new tool, a
new option or a new field can come in 1.x. A change that breaks a script that
works needs 2.0. [docs/stability.md](stability.md) gives the rules.

| Version | What it adds |
| --- | --- |
| 0.3 | Eight tools: `df`, `du`, `hash`, `lscpu`, `ip`, `flushdns`, `say` and `uptime`. |
| 0.4 | The contract: [docs/json.md](json.md) names each field of each JSON output, tests hold each field, and tests give the parsers bytes at random. |
| 0.5 | `att lsusb` and `att lspci` on Windows read SetupAPI and not PowerShell, with the same output. |
| 0.9 | The release candidate: the stability rules, and the known limits of each tool. No new tool and no new option until 1.0. |
| 1.0 | The promise starts. |

1.0 does not wait for the tests on real hardware below. Each test that passes
goes into [docs/tools.md](tools.md). A fault that a test finds is a fix in a
1.0.x release.

## 1. Tests on real hardware

A virtual machine of CI has no USB stick, no monitor, no battery and no Wi-Fi
card. These tests wait for a person with the hardware, and
[docs/tools.md](tools.md) marks each one.

| Test | Hosts | What passes |
| --- | --- | --- |
| `att probe` on a real fake USB stick | each | the usable size agrees with `f3probe` on Linux, or with H2testw on Windows |
| `att probe` on a real honest USB stick | Linux, Windows | it passes, and the files on the stick are the same after |
| `att flash` of an installer to a real USB drive | each | the computer starts from the drive |
| `att surface` on a real USB drive | each | it reads each sector, and `--write` erases and checks it |
| `att eject` on a real USB drive | Linux, Windows | the drive is safe to remove, and the host says so |
| `att edid` with a real monitor | Linux, Windows | the values agree with `edid-decode` on Linux |
| `att ddc` with a real monitor | each | `get brightness` agrees with the menu of the monitor, and `set` changes it |
| `att wifi` with a real card | Linux, Windows | the list agrees with the networks of the host |
| `att battery` on a real laptop | Linux, Windows | the health agrees with `upower` or `powercfg /batteryreport` |
| `att sensors` on real hardware | Linux, Windows | the values agree with `sensors` or with the thermal zones of the host |
| `att wol` to a real computer | each | the computer wakes |
| `att clip` on a Linux desktop | Linux | a line copies and pastes on Wayland and on X11 |
| `att awake` on a Linux desktop | Linux | logind shows the lock in `systemd-inhibit --list` |
| `att notify` on a Linux desktop | Linux | the notification shows, with `notify-send` and with `gdbus` |
| `att trash` on a desktop | Linux, Windows | the file manager shows the file in the trash, and puts it back |

## 2. Closer to the API of the host

Some tools run a command of the host where an API exists, because the API is
very large for the job. Each one can move to the API, one tool at a time, with
no change to its output:

- `att lsblk` on Windows runs PowerShell, which takes about half a second to
  start. The storage IOCTLs give the same data in a few milliseconds.
  `att lsusb` and `att lspci` read SetupAPI since version 0.5.
- `att lsblk`, `att mount` and `att eject` on macOS run `diskutil` and
  `hdiutil`. Disk Arbitration is the framework under both.
- `att whoport` and `att whoholds` on macOS run `lsof`. `libproc` is the API
  under it.

## 3. More tools

A new tool needs a gap: one host has the tool, and the others do not.
[CONTRIBUTING.md](../CONTRIBUTING.md) says what makes a tool belong here.
Ideas that wait for an issue:

- `att smart`: the health of a drive. OnyxDiskInfo does this already, so it
  stays out of AllTheTools for now.
