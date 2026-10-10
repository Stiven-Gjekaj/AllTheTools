#!/usr/bin/env bash
# Write the Homebrew formula of one release, from the SHA256SUMS of that
# release.
#
# The formula installs the binary of the release, for macOS on each chip and
# for Linux on each architecture. It goes into the tap of the project, and a
# person commits it there: no bot pushes to the tap.
#
# Usage: scripts/homebrew-formula.sh <version> <SHA256SUMS> [base-url] > allthetools.rb

set -euo pipefail

USAGE="usage: homebrew-formula.sh <version> <SHA256SUMS> [base-url]"
VERSION="${1:?$USAGE}"
SUMS="${2:?$USAGE}"
BASE="${3:-https://github.com/Stiven-Gjekaj/AllTheTools/releases/download/v${VERSION}}"
SCRIPT="homebrew-formula.sh"
source "$(dirname "$0")/lib-digest.sh"

MAC_ARM="$(digest "att-${VERSION}-aarch64-apple-darwin")"
MAC_INTEL="$(digest "att-${VERSION}-x86_64-apple-darwin")"
LINUX_ARM="$(digest "att-${VERSION}-aarch64-unknown-linux-musl")"
LINUX_INTEL="$(digest "att-${VERSION}-x86_64-unknown-linux-musl")"

cat <<FORMULA
# scripts/homebrew-formula.sh in the AllTheTools repository writes this file
# from the SHA256SUMS of the release. Change the script, and not this file.
class Allthetools < Formula
  desc "${DESCRIPTION}"
  homepage "https://github.com/Stiven-Gjekaj/AllTheTools"
  license "MIT"

  on_macos do
    on_arm do
      url "${BASE}/att-${VERSION}-aarch64-apple-darwin"
      sha256 "${MAC_ARM}"
    end
    on_intel do
      url "${BASE}/att-${VERSION}-x86_64-apple-darwin"
      sha256 "${MAC_INTEL}"
    end
  end

  on_linux do
    on_arm do
      url "${BASE}/att-${VERSION}-aarch64-unknown-linux-musl"
      sha256 "${LINUX_ARM}"
    end
    on_intel do
      url "${BASE}/att-${VERSION}-x86_64-unknown-linux-musl"
      sha256 "${LINUX_INTEL}"
    end
  end

  def install
    binary = Dir["att-*"].first
    chmod 0755, binary
    bin.install binary => "att"
  end

  def caveats
    <<~EOS
      att link makes the familiar names, such as lsusb and f3probe, run att.
    EOS
  end

  test do
    assert_match "att #{version}", shell_output("#{bin}/att --version")
  end
end
FORMULA
