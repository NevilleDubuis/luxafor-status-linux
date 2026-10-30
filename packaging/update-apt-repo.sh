#!/usr/bin/env bash
# Adds a .deb to the apt repository published on GitHub Pages, and signs it.
#
# Usage: packaging/update-apt-repo.sh REPO_DIR DEB [KEY_ID]
#   REPO_DIR  checkout of the gh-pages branch (created if empty)
#   DEB       the package to add
#   KEY_ID    GPG key to sign with (default: the first secret key)
#
# Suites: "stable" gets releases (vX.Y.Z), "testing" gets every version,
# pre-releases included. Packages are architecture-independent ("all").
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO="${1:?usage: update-apt-repo.sh REPO_DIR DEB [KEY_ID]}"
DEB="${2:?usage: update-apt-repo.sh REPO_DIR DEB [KEY_ID]}"
DEB="$(realpath "$DEB")"
mkdir -p "$REPO"
REPO="$(realpath "$REPO")"
KEY="${3:-$(gpg --list-secret-keys --with-colons | awk -F: '$1 == "fpr" { print $10; exit }')}"
[ -n "$KEY" ] || { echo "No GPG secret key to sign with" >&2; exit 1; }
ARCHES="amd64 arm64 armhf i386"
URL="https://nevilledubuis.github.io/luxafor-status-linux"

version="$(dpkg-deb -f "$DEB" Version)"
suites=(testing)
[[ "$version" == *"~"* ]] || suites+=(stable)

cd "$REPO"
touch .nojekyll   # serve files as they are
gpg --armor --export "$KEY" > key.asc

for suite in "${suites[@]}"; do
    mkdir -p "pool/$suite"
    cp "$DEB" "pool/$suite/"
done

for suite in stable testing; do
    [ -d "pool/$suite" ] || continue
    dists="dists/$suite"
    rm -rf "$dists"
    for arch in all $ARCHES; do
        mkdir -p "$dists/main/binary-$arch"
        apt-ftparchive packages "pool/$suite" > "$dists/main/binary-$arch/Packages"
        gzip -9nk "$dists/main/binary-$arch/Packages"
    done
    apt-ftparchive \
        -o APT::FTPArchive::Release::Origin="luxafor-status" \
        -o APT::FTPArchive::Release::Label="luxafor-status" \
        -o APT::FTPArchive::Release::Suite="$suite" \
        -o APT::FTPArchive::Release::Codename="$suite" \
        -o APT::FTPArchive::Release::Architectures="all $ARCHES" \
        -o APT::FTPArchive::Release::Components="main" \
        -o APT::FTPArchive::Release::Description="luxafor-status ($suite)" \
        release "$dists" > "$dists/Release.tmp"
    mv "$dists/Release.tmp" "$dists/Release"
    gpg --batch --yes --local-user "$KEY" --clearsign -o "$dists/InRelease" "$dists/Release"
    gpg --batch --yes --local-user "$KEY" --armor --detach-sign -o "$dists/Release.gpg" "$dists/Release"
done

# A page with the instructions, for people opening the URL in a browser.
sed -e "s|@URL@|$URL|g" -e "s|@VERSION@|$version|g" "$ROOT/packaging/apt-repo-index.html" > index.html
echo "Added luxafor-status $version to: ${suites[*]}"
