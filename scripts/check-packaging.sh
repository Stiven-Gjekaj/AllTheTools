#!/usr/bin/env bash
# Run each package script on a SHA256SUMS that this script makes, and check
# the shape of what each one writes. The digests are made up, and nothing is
# downloaded or published.
#
# Usage: scripts/check-packaging.sh

set -euo pipefail

cd "$(dirname "$0")/.."
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

version="9.8.7"
for file in \
    "att-${version}-x86_64-unknown-linux-musl" \
    "att-${version}-aarch64-unknown-linux-musl" \
    "att-${version}-x86_64-apple-darwin" \
    "att-${version}-aarch64-apple-darwin" \
    "att-${version}-x86_64-pc-windows-msvc.exe" \
    "att-${version}-aarch64-pc-windows-msvc.exe"; do
    printf '%064x  %s\n' "${#file}" "$file" >> "$work/SHA256SUMS"
done

scripts/homebrew-formula.sh "$version" "$work/SHA256SUMS" > "$work/allthetools.rb"
grep -q 'class Allthetools < Formula' "$work/allthetools.rb"
test "$(grep -c 'sha256 "' "$work/allthetools.rb")" = 4
if command -v ruby >/dev/null 2>&1; then
    ruby -c "$work/allthetools.rb" > /dev/null
fi

scripts/scoop-manifest.sh "$version" "$work/SHA256SUMS" > "$work/allthetools.json"
python3 -c 'import json, sys; m = json.load(open(sys.argv[1])); assert m["version"] == sys.argv[2]; assert m["bin"] == "att.exe"' \
    "$work/allthetools.json" "$version"

dir="$(scripts/winget-manifest.sh "$version" "$work/SHA256SUMS" "$work/winget")"
test "$(ls "$dir" | wc -l | tr -d ' ')" = 3
grep -q "PackageVersion: ${version}" "$dir/Stiven-Gjekaj.AllTheTools.installer.yaml"

scripts/aur-pkgbuild.sh "$version" "$work/SHA256SUMS" "$work/aur" > /dev/null
bash -n "$work/aur/PKGBUILD"
grep -q "pkgver = ${version}" "$work/aur/.SRCINFO"

# A SHA256SUMS that lacks a file stops each script, with the name of the file.
grep -v 'apple-darwin' "$work/SHA256SUMS" > "$work/SHORT"
if scripts/homebrew-formula.sh "$version" "$work/SHORT" > /dev/null 2> "$work/error"; then
    echo "homebrew-formula.sh took a SHA256SUMS with no macOS binary" >&2
    exit 1
fi
grep -q "apple-darwin" "$work/error"

echo "Each package script wrote a file of the right shape."
