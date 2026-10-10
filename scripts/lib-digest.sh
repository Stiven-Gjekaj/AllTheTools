# The digest of one file of a release, or a stop that names what is missing.
# SHA256SUMS holds lines of "<digest>  <file name>".
#
# Each package script sources this file, after it sets SUMS and SCRIPT.

digest() {
    local file="$1"
    local found
    found="$(awk -v f="$file" '$2 == f { print $1 }' "$SUMS")"
    if [[ ! "$found" =~ ^[0-9a-f]{64}$ ]]; then
        echo "${SCRIPT}: $SUMS holds no digest for $file" >&2
        exit 1
    fi
    echo "$found"
}

DESCRIPTION="The same small tools on Windows, on macOS and on Linux, in one binary"
