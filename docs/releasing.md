<div align="center">
  <a href="../README.md"><b>AllTheTools</b></a>
</div>

# Releasing

A release is a decision that a person makes. The workflow does the rest, and
it does nothing until a tag tells it to. Each package manager gets the release
from a person, and no bot pushes to a tap, a bucket or the AUR.

## Before a release

Every item has to be true, and not only probably true.

- CI is green on the commit that the tag will name, on the three hosts. That
  includes the job that checks the package scripts.
- The version in the root `Cargo.toml` is the version of the release.
- [CHANGELOG.md](../CHANGELOG.md) has a section for the version.

## Cut a release

1. Set `version` under `[workspace.package]` in the root `Cargo.toml`, and
   run `cargo check` so `Cargo.lock` follows. Commit both. The commit subject
   names no version number.
2. Push, and wait for CI to pass on that commit.
3. Tag the commit and push the tag:

       git tag v0.1.0
       git push origin v0.1.0

4. The [release workflow](../.github/workflows/release.yml) builds six
   binaries, makes a `.deb` and a `.rpm` for each Linux architecture, hashes
   each file, attests each one, and opens a **draft** release. It refuses a
   tag that disagrees with the version in `Cargo.toml`, because that binary
   would name a release that it does not belong to.
5. Open the draft. Read the list of assets: six binaries, two `.deb`, two
   `.rpm` and `SHA256SUMS`. Publish it.
6. Get the checksums of the release:

       gh release download v0.1.0 --pattern SHA256SUMS --dir release

Then give the release to each package manager. Each step writes the files from
`SHA256SUMS` with a script of this repository, so no person copies a digest by
hand.

### Homebrew

Write the formula into the tap,
[Stiven-Gjekaj/homebrew-tap](https://github.com/Stiven-Gjekaj/homebrew-tap),
and commit it there:

    scripts/homebrew-formula.sh 0.1.0 release/SHA256SUMS > ../homebrew-tap/Formula/allthetools.rb

Check it the way a user installs it:

    brew install stiven-gjekaj/tap/allthetools
    att --version

### Scoop

Write the manifest into the bucket,
[Stiven-Gjekaj/scoop-bucket](https://github.com/Stiven-Gjekaj/scoop-bucket),
and commit it there:

    scripts/scoop-manifest.sh 0.1.0 release/SHA256SUMS > ../scoop-bucket/bucket/allthetools.json

### winget

Write the manifests into a branch of the fork
[Stiven-Gjekaj/winget-pkgs](https://github.com/Stiven-Gjekaj/winget-pkgs),
check them on Windows, and open a pull request to
[microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs) with them.
The repository holds hundreds of thousands of files, so sync the fork and clone
only the folder of the publisher:

    gh repo sync Stiven-Gjekaj/winget-pkgs
    git clone --depth 1 --filter=blob:none --sparse https://github.com/Stiven-Gjekaj/winget-pkgs.git ../winget-pkgs
    git -C ../winget-pkgs sparse-checkout set manifests/s/Stiven-Gjekaj
    git -C ../winget-pkgs checkout -b Stiven-Gjekaj.AllTheTools-0.1.0
    scripts/winget-manifest.sh 0.1.0 release/SHA256SUMS ../winget-pkgs
    winget validate --manifest ..\winget-pkgs\manifests\s\Stiven-Gjekaj\AllTheTools\0.1.0

The title of the pull request is `New package: Stiven-Gjekaj.AllTheTools
version 0.1.0` for the first version, and
`Update: Stiven-Gjekaj.AllTheTools to <version>` for each later one. Microsoft
checks the pull request, and a bot merges it.

### The AUR

The package is `allthetools-bin`, and it installs the musl binary of the
release. The AUR keeps each package in a Git repository of its own, and a push
needs the SSH key of the account of the maintainer on aur.archlinux.org.

From a checkout of the tag, so the licence matches the tag:

    git clone ssh://aur@aur.archlinux.org/allthetools-bin.git ../aur-allthetools-bin
    scripts/aur-pkgbuild.sh 0.1.0 release/SHA256SUMS ../aur-allthetools-bin

Check it on Arch Linux, where `makepkg` exists, and then commit and push:

    cd ../aur-allthetools-bin
    makepkg --printsrcinfo | diff - .SRCINFO
    makepkg -si
    git add PKGBUILD .SRCINFO
    git commit -m "Update to 0.1.0"
    git push

The first push creates the package on the AUR.

### Debian and RPM

The `.deb` and the `.rpm` are assets of the release, and the
[README](../README.md#install) says how to install them. No repository of a
distribution holds them yet.

## Test the workflow without a release

    gh workflow run release.yml

A run started by hand builds all six binaries, makes the four packages and
hashes them, and it publishes nothing. It makes no attestation either, because
on a public repository an attestation goes into the public Sigstore log. The
files wait in the run as an artifact named `dist`:

    gh run download <run-id> --name dist

## Check a download

Two checks, and they answer different questions.

The checksum says that the file arrived whole:

    shasum -a 256 -c SHA256SUMS --ignore-missing

The attestation says that this file came out of this repository, from the
commit that the tag names, through the release workflow:

    gh attestation verify att-0.1.0-aarch64-apple-darwin --repo Stiven-Gjekaj/AllTheTools

| Asset | Host |
| --- | --- |
| `att-<version>-x86_64-unknown-linux-musl` | Linux on x86_64, any distribution |
| `att-<version>-aarch64-unknown-linux-musl` | Linux on arm64, any distribution |
| `att-<version>-x86_64-apple-darwin` | macOS on Intel |
| `att-<version>-aarch64-apple-darwin` | macOS on Apple silicon |
| `att-<version>-x86_64-pc-windows-msvc.exe` | Windows on x86_64 |
| `att-<version>-aarch64-pc-windows-msvc.exe` | Windows on arm64 |
| `allthetools_<version>-1_amd64.deb`, `allthetools_<version>-1_arm64.deb` | Debian, Ubuntu |
| `allthetools-<version>-1.x86_64.rpm`, `allthetools-<version>-1.aarch64.rpm` | Fedora, openSUSE |
