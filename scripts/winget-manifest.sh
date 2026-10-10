#!/usr/bin/env bash
# Write the winget manifest of one release, from the SHA256SUMS of that
# release.
#
# The manifest installs the Windows binary of the release as a portable
# command, for x86_64 and for arm64. A person opens the pull request to
# winget-pkgs: no bot does.
#
# Usage: scripts/winget-manifest.sh <version> <SHA256SUMS> <directory> [base-url]
#
# The three files go into <directory>/manifests/s/Stiven-Gjekaj/AllTheTools/
# <version>/, which is where winget-pkgs keeps them.

set -euo pipefail

USAGE="usage: winget-manifest.sh <version> <SHA256SUMS> <directory> [base-url]"
VERSION="${1:?$USAGE}"
SUMS="${2:?$USAGE}"
OUT="${3:?$USAGE}"
BASE="${4:-https://github.com/Stiven-Gjekaj/AllTheTools/releases/download/v${VERSION}}"
SCRIPT="winget-manifest.sh"
source "$(dirname "$0")/lib-digest.sh"

ID="Stiven-Gjekaj.AllTheTools"
SCHEMA="1.12.0"

# winget-pkgs writes a digest in capitals.
X64="$(digest "att-${VERSION}-x86_64-pc-windows-msvc.exe" | tr 'a-f' 'A-F')"
ARM64="$(digest "att-${VERSION}-aarch64-pc-windows-msvc.exe" | tr 'a-f' 'A-F')"

DIR="${OUT}/manifests/s/Stiven-Gjekaj/AllTheTools/${VERSION}"
mkdir -p "$DIR"

cat > "${DIR}/${ID}.yaml" <<YAML
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.version.${SCHEMA}.schema.json
PackageIdentifier: ${ID}
PackageVersion: ${VERSION}
DefaultLocale: en-US
ManifestType: version
ManifestVersion: ${SCHEMA}
YAML

cat > "${DIR}/${ID}.installer.yaml" <<YAML
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.installer.${SCHEMA}.schema.json
PackageIdentifier: ${ID}
PackageVersion: ${VERSION}
InstallerType: portable
Commands:
- att
Installers:
- Architecture: x64
  InstallerUrl: ${BASE}/att-${VERSION}-x86_64-pc-windows-msvc.exe
  InstallerSha256: ${X64}
- Architecture: arm64
  InstallerUrl: ${BASE}/att-${VERSION}-aarch64-pc-windows-msvc.exe
  InstallerSha256: ${ARM64}
ManifestType: installer
ManifestVersion: ${SCHEMA}
YAML

cat > "${DIR}/${ID}.locale.en-US.yaml" <<YAML
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.defaultLocale.${SCHEMA}.schema.json
PackageIdentifier: ${ID}
PackageVersion: ${VERSION}
PackageLocale: en-US
Publisher: Stiven Gjekaj
PublisherUrl: https://github.com/Stiven-Gjekaj
PackageName: AllTheTools
PackageUrl: https://github.com/Stiven-Gjekaj/AllTheTools
License: MIT
LicenseUrl: https://github.com/Stiven-Gjekaj/AllTheTools/blob/main/LICENSE
ShortDescription: ${DESCRIPTION}
Description: One command, att, that gives tools that Windows does not ship, with the same options on macOS and on Linux. It lists USB and PCI devices and drives, finds the real size of a fake flash drive, scans a drive for bad sectors, reads sensors and the battery, finds the process of a port or of a file, keeps the system awake, measures the network, and more.
Tags:
- cli
- lsusb
- lsblk
- f3probe
- badblocks
- timeout
ManifestType: defaultLocale
ManifestVersion: ${SCHEMA}
YAML

echo "$DIR"
