<div align="center">
  <a href="../README.md"><b>AllTheTools</b></a>
</div>

# Each tool on each host

This file says how each tool gets its answer on each host, what privilege it
needs, what it cannot do, and on what it was tested.

Two kinds of test exist:

- **CI** runs each tool on a virtual machine of GitHub Actions, on each
  change: Ubuntu on x86_64, macOS on Apple silicon, and Windows on x86_64.
  `scripts/smoke.sh` is the list. A virtual machine has no USB device, no
  battery, no monitor and no Wi-Fi card, so for those tools CI proves that
  the code runs and says "none".
- **Hardware** is a run on a real computer with the real device, and a
  comparison with a tool of the host. The table under each tool says where
  that run happened.

A unit test never opens a device. Each parser has tests with output that a
real host gave, and the three tools that write to a drive have tests against
fake drives in memory.

---

## The rule for the host

Each tool asks the API of the host first. When no public API exists, or the
API is very large for the job, the tool runs a command that ships with the
host and reads its machine output: a property list of `ioreg` or `diskutil`
on macOS, and JSON of PowerShell on Windows. No tool parses text for a person.
No tool runs a command that the host does not ship, except on Linux, where
the error names the package to install.

---

## Drives

### `att lsblk`

| Host | Source | Privilege |
| :-- | :-- | :-- |
| Linux | `/sys/block`, `/run/udev/data`, `/proc/self/mountinfo` | none |
| macOS | `diskutil list -plist` and `diskutil info -plist` | none |
| Windows | `Get-Disk`, `Get-Partition` and `Get-Volume` in JSON | none |

The system drive is the drive that holds `/`, down through a device mapper on
Linux and through the APFS container on macOS. On Windows it is the drive
with `IsBoot` or `IsSystem`.

Tested: CI on the three hosts, with a check that the list marks a system
drive. Hardware: macOS on a MacBook with Apple silicon, with a disk image and
with a USB hub.

### `att eject`

| Host | How | Privilege |
| :-- | :-- | :-- |
| Linux | `udisksctl unmount` and `power-off`; with no udisks, `umount2` and the `delete` file of sysfs | none with udisks, root without |
| macOS | `diskutil eject` | none |
| Windows | lock and dismount of each volume, then `CM_Request_Device_EjectW` on the device | none |

It refuses the system drive. On Windows, a program that holds a file makes
Windows refuse, and `att whoholds E:` names that program.

Tested: CI checks the refusal of the system drive on the three hosts, and
ejects a disk image on macOS. Hardware: macOS with a disk image, and with a
126 GB USB stick. **Not yet tested on a real USB drive on Linux and on
Windows.**

### `att mount`

| Host | Kinds | How |
| :-- | :-- | :-- |
| Linux | ISO, raw IMG | `udisksctl loop-setup` and `mount`; with no udisks, root runs `losetup` and mounts under `/mnt/att` |
| macOS | ISO, raw IMG, DMG | `hdiutil attach -plist` |
| Windows | ISO, VHD, VHDX | `Mount-DiskImage` |

A host refuses the kinds that it cannot attach, and the error says which kinds
it takes.

Tested: CI attaches an ISO on macOS, an ext4 image on Linux through udisks
as root, and a VHD on Windows, and detaches each one. Hardware: macOS with a
DMG, an ISO and a raw image. **The path of Linux with no udisks, through
`losetup`, is not yet tested.**

### `att whoholds`

| Host | Source | Privilege |
| :-- | :-- | :-- |
| Linux | the links in `/proc/<pid>/fd`, `cwd`, `root`, `exe`, and `/proc/<pid>/maps` | root to see the processes of other users |
| macOS | `lsof -F pcfn`: a mount point as a volume, a folder with `+D` | the same |
| Windows | the Restart Manager, with each file under the path, up to 20,000 | none |

The Restart Manager names the process and not the file, so the PATH column is
empty on Windows.

Tested: CI finds a process that holds a file on the three hosts. Hardware:
macOS with a folder, a file and a mounted volume.

### `att surface`

Reads each sector in chunks of 4 MiB. A chunk that fails is read again sector
by sector, so the report names each bad sector. A chunk that takes longer than
`--slow` (1 second) is slow. `--write` writes a pattern to each sector, and then
reads each sector back and compares it, as `badblocks -w` does. That erases the
drive.

| Host | Device | Privilege |
| :-- | :-- | :-- |
| Linux | `/dev/sdX` with `O_DIRECT`, and `O_EXCL` for a write | root |
| macOS | `/dev/rdiskN`, the raw node, which has no cache | root, or the owner of a disk image |
| Windows | `\\.\PhysicalDriveN` with no buffering; for a write, each volume locked and the drive offline | Administrator |

It also takes an image file, which needs no privilege.

Tested: unit tests against drives in memory with bad sectors, a drive that
drops writes, and a drive that wraps. CI reads and writes an image file on the
three hosts, a loop device as root on Linux, and an attached VHD on Windows.
Hardware: macOS with an image file and with the raw node of a disk image.
**Not yet tested on a real USB drive or a real drive with bad sectors.**

### `att probe`

The test of f3probe. It keeps a copy of about 300 sample blocks, writes a
marker to each one from the highest to the lowest, reads each one back, and
then searches for the edge between the good blocks and the bad ones. At the
end it writes each copy back, from the highest block to the lowest.

It finds a fake that drops each write past its real size, and a fake that
wraps at a power of two, which is how most fakes fail. It can miss a fake that
wraps at another size, and a fake with a large cache. f3probe has the same two
limits. `att surface --write` checks each sector and has neither, and it takes
much longer and erases the drive.

The privilege and the devices are those of `att surface`.

Tested: unit tests against fake drives in memory: drives that drop, drives
that wrap at a power of two, and drives that wrap at another size. For each one
that the test finds, it gives the real size to the block, and an honest drive
keeps each byte. CI probes an image of random bytes on the three hosts, and a
block device: a loop device on Linux, the raw node of an image on macOS, and a
VHD on Windows. The SHA-256 of the image is the same before and after.
Hardware: macOS with an image file and with the raw node of a disk image, with
the same check of the hash. Hardware: macOS with an honest 126 GB USB stick.
It found the full size, it wrote back 504 blocks, and `diskutil verifyVolume`
found the file system correct after. **Not yet tested on a real fake USB
stick.**

### `att flash`

It writes an image, such as the ISO of a Linux installer, to the start of a
drive, and then reads the drive back and compares each byte with the image.
The privilege and the devices are those of `att surface`, and it has the same
guard: it refuses the system drive, a fixed drive needs `--force`, and the
person types the name of the drive.

It refuses an image that is on a volume of the target, and an archive such as
`.gz`, `.xz`, `.zst`, `.bz2` or `.zip`, which it finds by the first bytes of
the file. Exit code 2 says that a byte read back wrong. A fake drive does
that, and `att probe` then finds its real size.

Tested: unit tests against drives in memory: an honest drive, a drive that is
too small, and a fake drive that wraps. CI writes an image to a file on the
three hosts, and to a block device: a loop device on Linux, the raw node of a
disk image on macOS, and a VHD on Windows. Hardware: macOS with a file and with
the raw node of a disk image. **Not yet tested on a real USB drive.**

---

## Hardware

### `att lsusb`

| Host | Source |
| :-- | :-- |
| Linux | `/sys/bus/usb/devices` |
| macOS | `ioreg -p IOUSB -l -r -c IOUSBHostDevice -a` |
| Windows | `Win32_PnPEntity` in JSON |

Each name is the name that the device reports. Linux `lsusb` uses the list
`usb.ids`, so its names can differ. Windows gives no bus and no address.

Tested: CI on the three hosts. Hardware: macOS with a USB hub, a card reader,
a network adapter and a USB stick, against `system_profiler`.

### `att lspci`

| Host | Source |
| :-- | :-- |
| Linux | `/sys/bus/pci/devices`, and the names in `pci.ids` when the host has it |
| macOS | `ioreg -r -c IOPCIDevice -a` |
| Windows | `Win32_PnPEntity` in JSON, with the class from the compatible IDs |

Tested: CI on the three hosts. Hardware: macOS on Apple silicon.

### `att sensors`

| Host | Source | Privilege |
| :-- | :-- | :-- |
| Linux | `/sys/class/hwmon`, as lm-sensors reads it | none |
| macOS | the temperature sensors of the HID event system on Apple silicon; the fans, and the temperatures of an Intel Mac, from the SMC | none |
| Windows | the thermal zones of ACPI, from the performance counters | none |

Windows has no public API for the temperature of each core or for the fans.
A tool that shows them loads a kernel driver of its own. `att sensors` shows
what Windows gives a standard user.

Tested: CI on the three hosts, which have no sensor. Hardware: macOS on a
MacBook Air with Apple M5: 40 sensors of the HID system, and 129 temperature
keys of the SMC. **Not yet tested on Linux and Windows hardware.**

### `att battery`

| Host | Source |
| :-- | :-- |
| Linux | `/sys/class/power_supply` |
| macOS | `ioreg -r -c AppleSmartBattery -a` |
| Windows | `BatteryStaticData`, `BatteryFullChargedCapacity`, `BatteryCycleCount` and `BatteryStatus` of WMI |

The health is the full capacity now over the capacity when new. macOS shows a
smoothed figure in System Settings, and `att battery` shows that figure beside
its own.

Tested: CI on the three hosts, which have no battery. Hardware: macOS, against
`system_profiler SPPowerDataType`. **Not yet tested on Linux and Windows
laptops.**

### `att ddc`

| Host | How | Need |
| :-- | :-- | :-- |
| Linux | the I2C bus of each connector, through `/dev/i2c-N` | the module `i2c-dev`, and the group of `/dev/i2c-N` |
| macOS | IOAVService on Apple silicon | an external display |
| Windows | the Monitor Configuration API of dxva2 | none |

An Intel Mac has no IOAVService. The built-in display of a laptop has no
DDC/CI. A monitor can have DDC/CI off in its menu. An adapter or a dock can
carry the picture and not DDC/CI. The USB-C Digital AV Adapter of Apple is
one, and m1ddc fails through it in the same way.

Tested: unit tests check the bytes of each message against the bytes that
ddcutil sends. CI lists the monitors on the three hosts, and finds none.
Hardware: macOS lists a monitor through the USB-C Digital AV Adapter, and the
adapter refuses each request. **Not yet tested with a real monitor that
answers, on any host.**

### `att edid`

| Host | Source |
| :-- | :-- |
| Linux | the `edid` file of each connector in `/sys/class/drm` |
| macOS | `IOPortTransportState` on Apple silicon and `IODisplay` on Intel, through `ioreg -a` |
| Windows | the `EDID` value in the device key of each monitor, through SetupAPI |

It reads the base block of EDID 1.3 and 1.4: the maker, the model, the serial
number, the date, the size, the preferred mode and the ranges. It names each
extension block, and `--raw` gives the bytes. The built-in display of a laptop
can give no EDID.

Tested: unit tests with the EDID of a real monitor. CI runs it on the three
hosts. Hardware: macOS with a portable monitor through a USB-C adapter,
against the values that the IOKit registry shows. **Not yet tested on Linux
and Windows hardware.**

### `att free`

| Host | Source |
| :-- | :-- |
| Linux | `/proc/meminfo`, with the sums of `free` of procps |
| macOS | `host_statistics64`, `hw.memsize` and `vm.swapusage` |
| Windows | `GetPerformanceInfo`, and the page files of `EnumPageFilesW` |

On macOS the used memory is the sum that Activity Monitor shows: the memory of
the apps, the wired memory and the compressed memory. Windows gives no count of
the memory that holds nothing, so FREE shows `-` there.

Tested: unit tests with the values of each host. CI checks that the total is
above zero and that the used memory is not above it, on the three hosts.
Hardware: macOS, against `vm_stat`.

---

## Network

### `att netquality`

It measures the idle latency, then the download and the upload with 8
connections for 8 seconds each, and the latency of new connections while the
link is full. It uses the speed test server of Cloudflare over plain HTTP, so
the binary needs no TLS library.

Its responsiveness counts round trips on new connections only.
`networkQuality` of macOS also counts round trips inside the busy
connections, so its RPM is often lower on the same link. Compare a number of
`att netquality` with another number of `att netquality`.

Tested: CI on the three hosts, with no gate, because the network of a runner
is not the code of att. Hardware: macOS, beside `networkQuality`: the download
matched to 1 Mbps. The upload to Cloudflare matched `curl` to the same server.

### `att wifi`

| Host | Source | Need |
| :-- | :-- | :-- |
| Linux | `nmcli` of NetworkManager | NetworkManager |
| macOS | a scan of CoreWLAN | Location Services for the terminal, for the names |
| Windows | the native WLAN API | location access for desktop apps on Windows 11 |

macOS and Windows hide the name of each network from a program that has no
permission for the location. `att wifi` then shows each network with its
signal, its channel and its security, and says how to give the permission.

Tested: CI on the three hosts, which have no Wi-Fi card. Hardware: macOS with
21 networks. The security of each one matched `system_profiler`.
**Not yet tested on Linux and Windows hardware.**

### `att whoport`

| Host | Source | Privilege |
| :-- | :-- | :-- |
| Linux | `/proc/net/tcp`, `tcp6`, `udp`, `udp6`, and the socket links in `/proc/<pid>/fd` | root to see the processes of other users |
| macOS | `lsof -nP -i -F pcftPnT` | the same |
| Windows | `GetExtendedTcpTable` and `GetExtendedUdpTable`, with the names from a process snapshot | none |

`--kill` asks before it stops each process, and `--yes` skips the question for a
script. It sends SIGTERM on Linux and macOS, and it ends the process on
Windows.

Tested: CI starts a server on a free port on the three hosts, finds it, stops
it, and checks that the port is free. Hardware: macOS.

### `att wol`

It sends the magic packet as a UDP broadcast to port 9, with an optional
SecureOn password. It uses the standard library only.

Tested: a unit test sends the packet to a socket on the loopback address and
reads it whole. **Not yet tested to wake a real computer.**

---

## Shell

### `att timeout`

It takes the options and gives the exit codes of GNU `timeout`. On Linux and
macOS the command starts in a process group of its own, and the signal reaches
each process that the command starts. When the command runs in a terminal, its
group gets the terminal, as a shell gives it to a job. On Windows the command
goes into a job object, and the job ends each process in it. Windows has no
signals, so `--signal` names only the exit code there.

Tested: CI on the three hosts, with a check of exit code 124. Hardware: macOS,
with each exit code and with a child process that must stop too.

### `att awake`

| Host | How |
| :-- | :-- |
| Linux | `systemd-inhibit`, which holds a lock of logind |
| macOS | `IOPMAssertionCreateWithName`, as `caffeinate` does |
| Windows | `SetThreadExecutionState` |

logind gives the lock only to a user with a session. A CI runner has none, and
the tool says that logind refused.

Tested: CI on the three hosts. Hardware: macOS, with the assertions checked in
`pmset -g assertions`.

### `att clip`

| Host | How |
| :-- | :-- |
| Linux | `wl-copy` and `wl-paste` on Wayland, `xclip` or `xsel` on X11 |
| macOS | `pbcopy` and `pbpaste`, with a locale of UTF-8 |
| Windows | the clipboard API, with `CF_UNICODETEXT` |

Tested: CI copies and pastes on macOS and Windows, and checks the error on a
Linux runner with no desktop. Hardware: macOS. **Not yet tested on a Linux
desktop.**

### `att unquarantine`

| Host | The mark |
| :-- | :-- |
| Linux | the attributes `user.xdg.origin.url` and `user.xdg.referrer.url` |
| macOS | the attribute `com.apple.quarantine` |
| Windows | the stream `Zone.Identifier` |

`--check` says what each mark holds, such as the program or the address of the
download, and it removes nothing.

Tested: CI puts a mark on a file with the tool of each host, and removes it.
Hardware: macOS.

### `att ldd`

It reads ELF, Mach-O and PE on each host, so a binary of one host can be read
on another. It finds each library by the rules of the loader of its format:
`ld.so` with `DT_RPATH`, `LD_LIBRARY_PATH`, `DT_RUNPATH` and `/etc/ld.so.conf`;
dyld with `@rpath`, `@loader_path` and `@executable_path`; and the search of
Windows. A library of macOS in the dyld shared cache, and an API set of
Windows, have no file, and the output says so.

It reads the binary and runs nothing. The `ldd` of Linux can run the program.

Tested: CI reads the binary of att on the three hosts. Hardware: macOS,
against `otool -L` for programs of Homebrew.

### `att watch`

It runs the command through `sh -c` on Linux and macOS and through `cmd /C`
on Windows, as `watch` does, or directly with `-x`. It shows the output and the
error stream, cut to the size of the terminal. `-d` marks the characters that
changed, `-g` stops when the output changes, and `-e` stops when the command
fails, with the exit code of the command.

Tested: unit tests of the screen and the marks. CI checks `-g` and `-e` on the
three hosts. Hardware: macOS.

### `att notify`

| Host | How |
| :-- | :-- |
| Linux | `notify-send`, or the notification server of the desktop through `gdbus` |
| macOS | `display notification` of AppleScript, through `osascript` |
| Windows | a toast of Windows Runtime, through PowerShell |

The title and the message go to each host as data, and never into the text of
a script, so a quote in a message changes nothing. On macOS the notification
comes from Script Editor, and System Settings can turn those off.

Tested: unit tests of the quotes. CI shows a notification on macOS and
Windows, and checks the error on a Linux runner with no desktop. Hardware:
macOS. **Not yet tested on a Linux desktop.**

### `att trash`

| Host | How |
| :-- | :-- |
| Linux | the trash of the freedesktop.org specification, which the file managers read |
| macOS | `trashItemAtURL` of NSFileManager, as the Finder does |
| Windows | `SHFileOperationW` with the flag that allows an undo |

The Finder and the Recycle Bin can put a file back. On Linux a file on the
volume of the home folder goes to `~/.local/share/Trash`, and a file on
another volume goes to `.Trash-<uid>` at the top of that volume. A symbolic
link goes to the trash, and not the file that it points to. A volume of
Windows with no Recycle Bin, such as a network share, cannot keep the file,
and Windows then asks before it deletes the file for good.

Tested: unit tests of the info file and the names. CI moves a file to the
trash on the three hosts. Hardware: macOS with a file, a folder and a link.
**Not yet tested on a Linux desktop or on Windows hardware.**

### `att hexdump`

It takes the options of `xxd`: `-c`, `-g`, `-b`, `-u`, `-s`, `-l`, `-p`,
`-i`, `-n` and `-r`. It writes the same text as `xxd`, and `-r` turns the text
of a dump or of `-p` back into bytes. With an output file, `-r` changes the
file at the offsets of the dump and does not cut it short, as `xxd -r` does.

Tested: unit tests with the output of `xxd` 2025-08-24. CI turns a dump back
into the same bytes on the three hosts. Hardware: macOS, against `xxd` for 15
sets of options.

### `att link`

It makes a symbolic link to `att` for each familiar name on Linux and macOS,
and a `.cmd` file on Windows. It skips a name that the host has on its PATH,
unless `--force`.

Tested: CI makes the names, runs one, and removes them, on the three hosts.
