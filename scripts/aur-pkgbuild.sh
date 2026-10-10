#!/usr/bin/env bash
# Write the PKGBUILD and the .SRCINFO of the AUR package allthetools-bin, from
# the SHA256SUMS of one release.
#
# The package installs the musl binary of the release, which runs on each
# distribution, for x86_64 and for aarch64. A person commits the two files to
# the AUR repository of the package and pushes them: no bot pushes to the
# AUR.
#
# Usage: scripts/aur-pkgbuild.sh <version> <SHA256SUMS> <directory> [base-url]

set -euo pipefail

USAGE="usage: aur-pkgbuild.sh <version> <SHA256SUMS> <directory> [base-url]"
VERSION="${1:?$USAGE}"
SUMS="${2:?$USAGE}"
OUT="${3:?$USAGE}"
BASE="${4:-https://github.com/Stiven-Gjekaj/AllTheTools/releases/download/v${VERSION}}"
SCRIPT="aur-pkgbuild.sh"
source "$(dirname "$0")/lib-digest.sh"

X64="$(digest "att-${VERSION}-x86_64-unknown-linux-musl")"
ARM64="$(digest "att-${VERSION}-aarch64-unknown-linux-musl")"
# The licence comes from the tag of the release. This is its digest in the
# working tree, which is the tree of the tag when a person runs the script
# from a checkout of the tag.
LICENCE="$(shasum -a 256 "$(dirname "$0")/../LICENSE" 2>/dev/null | cut -d' ' -f1 \
    || sha256sum "$(dirname "$0")/../LICENSE" | cut -d' ' -f1)"
RAW="https://raw.githubusercontent.com/Stiven-Gjekaj/AllTheTools/v${VERSION}/LICENSE"

mkdir -p "$OUT"

cat > "${OUT}/PKGBUILD" <<PKGBUILD
# Maintainer: Stiven Gjekaj <stivenagostingjekaj at gmail dot com>
#
# scripts/aur-pkgbuild.sh in the AllTheTools repository writes this file
# from the SHA256SUMS of the release. Change the script, and not this file.

pkgname=allthetools-bin
pkgver=${VERSION}
pkgrel=1
pkgdesc="${DESCRIPTION}"
arch=('x86_64' 'aarch64')
url="https://github.com/Stiven-Gjekaj/AllTheTools"
license=('MIT')
provides=('allthetools')
conflicts=('allthetools')
source=("LICENSE-\${pkgver}::${RAW}")
source_x86_64=("att-\${pkgver}-x86_64::${BASE}/att-\${pkgver}-x86_64-unknown-linux-musl")
source_aarch64=("att-\${pkgver}-aarch64::${BASE}/att-\${pkgver}-aarch64-unknown-linux-musl")
sha256sums=('${LICENCE}')
sha256sums_x86_64=('${X64}')
sha256sums_aarch64=('${ARM64}')

package() {
    install -Dm755 "att-\${pkgver}-\${CARCH}" "\${pkgdir}/usr/bin/att"
    install -Dm644 "LICENSE-\${pkgver}" "\${pkgdir}/usr/share/licenses/\${pkgname}/LICENSE"
}
PKGBUILD

cat > "${OUT}/.SRCINFO" <<SRCINFO
pkgbase = allthetools-bin
	pkgdesc = ${DESCRIPTION}
	pkgver = ${VERSION}
	pkgrel = 1
	url = https://github.com/Stiven-Gjekaj/AllTheTools
	arch = x86_64
	arch = aarch64
	license = MIT
	provides = allthetools
	conflicts = allthetools
	source = LICENSE-${VERSION}::${RAW}
	sha256sums = ${LICENCE}
	source_x86_64 = att-${VERSION}-x86_64::${BASE}/att-${VERSION}-x86_64-unknown-linux-musl
	sha256sums_x86_64 = ${X64}
	source_aarch64 = att-${VERSION}-aarch64::${BASE}/att-${VERSION}-aarch64-unknown-linux-musl
	sha256sums_aarch64 = ${ARM64}

pkgname = allthetools-bin
SRCINFO

echo "$OUT"
