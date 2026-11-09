#!/usr/bin/env bash
# Check the native archive and compare the shared payload without installing it.
set -euo pipefail
if [[ $# == 1 && $1 == --versions ]]; then
    previous=
    for version in 0.1.0alpha0 0.1.0alpha1 0.1.0alpha2 0.1.0alpha10 \
        0.1.0beta0 0.1.0beta2 0.1.0rc0 0.1.0rc2 0.1.0rc10 0.1.0 0.1.1alpha0 0.1.1; do
        if [[ -n $previous ]]; then (( $(vercmp "$previous" "$version") < 0 )); fi
        previous=$version
    done
    echo 'Arch prerelease upgrade ordering passed.'
    exit
fi
[[ $# == 3 ]] || { echo 'Usage: test-package.sh PACKAGE PAYLOAD.tar ARCH_VERSION' >&2; exit 2; }
package=$(realpath "$1") payload=$(realpath "$2") version=$3
[[ $(basename "$package") == "ivy-terminal-$version-1-x86_64.pkg.tar.zst" ]]
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir "$tmp/package" "$tmp/payload"
bsdtar -xf "$package" -C "$tmp/package"
bsdtar -xf "$payload" -C "$tmp/payload"
grep -qx 'pkgname = ivy-terminal' "$tmp/package/.PKGINFO"
grep -Fqx "pkgver = $version-1" "$tmp/package/.PKGINFO"
grep -qx 'arch = x86_64' "$tmp/package/.PKGINFO"
[[ ! -e $tmp/package/.INSTALL && ! -e $tmp/package/home && ! -e $tmp/package/root ]]
for binary in app panel-mcp ivy-session; do
    [[ -x $tmp/package/usr/lib/ivy-terminal/$binary ]]
    cmp "$tmp/payload/usr/lib/ivy-terminal/$binary" "$tmp/package/usr/lib/ivy-terminal/$binary"
done
diff -r "$tmp/payload/usr/lib/ivy-terminal/session-recovery" "$tmp/package/usr/lib/ivy-terminal/session-recovery"
diff -r "$(dirname "$(realpath "$0")")/../../integrations/session-recovery" "$tmp/package/usr/lib/ivy-terminal/session-recovery"
diff -r "$tmp/payload/usr/share/doc/ivy-terminal" "$tmp/package/usr/share/doc/ivy-terminal"
cmp "$tmp/payload/usr/share/doc/ivy-terminal/LICENSE" "$tmp/package/usr/share/licenses/ivy-terminal/LICENSE"
[[ $(readlink "$tmp/package/usr/bin/ivy") == /usr/lib/ivy-terminal/app ]]
[[ $(readlink "$tmp/package/usr/bin/panel-mcp") == /usr/lib/ivy-terminal/panel-mcp ]]
[[ $(readlink "$tmp/package/usr/bin/ivy-session") == /usr/lib/ivy-terminal/ivy-session ]]
desktop-file-validate "$tmp/package/usr/share/applications/Ivy.desktop"
cmp "$tmp/payload/usr/share/applications/Ivy.desktop" "$tmp/package/usr/share/applications/Ivy.desktop"
diff -r "$tmp/payload/usr/share/icons" "$tmp/package/usr/share/icons"
echo 'Arch package metadata, executables and original notices passed.'
