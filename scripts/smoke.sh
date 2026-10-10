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

# whoport finds a server that this script starts on a free port, and stops
# it. Nothing else is stopped.
step whoport
port=$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])' 2>/dev/null \
    || python -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])')
PY=python3
command -v python3 >/dev/null 2>&1 || PY=python
"$PY" -m http.server "$port" --bind 127.0.0.1 >/dev/null 2>&1 &
server=$!
for _ in 1 2 3 4 5 6 7 8 9 10; do
    "$ATT" whoport "$port" >/dev/null 2>&1 && break
    sleep 1
done
step whoport "$port"
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
