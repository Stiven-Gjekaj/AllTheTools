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
    *) printf '[ZoneTransfer]\r\nZoneId=3\r\n' > "$mark_dir/download:Zone.Identifier" ;;
esac
step unquarantine --check "$mark_dir/download"
"$ATT" unquarantine --check "$mark_dir/download" | grep -q "1 of 1"
step unquarantine "$mark_dir/download"
"$ATT" unquarantine --check "$mark_dir/download" | grep -q "0 of 1"
rm -rf "$mark_dir"
