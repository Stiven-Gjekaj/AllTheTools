#!/usr/bin/env bash
# Run each tool of att that reads and changes nothing, on the host that runs
# this script.
#
# A unit test never reaches the host file of a tool. This script does, so it
# proves that each host file compiles, links, finds the host API and returns.
# A runner of CI has no USB drive, no battery and no monitor, so a tool may
# find nothing. A tool that finds nothing must say so and give exit code 0.
#
# Usage: scripts/smoke.sh <path to att>

set -euo pipefail

ATT="${1:?usage: smoke.sh <path to att>}"
# An absolute path, so a step that changes the folder still finds att.
ATT="$(cd "$(dirname "$ATT")" && pwd)/$(basename "$ATT")"

# The file of the binary. Windows adds .exe, and a shell finds it without the
# suffix, but a tool that reads the file needs the full name.
BIN="$ATT"
if [ -f "$ATT.exe" ]; then
    BIN="$ATT.exe"
fi

step() {
    echo "::group::att $*"
    "$ATT" "$@"
    echo "::endgroup::"
}

# Run a diskpart script on Windows. The shell of Git for Windows turns an
# argument that starts with a slash into a path, and /s is no path.
diskpart_script() {
    MSYS_NO_PATHCONV=1 diskpart /s "$(cygpath -w "$1")"
}

# Make an empty VHD of 64 MB on Windows, and attach it with no letter.
# diskpart makes it, and Storage Management attaches it, because diskpart
# lost track of a VHD after a write to its raw disk.
vhd_attach() {
    printf 'create vdisk file="%s" maximum=64 type=fixed\n' "$(cygpath -w "$1")" > "$1.txt"
    diskpart_script "$1.txt" > "$1.log" || { cat "$1.log"; exit 1; }
    powershell.exe -NoProfile -Command "Mount-DiskImage -ImagePath '$(cygpath -w "$1")' -Access ReadWrite -NoDriveLetter | Out-Null"
}

vhd_detach() {
    powershell.exe -NoProfile -Command "Dismount-DiskImage -ImagePath '$(cygpath -w "$1")' | Out-Null"
}

step --version

# timeout gives the exit code of GNU timeout. It stops nothing outside the
# command that it starts.
step timeout 5 true
set +e
"$ATT" timeout 1 sleep 5
code=$?
set -e
if [ "$code" != 124 ]; then
    echo "att timeout gave $code for a command past its time, and not 124" >&2
    exit 1
fi

# ldd reads the binary of att itself, which the loader of the host found.
step ldd "$BIN"
step ldd --tree --json "$BIN"

# unquarantine removes a mark that this script puts on a file with the tool of
# the host. It changes only that file, in a directory of its own.
mark_dir="$(mktemp -d)"
echo sample > "$mark_dir/download"
case "$(uname -s)" in
    Darwin) xattr -w com.apple.quarantine "0083;66f1c2a0;Safari;" "$mark_dir/download" ;;
    Linux) python3 -c 'import os, sys; os.setxattr(sys.argv[1], "user.xdg.origin.url", b"https://example.com/")' "$mark_dir/download" ;;
    # The shell of Git for Windows changes a colon in a file name, so
    # PowerShell writes the stream.
    *) powershell.exe -NoProfile -Command "Set-Content -Path '$(cygpath -w "$mark_dir/download")' -Stream Zone.Identifier -Value '[ZoneTransfer]','ZoneId=3'" ;;
esac
step unquarantine --check "$mark_dir/download"
"$ATT" unquarantine --check "$mark_dir/download" | grep -q "1 of 1"
step unquarantine "$mark_dir/download"
"$ATT" unquarantine --check "$mark_dir/download" | grep -q "0 of 1"
rm -rf "$mark_dir"

# clip copies a line and pastes it back. A Linux runner has no desktop
# session and so no clipboard, and the tool says so. The check runs only in
# CI, so a run on the computer of a person does not replace their clipboard.
case "${CI:-}-$(uname -s)" in
    -*) ;;
    *-Linux)
        if "$ATT" clip paste; then
            echo "att clip pasted with no desktop session" >&2
            exit 1
        fi
        ;;
    *)
        "$ATT" clip copy "att smoke $$"
        test "$("$ATT" clip paste)" = "att smoke $$"
        ;;
esac

# awake holds the system awake for one second, and for one short command.
# logind gives the lock only to a user with a session, and a Linux runner
# has none. There the tool must refuse with the reason of logind.
case "$(uname -s)" in
    Linux)
        if out="$("$ATT" awake --time 1 2>&1)"; then
            echo "att awake held a lock with no session: $out" >&2
            exit 1
        fi
        echo "$out"
        echo "$out" | grep -q "logind refused the lock"
        ;;
    *)
        step awake --time 1
        step awake --display true
        ;;
esac

# lsusb lists the USB devices. A runner can have none.
step lsusb
step lsusb --long
step lsusb --json

# lspci lists the PCI devices. Each runner is a virtual machine with some.
step lspci
step lspci --json

# battery reads the batteries. A runner has none, and the tool says so.
step battery
step battery --json

# sensors reads what the host gives to a standard user. A virtual machine
# can have no sensor, and the tool says so.
step sensors
step sensors --json

# whoport finds a server that this script starts, and stops it. Nothing
# else is stopped. The server takes a free port itself and writes its number
# to a file, so no other program can take the port between the two.
step whoport
PY=python3
command -v python3 >/dev/null 2>&1 || PY=python
port_file="$(mktemp)"
rm -f "$port_file"
"$PY" -c '
import socket, sys, time
s = socket.socket()
s.bind(("127.0.0.1", 0))
s.listen(1)
open(sys.argv[1], "w").write(str(s.getsockname()[1]))
time.sleep(120)
' "$port_file" &
server=$!
for _ in $(seq 1 50); do
    [ -s "$port_file" ] && break
    sleep 0.2
done
port="$(cat "$port_file")"
if ! "$ATT" whoport "$port"; then
    echo "att whoport did not find the server on port $port" >&2
    lsof -nP -iTCP:"$port" 2>/dev/null || true
    kill "$server" 2>/dev/null || true
    exit 1
fi
step whoport "$port" --kill --yes
sleep 1
if "$ATT" whoport "$port" >/dev/null 2>&1; then
    echo "att whoport --kill left the server on port $port" >&2
    kill "$server" 2>/dev/null || true
    exit 1
fi

# wifi scans. A runner has no Wi-Fi card, and the tool says so. A Linux
# runner has no NetworkManager, and the tool names the package.
case "$(uname -s)" in
    Linux)
        if out="$("$ATT" wifi 2>&1)"; then
            echo "$out"
        else
            echo "$out"
            echo "$out" | grep -q "NetworkManager"
        fi
        ;;
    *) step wifi ;;
esac

# netquality needs the internet and a server that answers. A failure of the
# network is not a failure of att, so this step prints its result and does
# not stop the script.
echo "::group::att netquality --time 2 --connections 2"
"$ATT" netquality --time 2 --connections 2 || echo "att netquality did not complete. This step does not gate."
echo "::endgroup::"

# lsblk lists the drives. Each runner has a system drive, and the list must
# mark it.
step lsblk
step lsblk --drives
"$ATT" lsblk --json | grep -q '"system":true' || {
    echo "att lsblk found no system drive on this runner" >&2
    exit 1
}

# eject refuses the system drive on each host. On macOS it also ejects a
# disk image that this script makes and attaches.
PY=python3
command -v python3 >/dev/null 2>&1 || PY=python
system="$("$ATT" lsblk --json | "$PY" -c 'import json, sys; print(next(d["id"] for d in json.load(sys.stdin) if d["system"]))')"
if out="$("$ATT" eject "$system" 2>&1)"; then
    echo "att eject took the system drive $system: $out" >&2
    exit 1
fi
echo "$out" | grep -q "running system"
if [ "$(uname -s)" = Darwin ]; then
    image_dir="$(mktemp -d)"
    hdiutil create -quiet -size 64m -fs MS-DOS -volname ATTSMOKE "$image_dir/smoke.dmg"
    device="$(hdiutil attach -nobrowse "$image_dir/smoke.dmg" | awk '/ATTSMOKE/ { print $1 }')"
    step eject "$device"
    if hdiutil info | grep -q "$image_dir/smoke.dmg"; then
        echo "att eject left the image attached" >&2
        exit 1
    fi
    rm -rf "$image_dir"
fi

# mount attaches an image that this script makes with the tools of the
# host, reads a file back through the mount, and detaches it.
image_dir="$(mktemp -d)"
case "$(uname -s)" in
    Darwin)
        mkdir "$image_dir/tree"
        echo "att smoke" > "$image_dir/tree/check.txt"
        hdiutil makehybrid -quiet -iso -joliet -default-volume-name ATTISO -o "$image_dir/smoke.iso" "$image_dir/tree"
        step mount "$image_dir/smoke.iso"
        grep -q "att smoke" /Volumes/ATTISO/check.txt
        step mount --detach "$image_dir/smoke.iso"
        ;;
    Linux)
        # A runner has no udisks, so att mount takes the path of root.
        truncate -s 32M "$image_dir/smoke.img"
        mkfs.ext4 -q "$image_dir/smoke.img"
        echo "::group::sudo att mount"
        sudo "$ATT" mount "$image_dir/smoke.img"
        echo "::endgroup::"
        mountpoint="$(findmnt -n -o TARGET -S "$(losetup -j "$image_dir/smoke.img" | cut -d: -f1)")"
        test -n "$mountpoint"
        echo "::group::sudo att mount --detach"
        sudo "$ATT" mount --detach "$image_dir/smoke.img"
        echo "::endgroup::"
        test -z "$(losetup -j "$image_dir/smoke.img")"
        ;;
    *)
        vhd="$(cygpath -w "$image_dir/smoke.vhd")"
        # FAT32 needs a volume of more than 32 MB.
        printf 'create vdisk file="%s" maximum=64 type=fixed\nselect vdisk file="%s"\nattach vdisk\ncreate partition primary\nformat fs=fat32 quick label=ATTVHD\ndetach vdisk\n' "$vhd" "$vhd" > "$image_dir/make.txt"
        if ! diskpart_script "$image_dir/make.txt" > "$image_dir/diskpart.log"; then
            cat "$image_dir/diskpart.log"
            exit 1
        fi
        step mount "$image_dir/smoke.vhd"
        step mount --detach "$image_dir/smoke.vhd"
        ;;
esac
rm -rf "$image_dir"

# whoholds finds a process that this script starts to hold a file open.
hold_dir="$(mktemp -d)"
echo data > "$hold_dir/held.txt"
"$PY" -c '
import sys, time
f = open(sys.argv[1])
open(sys.argv[2], "w").write("open")
time.sleep(60)
' "$hold_dir/held.txt" "$hold_dir/ready" &
holder=$!
for _ in $(seq 1 50); do
    [ -s "$hold_dir/ready" ] && break
    sleep 0.2
done
step whoholds "$hold_dir/held.txt"
"$ATT" whoholds --json "$hold_dir/held.txt" | grep -qi "python" || {
    echo "att whoholds did not find the process that holds the file" >&2
    kill "$holder" 2>/dev/null || true
    exit 1
}
kill "$holder" 2>/dev/null || true
wait "$holder" 2>/dev/null || true
rm -rf "$hold_dir"

# surface reads an image file, then writes it and reads it back. The answer
# to the question comes from a pipe. Linux and Windows then do the same on a
# real block device that backs a file: a loop device, and an attached VHD.
scan_dir="$(mktemp -d)"
head -c 33554432 /dev/zero > "$scan_dir/scan.img"
step surface "$scan_dir/scan.img"
printf 'scan.img\n' | "$ATT" surface --write "$scan_dir/scan.img"
case "$(uname -s)" in
    Linux)
        loop="$(sudo losetup --find --show "$scan_dir/scan.img")"
        echo "::group::sudo att surface $loop"
        sudo "$ATT" surface "$loop"
        printf '%s\n' "${loop#/dev/}" | sudo "$ATT" surface --write --force "$loop"
        echo "::endgroup::"
        sudo losetup --detach "$loop"
        ;;
    Darwin) ;;
    *)
        vhd_attach "$scan_dir/scan.vhd"
        number="$("$ATT" lsblk --json | "$PY" -c 'import json, sys; print([d["id"] for d in json.load(sys.stdin) if d["virtual_disk"]][-1])')"
        step surface "$number"
        printf '%s\n' "$number" | "$ATT" surface --write --force "$number"
        vhd_detach "$scan_dir/scan.vhd"
        ;;
esac
rm -rf "$scan_dir"

# probe tests an image of random bytes, and the image must hold the same
# bytes after the test. Linux and macOS do the same on a block device that
# backs the file, and Windows on an attached VHD.
hash_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d' ' -f1
    else
        shasum -a 256 "$1" | cut -d' ' -f1
    fi
}
probe_dir="$(mktemp -d)"
head -c 67108864 /dev/urandom > "$probe_dir/probe.img"
before="$(hash_of "$probe_dir/probe.img")"
printf 'probe.img\n' | "$ATT" probe "$probe_dir/probe.img"
test "$(hash_of "$probe_dir/probe.img")" = "$before"
case "$(uname -s)" in
    Linux)
        loop="$(sudo losetup --find --show "$probe_dir/probe.img")"
        echo "::group::sudo att probe $loop"
        printf '%s\n' "${loop#/dev/}" | sudo "$ATT" probe --force "$loop"
        echo "::endgroup::"
        sudo losetup --detach "$loop"
        test "$(hash_of "$probe_dir/probe.img")" = "$before"
        ;;
    Darwin)
        device="$(hdiutil attach -nobrowse -nomount -imagekey diskimage-class=CRawDiskImage "$probe_dir/probe.img" | awk 'NR == 1 { print $1 }')"
        echo "::group::att probe $device"
        printf '%s\n' "${device#/dev/}" | "$ATT" probe "$device"
        echo "::endgroup::"
        hdiutil detach "$device" > /dev/null
        test "$(hash_of "$probe_dir/probe.img")" = "$before"
        ;;
    *)
        vhd_attach "$probe_dir/probe.vhd"
        number="$("$ATT" lsblk --json | "$PY" -c 'import json, sys; print([d["id"] for d in json.load(sys.stdin) if d["virtual_disk"]][-1])')"
        echo "::group::att probe $number"
        printf '%s\n' "$number" | "$ATT" probe --force "$number"
        echo "::endgroup::"
        vhd_detach "$probe_dir/probe.vhd"
        ;;
esac
rm -rf "$probe_dir"

# ddc lists the monitors that answer. A runner has none, and the tool says
# so.
step ddc list

# hexdump writes the dump of a file, and the reverse gives the same bytes.
hex_dir="$(mktemp -d)"
head -c 5000 /dev/urandom > "$hex_dir/data.bin"
step hexdump -l 64 "$hex_dir/data.bin"
"$ATT" hexdump "$hex_dir/data.bin" | "$ATT" hexdump -r > "$hex_dir/back.bin"
cmp "$hex_dir/data.bin" "$hex_dir/back.bin"
"$ATT" hexdump -p "$hex_dir/data.bin" | "$ATT" hexdump -r -p > "$hex_dir/plain.bin"
cmp "$hex_dir/data.bin" "$hex_dir/plain.bin"
rm -rf "$hex_dir"

# free reads the memory of the host. Each runner has memory, and the JSON
# must give a total above zero.
step free
step free --json
"$ATT" free --json | "$PY" -c 'import json, sys; m = json.load(sys.stdin); assert m["total"] > 0 and m["used"] <= m["total"], m'

# watch stops when the output changes, and when the command fails it gives
# the exit code of the command.
step watch -g -n 0.2 -x "$PY" -c 'import time; print(time.time())'
set +e
"$ATT" watch -e -n 0.2 -x "$PY" -c 'import sys; sys.exit(3)' > /dev/null
code=$?
set -e
if [ "$code" != 3 ]; then
    echo "att watch -e gave $code for a command that gave 3" >&2
    exit 1
fi

# edid reads the displays. A runner can have none, and the tool says so.
step edid
step edid --json

# notify and trash change the desktop of the person, so they run only in CI.
# A Linux runner has no desktop session, and notify must say so.
case "${CI:-}-$(uname -s)" in
    -*) ;;
    *-Linux)
        if out="$("$ATT" notify "att smoke" 2>&1)"; then
            echo "att notify showed a notification with no desktop session: $out" >&2
            exit 1
        fi
        echo "$out"
        # The error must come from the missing session, and not from a bad
        # call of gdbus, which prints its usage.
        if echo "$out" | grep -q "Usage"; then
            echo "att notify called gdbus with bad arguments" >&2
            exit 1
        fi
        ;;
    *) step notify -t "att smoke" "CI checks att notify" ;;
esac
if [ -n "${CI:-}" ]; then
    trash_dir="$(mktemp -d)"
    echo "att smoke" > "$trash_dir/trash me.txt"
    step trash -v "$trash_dir/trash me.txt"
    test ! -e "$trash_dir/trash me.txt"
    if [ "$(uname -s)" = Linux ]; then
        ls ~/.local/share/Trash/info/ "$trash_dir"/../.Trash-* 2>/dev/null || true
    fi
    if "$ATT" trash "$trash_dir/no such file" 2>/dev/null; then
        echo "att trash took a path that does not exist" >&2
        exit 1
    fi
    rm -rf "$trash_dir"
fi

# flash writes an image to a file, and to a block device that backs a file:
# a loop device on Linux, the raw node of an image on macOS, and an attached
# VHD on Windows. flash reads each one back itself, and gives exit code 2 if
# a byte differs.
flash_dir="$(mktemp -d)"
head -c 10000001 /dev/urandom > "$flash_dir/in.img"
head -c 33554432 /dev/zero > "$flash_dir/out.img"
printf 'out.img\n' | "$ATT" flash "$flash_dir/in.img" "$flash_dir/out.img"
head -c 10000001 "$flash_dir/out.img" | cmp - "$flash_dir/in.img"
case "$(uname -s)" in
    Linux)
        loop="$(sudo losetup --find --show "$flash_dir/out.img")"
        echo "::group::sudo att flash $loop"
        printf '%s\n' "${loop#/dev/}" | sudo "$ATT" flash --force "$flash_dir/in.img" "$loop"
        echo "::endgroup::"
        sudo losetup --detach "$loop"
        ;;
    Darwin)
        device="$(hdiutil attach -nobrowse -nomount -imagekey diskimage-class=CRawDiskImage "$flash_dir/out.img" | awk 'NR == 1 { print $1 }')"
        echo "::group::att flash $device"
        printf '%s\n' "${device#/dev/}" | "$ATT" flash "$flash_dir/in.img" "$device"
        echo "::endgroup::"
        hdiutil detach "$device" > /dev/null
        ;;
    *)
        vhd_attach "$flash_dir/flash.vhd"
        number="$("$ATT" lsblk --json | "$PY" -c 'import json, sys; print([d["id"] for d in json.load(sys.stdin) if d["virtual_disk"]][-1])')"
        echo "::group::att flash $number"
        printf '%s\n' "$number" | "$ATT" flash --force "$flash_dir/in.img" "$number"
        echo "::endgroup::"
        vhd_detach "$flash_dir/flash.vhd"
        ;;
esac
head -c 10000001 "$flash_dir/out.img" | cmp - "$flash_dir/in.img"
rm -rf "$flash_dir"

# df lists the volumes. Each runner has a volume with a size, and the
# volume of the current folder must be one of them.
step df
step df .
"$ATT" df --json | "$PY" -c 'import json, sys; v = json.load(sys.stdin); assert v and all(x["used"] <= x["size"] for x in v), v'

# du adds up a folder that this script makes, with sizes that it knows.
du_dir="$(mktemp -d)"
mkdir "$du_dir/sub"
head -c 1000 /dev/zero > "$du_dir/a.bin"
head -c 5000 /dev/zero > "$du_dir/sub/b.bin"
step du "$du_dir"
"$ATT" du --apparent-size --json "$du_dir" | "$PY" -c '
import json, sys
r = json.load(sys.stdin)[0]
e = {x["path"]: x for x in r["entries"]}
assert e["a.bin"]["size"] == 1000 and not e["a.bin"]["folder"], e
assert e["sub"]["size"] >= 5000 and e["sub"]["folder"], e
assert r["entries"][0]["path"] == "sub", r
assert r["total"] >= 6000, r'
rm -rf "$du_dir"

# hash gives the same digests as hashlib of Python, and the check finds a
# file that changed.
hash_dir="$(mktemp -d)"
head -c 100003 /dev/urandom > "$hash_dir/data.bin"
for algorithm in md5 sha1 sha256 sha512; do
    ours="$("$ATT" hash -a "$algorithm" "$hash_dir/data.bin" | cut -d' ' -f1)"
    theirs="$("$PY" -c 'import hashlib, sys; print(hashlib.new(sys.argv[1], open(sys.argv[2], "rb").read()).hexdigest())' "$algorithm" "$hash_dir/data.bin")"
    if [ "$ours" != "$theirs" ]; then
        echo "att hash -a $algorithm gave $ours, and Python gave $theirs" >&2
        exit 1
    fi
done
(cd "$hash_dir" && "$ATT" hash data.bin > SHA256SUMS)
step hash -c "$hash_dir/SHA256SUMS"
printf 'x' >> "$hash_dir/data.bin"
set +e
"$ATT" hash -c "$hash_dir/SHA256SUMS"
code=$?
set -e
if [ "$code" != 2 ]; then
    echo "att hash -c gave $code for a file that changed, and not 2" >&2
    exit 1
fi
rm -rf "$hash_dir"

# lscpu reads the processor. Each runner has cores and threads.
step lscpu
"$ATT" lscpu --json | "$PY" -c 'import json, sys; c = json.load(sys.stdin); assert c["cores"] > 0 and c["threads"] >= c["cores"], c'

# ip lists the interfaces. Each host has a loopback interface with
# 127.0.0.1.
step ip
"$ATT" ip --json | "$PY" -c '
import json, sys
found = [a["address"] for i in json.load(sys.stdin) for a in i["addresses"]]
assert "127.0.0.1" in found, found'

# uptime gives the time since the start of the runner.
step uptime
step uptime --since
"$ATT" uptime --json | "$PY" -c 'import json, sys; u = json.load(sys.stdin); assert u["seconds"] > 0 and u["boot"] > 1600000000, u'

# flushdns and say change the state of the computer of the person, so they
# run only in CI. flushdns needs root on macOS and Linux. A Linux runner has
# no speech program, and say must name the package.
if [ -n "${CI:-}" ]; then
    case "$(uname -s)" in
        Linux | Darwin)
            echo "::group::sudo att flushdns"
            sudo "$ATT" flushdns
            echo "::endgroup::"
            ;;
        *) step flushdns ;;
    esac
    case "$(uname -s)" in
        Linux)
            if out="$("$ATT" say "att smoke" 2>&1)"; then
                echo "$out"
            else
                echo "$out"
                echo "$out" | grep -q "espeak"
            fi
            ;;
        *)
            # A runner has no sound device, so the speech goes to a file.
            step say --list
            say_dir="$(mktemp -d)"
            step say -r 300 -o "$say_dir/smoke.wav" "att smoke"
            test "$(head -c 4 "$say_dir/smoke.wav")" = RIFF
            rm -rf "$say_dir"
            ;;
    esac
fi

# link makes the familiar names in a directory of this script, runs one of
# them, and removes them all.
link_dir="$(mktemp -d)"
step link "$link_dir"
case "$(uname -s)" in
    Linux | Darwin) "$link_dir/wakeonlan" --broadcast 127.0.0.1 aa:bb:cc:dd:ee:ff ;;
    *) MSYS_NO_PATHCONV=1 cmd /c "$(cygpath -w "$link_dir/wakeonlan.cmd")" --broadcast 127.0.0.1 aa:bb:cc:dd:ee:ff ;;
esac
step link --remove "$link_dir"
test -z "$(ls -A "$link_dir")"
rm -rf "$link_dir"
