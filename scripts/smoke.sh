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
