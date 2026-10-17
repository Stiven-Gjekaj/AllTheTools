<div align="center">
  <a href="../README.md"><b>AllTheTools</b></a>
</div>

# The JSON output

Each tool that lists or measures takes `--json`, and prints one line of JSON
on the output stream. This file names each field. The tests in
`crates/allthetools/src/contract.rs` hold the same names, so a change to a
field fails the tests before it reaches a release.

From version 1.0, the fields of this file stay as they are for each 1.x
release. A 1.x release can add a field. A script must skip a field that it
does not know. The file docs/stability.md of version 0.9 gives the rules.

## The rules for each field

- A size is in bytes, as a whole number.
- A time is in seconds, unless the name of the field gives another unit, such
  as `_ms` for milliseconds.
- A field that the host does not give is `null`. It is never left out.
- A list that has no item is `[]`. It is never `null`.
- A word in a field, such as a state, is one of the words that this file
  names. A host can give other words only where this file says so.

## Drives

### `att lsblk --json`

A list of disks.

| Field | Type | Meaning |
| --- | --- | --- |
| `id` | string | The name that each disk tool takes, such as `disk4`, `sdb` or `2` |
| `path` | string | The device of the disk |
| `raw_path` | string | The device that the tools that write open |
| `size` | number | The size of the disk |
| `model` | string or null | The model that the disk reports |
| `serial` | string or null | The serial number |
| `bus` | string or null | The bus, such as `USB`, `SATA` or `NVMe` |
| `removable` | bool | True when the host marks the disk as removable |
| `system` | bool | True when the running system starts from the disk |
| `read_only` | bool | True when the disk takes no write |
| `virtual_disk` | bool | True for a disk image or a virtual disk |
| `partitions` | list | The partitions, each as below |

A partition:

| Field | Type | Meaning |
| --- | --- | --- |
| `id` | string | The name of the partition, such as `disk4s1` |
| `path` | string | The device of the partition |
| `size` | number | The size of the partition |
| `filesystem` | string or null | The type of the file system |
| `label` | string or null | The name of the volume |
| `mount` | string or null | The mount point |

### `att df --json`

A list of volumes.

| Field | Type | Meaning |
| --- | --- | --- |
| `source` | string | The device, or the label of the volume on Windows |
| `fstype` | string | The type of the file system |
| `mount` | string | The mount point, or the drive letter on Windows |
| `size` | number | The size of the volume |
| `used` | number | The used space |
| `available` | number | The space that a user who is not root can still write |

### `att du --json`

A list with one report for each path.

| Field | Type | Meaning |
| --- | --- | --- |
| `path` | string | The path as the person gave it |
| `total` | number | The size of the path and all that it holds |
| `entries` | list | The largest entries, the largest first |

An entry:

| Field | Type | Meaning |
| --- | --- | --- |
| `path` | string | The path under the start, with `/` between the names |
| `size` | number | The size of the entry and all that it holds |
| `folder` | bool | True for a folder |

### `att mount --json`

A list of the devices that an attach gave.

| Field | Type | Meaning |
| --- | --- | --- |
| `device` | string | The device of the image or of its partition |
| `mount` | string or null | The mount point |
| `filesystem` | string or null | The type of the file system |

### `att whoholds --json`

A list of processes.

| Field | Type | Meaning |
| --- | --- | --- |
| `pid` | number | The process ID |
| `process` | string | The name of the program |
| `how` | string | How the process holds the path, such as `open file` or `working directory` |
| `path` | string or null | The file that the process holds |

### `att probe --json`

One object.

| Field | Type | Meaning |
| --- | --- | --- |
| `announced` | number | The size that the drive reports |
| `usable` | number | The size that keeps data |
| `blocks_written` | number | The blocks that the test wrote |
| `not_restored` | number | The blocks that the test could not write back. Zero after a run that completed |

### `att surface --json`

One object.

| Field | Type | Meaning |
| --- | --- | --- |
| `size` | number | The size of the drive |
| `sector` | number | The size of one sector |
| `checked` | number | The bytes that the scan read, or wrote and read back |
| `bad` | list of numbers | The offset of each bad sector, up to 100 |
| `bad_count` | number | The count of all bad sectors |
| `slow` | list of pairs | The offset and the time in milliseconds of each slow chunk |
| `seconds` | number | The time of the scan |

### `att flash --json`

One object.

| Field | Type | Meaning |
| --- | --- | --- |
| `image` | string | The image file |
| `target` | string | The drive or the file, for a person |
| `bytes` | number | The bytes that the tool wrote |
| `verified` | bool | True when the drive read back the same bytes |
| `mismatch` | number or null | The offset of the first byte that read back wrong |
| `seconds` | number | The time of the write and of the check |

## Hardware

### `att lsusb --json`

A list of devices.

| Field | Type | Meaning |
| --- | --- | --- |
| `bus` | number or null | The bus. Windows gives none |
| `address` | number or null | The address on the bus. Windows gives none |
| `vendor_id` | number | The vendor ID |
| `product_id` | number | The product ID |
| `vendor` | string or null | The vendor that the device reports |
| `product` | string or null | The product that the device reports |
| `serial` | string or null | The serial number |
| `speed_mbps` | number or null | The speed of the link |
| `class` | number or null | The USB class of the device |

### `att lspci --json`

A list of devices.

| Field | Type | Meaning |
| --- | --- | --- |
| `slot` | string or null | The address on the bus, such as `00:02.0` |
| `vendor_id` | number | The vendor ID |
| `device_id` | number | The device ID |
| `class` | number or null | The class code |
| `revision` | number or null | The revision |
| `vendor` | string or null | The name of the vendor |
| `device` | string or null | The name of the device |

### `att sensors --json`

A list of readings.

| Field | Type | Meaning |
| --- | --- | --- |
| `chip` | string | The source of the reading |
| `label` | string | The name of the sensor |
| `kind` | string | `temperature` in degrees Celsius, `fan` in turns each minute, or `voltage` in volts |
| `value` | number | The value now |
| `high` | number or null | The high limit |
| `critical` | number or null | The critical limit |

### `att battery --json`

A list of batteries.

| Field | Type | Meaning |
| --- | --- | --- |
| `name` | string or null | The name of the battery |
| `maker` | string or null | The maker |
| `serial` | string or null | The serial number |
| `state` | string | `charging`, `discharging`, `full`, `not_charging` or `unknown` |
| `percent` | number or null | The charge now, as a percentage of a full charge |
| `unit` | string | The unit of the capacities: `mWh` or `mAh` |
| `design_capacity` | number or null | The capacity when the battery was new |
| `full_capacity` | number or null | The capacity of a full charge now |
| `cycles` | number or null | The count of charge cycles |
| `design_cycles` | number or null | The cycles that the maker designs the battery for |
| `voltage` | number or null | The voltage, in volts |
| `temperature_c` | number or null | The temperature, in degrees Celsius |
| `minutes_left` | number or null | The time to empty, or to full |
| `host_health` | number or null | The health that the settings of the host show |

### `att free --json`

One object.

| Field | Type | Meaning |
| --- | --- | --- |
| `total` | number | The memory of the host |
| `used` | number | The memory that programs and the system hold |
| `free` | number or null | The memory that holds nothing. Windows gives none |
| `cache` | number or null | The memory that holds a copy of files |
| `available` | number | The memory that a new program can get with no swap |
| `swap_total` | number | The size of the swap |
| `swap_used` | number | The used swap |

### `att lscpu --json`

One object.

| Field | Type | Meaning |
| --- | --- | --- |
| `architecture` | string | The architecture of att, such as `x86_64` or `aarch64` |
| `model` | string or null | The name of the processor |
| `vendor` | string or null | The maker |
| `sockets` | number or null | The count of processor packages |
| `cores` | number | The count of cores |
| `threads` | number | The count of logical processors |
| `kinds` | list | The kinds of cores, when the processor has more than one |
| `max_mhz` | number or null | The highest speed |
| `caches` | list | The caches, each group as below |
| `flags` | list of strings | The features of the processor |

A kind has `name` (string) and `cores` (number). A cache has `level`
(number), `kind` (`data`, `instruction` or `unified`), `size` (number, the
size of one cache) and `count` (number).

### `att edid --json`

A list of displays.

| Field | Type | Meaning |
| --- | --- | --- |
| `source` | string or null | The port of the display |
| `maker` | string | The three letters of the maker |
| `product` | number | The product code |
| `name` | string or null | The name of the model |
| `serial` | string or null | The serial number as text |
| `serial_number` | number | The serial number as a number. Zero says none |
| `week` | number or null | The week of manufacture |
| `year` | number | The year of manufacture, or the model year |
| `model_year` | bool | True when `year` is the model year |
| `version` | string | The version of EDID, such as `1.4` |
| `digital` | bool | True for a digital input |
| `width_cm` | number | The width of the screen |
| `height_cm` | number | The height of the screen |
| `preferred` | object or null | The preferred mode, as below |
| `vertical_hz` | pair or null | The lowest and the highest vertical rate |
| `horizontal_khz` | pair or null | The lowest and the highest horizontal rate |
| `max_pixel_clock_mhz` | number or null | The highest pixel clock |
| `extensions` | list of strings | The name of each extension block |
| `checksum_ok` | bool | True when the base block adds up |
| `edid` | string | The whole EDID, as hex digits |

A mode has `width`, `height`, `refresh_hz` and `pixel_clock_khz`.

## Network

### `att netquality --json`

One object.

| Field | Type | Meaning |
| --- | --- | --- |
| `server` | string | The name of the server |
| `address` | string | The address of the server |
| `idle_latency_ms` | number or null | The latency with no load |
| `download_mbps` | number | The speed of the download |
| `download_latency_ms` | number or null | The latency during the download |
| `upload_mbps` | number | The speed of the upload |
| `upload_latency_ms` | number or null | The latency during the upload |
| `responsiveness_rpm` | number or null | The round trips each minute under load |
| `probes` | number | The count of latency probes |

### `att wifi --json`

A list of networks, the strongest first.

| Field | Type | Meaning |
| --- | --- | --- |
| `ssid` | string or null | The name of the network |
| `bssid` | string or null | The hardware address of the access point |
| `signal_dbm` | number or null | The strength of the signal |
| `channel` | number or null | The channel |
| `band_ghz` | number or null | The band |
| `security` | string | The security mode, such as `open`, `WPA2` or `WPA3` |

### `att whoport --json`

A list of sockets.

| Field | Type | Meaning |
| --- | --- | --- |
| `proto` | string | `tcp` or `udp` |
| `address` | string | The local address |
| `port` | number | The local port |
| `remote` | string or null | The remote address and port |
| `state` | string or null | The state of a TCP socket, such as `LISTEN`, as the host names it |
| `pid` | number or null | The process ID |
| `process` | string or null | The name of the program |

### `att ip --json`

A list of interfaces.

| Field | Type | Meaning |
| --- | --- | --- |
| `name` | string | The name of the interface |
| `state` | string | `up`, `no link` or `down` |
| `loopback` | bool | True for a loopback interface |
| `mac` | string or null | The hardware address |
| `mtu` | number or null | The MTU |
| `addresses` | list | Each address, with `address` (string) and `prefix` (number) |

## Shell

### `att uptime --json`

One object.

| Field | Type | Meaning |
| --- | --- | --- |
| `seconds` | number | The time since the start |
| `boot` | number | The start, in seconds since 1970-01-01 UTC |
| `load` | list of three numbers, or null | The load of 1, 5 and 15 minutes. Windows gives none |

### `att ldd --json`

One object.

| Field | Type | Meaning |
| --- | --- | --- |
| `file` | string | The binary |
| `format` | string | `ELF 64-bit`, `ELF 32-bit`, `Mach-O`, `PE 64-bit` or `PE 32-bit` |
| `interpreter` | string or null | The program loader that an ELF names |
| `libraries` | list | Each library, as below |

A library:

| Field | Type | Meaning |
| --- | --- | --- |
| `name` | string | The name that the binary gives |
| `path` | string or null | The file that the loader finds |
| `status` | string | `found`, `not found`, `shared cache` or `API set` |
| `depth` | number | The level in the tree, 1 for a library that the binary names |
| `repeat` | bool | True when the library shows higher in the tree already |
