#!/usr/bin/env bash
# Run as an ordinary user in Arch, after preparing a checked Linux payload tar.
set -euo pipefail
[[ $# == 3 ]] || { echo 'Usage: package-arch.sh PAYLOAD.tar OUTPUT_DIR VERSION' >&2; exit 2; }
payload=$(realpath "$1")
output=$2 version=$3
[[ $version =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-(alpha|beta|rc)\.(0|[1-9][0-9]*))?$ ]] || {
    echo "Unsupported release version: $version" >&2; exit 1;
}
[[ $(id -u) != 0 ]] || { echo 'makepkg requires an ordinary build user' >&2; exit 1; }
[[ -f $payload ]] || { echo "Missing payload: $payload" >&2; exit 1; }
mkdir -p "$output"
output=$(realpath "$output")
repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
cp "$repo/packaging/arch/PKGBUILD" "$tmp/PKGBUILD"
cp "$payload" "$tmp/payload.tar"
IVY_ARCH_VERSION=${version/-/}
IVY_ARCH_VERSION=${IVY_ARCH_VERSION/alpha./alpha}
IVY_ARCH_VERSION=${IVY_ARCH_VERSION/beta./beta}
IVY_ARCH_VERSION=${IVY_ARCH_VERSION/rc./rc}
IVY_PAYLOAD_SHA256=$(sha256sum "$tmp/payload.tar")
IVY_PAYLOAD_SHA256=${IVY_PAYLOAD_SHA256%% *}
export IVY_ARCH_VERSION IVY_PAYLOAD_SHA256
(cd "$tmp" && PKGDEST="$output" PKGEXT='.pkg.tar.zst' makepkg --noconfirm --clean)
