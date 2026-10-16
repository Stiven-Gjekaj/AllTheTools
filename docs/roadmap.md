<div align="center">
  <a href="../README.md"><b>AllTheTools</b></a>
</div>

# The roadmap

The twenty tools are built, and CI runs them on the three hosts on each change.
This file says what comes next, in order.

## 1. Tests on real hardware

A virtual machine of CI has no USB stick, no monitor, no battery and no Wi-Fi
card. These tests wait for a person with the hardware, and
[docs/tools.md](tools.md) marks each one.

| Test | Hosts | What passes |
| --- | --- | --- |
| `att probe` on a real fake USB stick | each | the usable size agrees with `f3probe` on Linux, or with H2testw on Windows |
| `att probe` on a real honest USB stick | Linux, Windows | it passes, and the files on the stick are the same after |
| `att surface` on a real USB drive | each | it reads each sector, and `--write` erases and checks it |
| `att eject` on a real USB drive | Linux, Windows | the drive is safe to remove, and the host says so |
| `att ddc` with a real monitor | each | `get brightness` agrees with the menu of the monitor, and `set` changes it |
| `att wifi` with a real card | Linux, Windows | the list agrees with the networks of the host |
| `att battery` on a real laptop | Linux, Windows | the health agrees with `upower` or `powercfg /batteryreport` |
| `att sensors` on real hardware | Linux, Windows | the values agree with `sensors` or with the thermal zones of the host |
| `att wol` to a real computer | each | the computer wakes |
| `att clip` on a Linux desktop | Linux | a line copies and pastes on Wayland and on X11 |
| `att awake` on a Linux desktop | Linux | logind shows the lock in `systemd-inhibit --list` |

## 2. Closer to the API of the host

Some tools run a command of the host where an API exists, because the API is
very large for the job. Each one can move to the API, one tool at a time, with
no change to its output:

- `att lsusb`, `att lspci` and `att lsblk` on Windows run PowerShell, which
  takes about half a second to start. SetupAPI and the storage IOCTLs give the
  same data in a few milliseconds.
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
- `att hexdump` with the options of `xxd`, which Windows does not have.
- `att trash`: move a file to the recycle bin, which Linux has as `gio trash`
  and a new macOS has as `trash`, and which Windows has no command for.
