#!/usr/bin/env bash
# Write the Scoop manifest of one release, from the SHA256SUMS of that
# release.
#
# The manifest installs the Windows binary of the release, for x86_64 and for
# arm64. A person puts it into the bucket and commits it there: no bot pushes
# to the bucket.
#
# Usage: scripts/scoop-manifest.sh <version> <SHA256SUMS> [base-url] > allthetools.json

set -euo pipefail

USAGE="usage: scoop-manifest.sh <version> <SHA256SUMS> [base-url]"
VERSION="${1:?$USAGE}"
SUMS="${2:?$USAGE}"
BASE="${3:-https://github.com/Stiven-Gjekaj/AllTheTools/releases/download/v${VERSION}}"
SCRIPT="scoop-manifest.sh"
source "$(dirname "$0")/lib-digest.sh"

X64="$(digest "att-${VERSION}-x86_64-pc-windows-msvc.exe")"
ARM64="$(digest "att-${VERSION}-aarch64-pc-windows-msvc.exe")"

# The address of a later release, for the autoupdate of Scoop. Scoop puts the
# version where $version stands, and it hashes the file itself.
LATER='https://github.com/Stiven-Gjekaj/AllTheTools/releases/download/v$version/att-$version'

cat <<MANIFEST
{
    "version": "${VERSION}",
    "description": "${DESCRIPTION}",
    "homepage": "https://github.com/Stiven-Gjekaj/AllTheTools",
    "license": "MIT",
    "notes": "att link makes the familiar names, such as lsusb and f3probe, run att. att probe and att surface need a shell that runs as Administrator.",
    "architecture": {
        "64bit": {
            "url": "${BASE}/att-${VERSION}-x86_64-pc-windows-msvc.exe#/att.exe",
            "hash": "${X64}"
        },
        "arm64": {
            "url": "${BASE}/att-${VERSION}-aarch64-pc-windows-msvc.exe#/att.exe",
            "hash": "${ARM64}"
        }
    },
    "bin": "att.exe",
    "checkver": "github",
    "autoupdate": {
        "architecture": {
            "64bit": {
                "url": "${LATER}-x86_64-pc-windows-msvc.exe#/att.exe"
            },
            "arm64": {
                "url": "${LATER}-aarch64-pc-windows-msvc.exe#/att.exe"
            }
        }
    }
}
MANIFEST
