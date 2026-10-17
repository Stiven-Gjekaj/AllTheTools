<div align="center">
  <a href="../README.md"><b>AllTheTools</b></a>
</div>

# Known limits

Each limit here is known, and a release does not count it as a fault. A
limit can go away in a minor release. [docs/tools.md](tools.md) says how each
tool works on each host.

## The tests on real hardware

CI runs each tool on a virtual machine of Linux, macOS and Windows. A virtual
machine has no USB stick, no monitor that answers DDC/CI, no battery and no
Wi-Fi card. The tools that need such hardware were tested on real hardware on
macOS only, and [docs/roadmap.md](roadmap.md) lists the tests that wait for
Linux and Windows hardware. A fault that one of those tests finds is a fix in
a patch release.

## Drives

- **`att probe`** finds a fake drive that drops each write past its real
  size, and a fake that wraps at a power of two. It can miss a fake that
  wraps at another size, and a fake with a large cache, as f3probe can.
  `att surface --write` finds both, and erases the drive.
- **`att flash`** writes an image as it is. It does not extract an archive
  such as `.xz` or `.zip`, and it refuses one.
- **`att du`** on Windows counts the length of each file, and not the space on
  the disk. It counts a file with more than one hard link one time for each
  link.
- **`att mount`** attaches ISO and raw images on Linux, ISO, raw and DMG on
  macOS, and ISO, VHD and VHDX on Windows. A host refuses the other kinds.

## Hardware

- **`att lsusb`** on Windows gives no bus, no address and no speed, because
  the PnP manager does not give them.
- **`att sensors`** on Windows shows the thermal zones of ACPI only. Windows
  has no public API for the temperature of each core or for the fans.
- **`att ddc`** needs a monitor with DDC/CI on in its menu, and a cable or an
  adapter that carries DDC/CI. The USB-C Digital AV Adapter of Apple does not.
  An Intel Mac has no IOAVService, and the built-in display of a laptop has no
  DDC/CI.
- **`att edid`** reads the base block of EDID 1.3 and 1.4, and only names the
  extension blocks.
- **`att free`** on Windows gives no count of the memory that holds nothing.
- **`att lscpu`** on Apple silicon gives no speed and no features, because
  macOS does not give them.

## Network

- **`att wifi`** shows the names of the networks only when the terminal has
  the permission for the location on macOS and on Windows 11.
- **`att flushdns`** needs root on Linux and macOS. A Linux host with no
  systemd-resolved and no nscd keeps no DNS cache.
- **`att netquality`** measures against the servers of Cloudflare only.

## Shell

- **`att notify`** on macOS shows the notification as Script Editor, and
  System Settings can turn those off.
- **`att say`** with spd-say cannot write a file.
- **`att trash`** on a Windows volume with no Recycle Bin, such as a network
  share, cannot keep the file, and Windows asks before it deletes the file.
- **`att uptime`** on Windows gives no load average.
- **`att timeout`** on Windows has no signals, so `--signal` gives only the
  exit code there.
- **`att hexdump`** takes the main options of `xxd`, and not `-e`, `-o`, `-d`
  or `-a`.
