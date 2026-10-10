<div align="center">

![AllTheTools](assets/wordmark.svg)

### The tools that one system has and the other two do not

_One binary. The same command on Windows, on macOS, and on Linux._

![Rust](https://img.shields.io/badge/rust-14b8a6?style=for-the-badge&logo=rust&logoColor=030712)
![Windows](https://img.shields.io/badge/windows-38bdf8?style=for-the-badge&logo=windows&logoColor=030712)
![macOS](https://img.shields.io/badge/macos-818cf8?style=for-the-badge&logo=apple&logoColor=030712)
![Linux](https://img.shields.io/badge/linux-a78bfa?style=for-the-badge&logo=linux&logoColor=030712)
[![MIT licence](https://img.shields.io/badge/mit_licence-c4b5fd?style=for-the-badge&logoColor=030712)](LICENSE)
[![Release](https://img.shields.io/github/v/release/Stiven-Gjekaj/AllTheTools?style=for-the-badge&label=release&labelColor=ede9fe&color=ede9fe)](https://github.com/Stiven-Gjekaj/AllTheTools/releases/latest)

<p align="center">
  <a href="#overview"><b>Overview</b></a> |
  <a href="#install"><b>Install</b></a> |
  <a href="#the-twenty-tools"><b>Tools</b></a> |
  <a href="#the-familiar-names"><b>Names</b></a> |
  <a href="#for-a-script"><b>Scripts</b></a> |
  <a href="docs/tools.md"><b>Each tool</b></a>
</p>

</div>

---

> [!NOTE]
> **Twenty tools are built.**
> CI runs each one on Windows, on macOS and on Linux, on each change.
> The tools that write to a drive are tested against fake drives in memory,
> and on image files, loop devices and VHDs in CI.
> [docs/tools.md](docs/tools.md) says how each tool works on each host, and
> which tools still wait for a test on real hardware, such as a real fake
> USB stick or a real external monitor.

---

## Overview

**AllTheTools** is one command, `att`, with twenty small tools in it.

Each tool closes a gap. Linux has `lsusb`, and macOS and Windows do not.
Linux has `f3probe` to find a fake flash drive, and the other two do not.
macOS has `caffeinate` and `networkQuality`, and Linux and Windows do not.
Windows has no `timeout` that stops a command, no `ldd` and no clipboard
command that pastes.

So the answer to a simple question depends on which computer you sit at, and
each answer has its own flags to learn. `att` gives one answer: the same
command, the same options and the same output on all three systems.

<table>
<tr>
<td width="50%" valign="top">

### What it aims to do

- **Close a gap**, and not copy a tool that each system has already.
- **Give the same output** on each host, as a table for a person or as JSON
  for a script.
- **Need nothing else.** No tool asks you to install a second tool.
- **Name a drive one way.** `att lsblk` shows the names, and each disk tool
  takes them.

</td>
<td width="50%" valign="top">

### How it aims to behave

- It uses the API of the host first, and a command that ships with the host
  only when no API exists.
- It never parses text for a person, because that text changes with the
  language of the host.
- It refuses to write to the disk that your system starts from.
- Each error says what went wrong, and then what to do about it.

</td>
</tr>
</table>

---

## Install

On macOS or on Linux, with Homebrew:

```bash
brew install stiven-gjekaj/tap/allthetools
```

On Windows, with Scoop:

```powershell
scoop bucket add stiven-gjekaj https://github.com/Stiven-Gjekaj/scoop-bucket
scoop install stiven-gjekaj/allthetools
```

On Windows, with winget, when Microsoft accepts the manifest:

```powershell
winget install Stiven-Gjekaj.AllTheTools
```

On Arch Linux, from the AUR:

```bash
yay -S allthetools-bin
```

On Debian, Ubuntu, Fedora or openSUSE, with the package of the
[latest release](https://github.com/Stiven-Gjekaj/AllTheTools/releases/latest):

```bash
sudo apt install ./allthetools_0.1.0-1_amd64.deb
```

```bash
sudo dnf install ./allthetools-0.1.0-1.x86_64.rpm
```

Or download the binary for your host from the
[latest release](https://github.com/Stiven-Gjekaj/AllTheTools/releases/latest).
The Linux binary carries its own C library, and the Windows binary carries
the Microsoft C runtime, so neither needs anything installed beside it.
[docs/releasing.md](docs/releasing.md) says how to check a download against
`SHA256SUMS` and against its build provenance.

### The warning of a download

A browser marks each file that it downloads, and macOS and Windows then warn
before they start it. The warning is about how the file arrived, and not about
what the file holds. Check the file first, and then take the mark off. On
macOS:

```bash
xattr -d com.apple.quarantine att-0.1.0-aarch64-apple-darwin
```

On Windows, SmartScreen can show "Windows protected your PC". Select
**More info**, and then **Run anyway**. After the first start,
`att unquarantine` takes the mark off other files on each host.

---

## The twenty tools

| Tool | What it does | The host that has it | The hosts that did not |
| :-- | :-- | :-- | :-- |
| `att probe` | Finds the real size of a fake flash drive | Linux: f3probe | macOS, Windows |
| `att surface` | Reads each sector of a drive, and names the bad ones | Linux: badblocks | macOS, Windows |
| `att lsblk` | Lists the drives and their partitions as a tree | Linux: lsblk | macOS, Windows |
| `att eject` | Unmounts a drive and makes it safe to remove | macOS, Linux | Windows |
| `att mount` | Attaches an ISO, IMG, DMG or VHD and mounts it | each, with other commands | one command |
| `att whoholds` | Shows the process that holds a file or a drive | macOS, Linux: lsof, fuser | Windows |
| `att lsusb` | Lists the USB devices | Linux: lsusb | macOS, Windows |
| `att lspci` | Lists the PCI devices | Linux: lspci | macOS, Windows |
| `att sensors` | Shows the temperatures and the fans | Linux: sensors | macOS, Windows |
| `att battery` | Shows the charge, the health and the cycles | each, with other tools | one report |
| `att ddc` | Sets the brightness and the input of a monitor | Linux: ddcutil | macOS, Windows |
| `att netquality` | Measures the speed and the responsiveness | macOS: networkQuality | Linux, Windows |
| `att wifi` | Lists the Wi-Fi networks near the computer | Linux: nmcli | macOS since 14.4, Windows |
| `att whoport` | Shows the process of a port, and stops it | each, with other commands | one table |
| `att wol` | Wakes a computer on the local network | none | macOS, Linux, Windows |
| `att timeout` | Stops a command when its time runs out | Linux: timeout | macOS, Windows |
| `att awake` | Keeps the system awake | macOS: caffeinate | Linux, Windows |
| `att clip` | Copies to the clipboard, and pastes | macOS: pbcopy, pbpaste | Linux, Windows |
| `att unquarantine` | Removes the download mark of a file | each, with other commands | one command |
| `att ldd` | Lists the libraries of a binary of any host | Linux: ldd | macOS, Windows |

`att <tool> --help` gives the options of each tool.
[docs/tools.md](docs/tools.md) says how each one works on each host.

### Drives

```bash
att lsblk
```

```
NAME         SIZE    BUS           FSTYPE  LABEL         MOUNTPOINT  MODEL
disk0        465.9G  Apple Fabric                                    APPLE SSD AP0512Z (system)
├─disk0s1    550.0M                Apple_APFS_ISC
└─disk0s2    460.4G                apfs container
disk4        58.6G   USB                                             SanDisk Ultra (removable)
└─disk4s1    58.6G                 msdos   STICK         /Volumes/STICK
```

Each disk tool takes a name from that list, a device path, or a mount point:
`disk4`, `/dev/sdb`, `2` for PhysicalDrive2, or `E:`.

```bash
att probe disk4            # is this USB stick fake? It writes back what it tests.
att surface disk4          # read each sector, and name the bad ones
att surface --write disk4  # write and read back each sector. This erases it.
att whoholds /Volumes/STICK
att eject disk4
att mount ubuntu-24.04.iso
```

`att probe` and `att surface --write` refuse the disk that your system starts
from. Nothing allows that write. A drive that the host does not mark as
removable needs `--force`. Each one asks you to type the name of the drive.

### Hardware

```bash
att lsusb
att lspci
att sensors
att battery
att ddc set brightness 70
att ddc set input hdmi2
```

### Network

```bash
att netquality
att wifi
att whoport 3000
att whoport 3000 --kill
att wol aa:bb:cc:dd:ee:ff
```

### Shell

```bash
att timeout 10s ./slow-script.sh
att awake --display -- cargo build --release
echo "some text" | att clip
att clip paste
att unquarantine -r ~/Downloads/SomeApp.app
att ldd ./my-program
```

---

## The familiar names

The packages install one command, `att`, and no other name. So a package never
takes a name that your system or another package owns.

`att link` adds the familiar names on request, beside `att`:

```bash
att link
```

```
badblocks runs att surface.
f3probe runs att probe.
lsusb runs att lsusb.
Skipped pbcopy: the host has a program of that name. Add --force to make it anyway.
...
```

Then `lsusb`, `lspci`, `lsblk`, `f3probe`, `badblocks`, `sensors`, `ldd`,
`timeout`, `caffeinate`, `pbcopy` and the others run the tools of `att`. A
name that the host has already is skipped. `att link --remove` removes each
name that runs `att`, and nothing else.

---

## For a script

Each tool that lists or measures takes `--json`, and it prints JSON on the
output stream. The text for a person goes to the error stream.

```bash
att lsusb --json
att lsblk --json
att probe --json disk4
```

The exit codes:

| Code | Meaning |
| :-- | :-- |
| 0 | The tool did its work, and it found no problem. |
| 1 | The tool failed, and the error stream says why. |
| 2 | The tool did its work, and it found a problem: `surface` found a bad sector, or `probe` found a fake drive. |
| 124 | `timeout` stopped the command when its time ran out, as GNU `timeout` does. |

---

## Project documents

| Document | What it holds |
| :-- | :-- |
| [docs/tools.md](docs/tools.md) | How each tool works on each host, and how it was tested |
| [docs/roadmap.md](docs/roadmap.md) | What comes next, and the tests that wait for real hardware |
| [docs/releasing.md](docs/releasing.md) | How a release is cut for each package manager, and how to check a download |
| [CHANGELOG.md](CHANGELOG.md) | Each release |
| [CONTRIBUTING.md](CONTRIBUTING.md) | How to take part, and the rule that no test opens a real device |
| [AGENTS.md](AGENTS.md) | The rules for anybody who changes this repository, human or agent |
| [SECURITY.md](SECURITY.md) | The threat model, and how to report a vulnerability privately |
| [TERMS.md](TERMS.md) | What the tools do to your data, and what you agree to |
| [SUPPORT.md](SUPPORT.md) | Where to ask a question |
| [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) | Applies to everybody who takes part |

---

## Licence

MIT. See [LICENSE](LICENSE) and [TERMS.md](TERMS.md).

<div align="center">
<sub>Two tools of AllTheTools write to drives. Read TERMS.md before you run them.</sub>
</div>
