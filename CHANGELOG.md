<div align="center">
  <a href="README.md"><img src="assets/logo.svg" alt="AllTheTools" height="44"></a>
</div>

# Changelog

Every release is written here, newest first. A version is `MAJOR.MINOR.PATCH`.

## Unreleased

Nothing yet.

## 0.2.0 (2026-10-16)

Seven new tools:

- `flash` writes a disk image to a USB drive, and reads it back to check it.
- `edid` shows the maker, the model, the serial number and the native mode of
  each connected display.
- `free` shows the memory and the swap.
- `watch` runs a command again and again, and marks what changed.
- `notify` shows a notification on the desktop.
- `trash` moves files to the trash of the host, where a person can get them
  back.
- `hexdump` shows the bytes of a file with the options of `xxd`, and turns the
  dump back into bytes.

`att link` makes four more names: `watch`, `xxd`, `free` and `trash`.

## 0.1.0 (2026-10-10)

The first version, with twenty tools in one binary, `att`:

- Drives: `probe`, `surface`, `lsblk`, `eject`, `mount` and `whoholds`.
- Hardware: `lsusb`, `lspci`, `sensors`, `battery` and `ddc`.
- Network: `netquality`, `wifi`, `whoport` and `wol`.
- Shell: `timeout`, `awake`, `clip`, `unquarantine` and `ldd`.
- `att link`, which makes the familiar names, such as `lsusb` and `f3probe`,
  run `att`.
- Binaries for Linux, macOS and Windows on x86_64 and arm64, a `.deb` and a
  `.rpm`, and the files of Homebrew, Scoop and winget.
