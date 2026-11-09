#!/usr/bin/env bash
# Inspect the exact package without installing it. Pass its path and the release version.
set -euo pipefail
[[ $# == 2 ]] || { echo 'Usage: scripts/test-linux-package.sh PACKAGE VERSION' >&2; exit 2; }
package=$(realpath "$1")
version=${2/-/\~}
[[ $(basename "$package") == "ivy_${version}_amd64.deb" ]]
[[ $(dpkg-deb -f "$package" Package) == ivy-terminal ]]
[[ $(dpkg-deb -f "$package" Version) == "$version" ]]
[[ $(dpkg-deb -f "$package" Architecture) == amd64 ]]
depends=$(dpkg-deb -f "$package" Depends)
[[ $depends == *libc6* && $depends == *libfontconfig1* && $depends == *libwayland-client0* ]]
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
dpkg-deb -x "$package" "$tmp/root"
dpkg-deb -e "$package" "$tmp/control"
for binary in app panel-mcp; do
    [[ -x $tmp/root/usr/lib/ivy-terminal/$binary ]]
    readelf -h "$tmp/root/usr/lib/ivy-terminal/$binary" | grep -q 'Advanced Micro Devices X86-64'
done
[[ $(readlink "$tmp/root/usr/bin/ivy") == /usr/lib/ivy-terminal/app ]]
[[ $(readlink "$tmp/root/usr/bin/panel-mcp") == /usr/lib/ivy-terminal/panel-mcp ]]
desktop=$tmp/root/usr/share/applications/Ivy.desktop
[[ -f $desktop ]]
grep -qx 'Name=Ivy' "$desktop"
grep -qx 'Exec=ivy' "$desktop"
grep -qx 'Icon=ivy' "$desktop"
if command -v desktop-file-validate >/dev/null; then desktop-file-validate "$desktop"; fi
[[ -s $tmp/root/usr/share/icons/hicolor/512x512/apps/ivy.png ]]
[[ -s $tmp/root/usr/share/icons/hicolor/scalable/apps/ivy.svg ]]
doc=$tmp/root/usr/share/doc/ivy-terminal
for file in LICENSE COPYRIGHT THIRD-PARTY.md vendor/egui-winit/LICENSE-MIT \
    vendor/egui-winit/LICENSE-APACHE resources/themes/README.md \
    resources/themes/catppuccin-LICENSE resources/themes/github-LICENSE \
    resources/themes/nord-LICENSE resources/themes/rose-pine-LICENSE; do
    [[ -s $doc/$file ]]
done
[[ -d $doc/cargo && -n $(ls -A "$doc/cargo") ]]
# User data is never package-owned or removed by a maintainer script.
[[ ! -e $tmp/root/home && ! -e $tmp/root/root ]]
for script in preinst postinst prerm postrm; do [[ ! -e $tmp/control/$script ]]; done
echo "Package checks passed: $package"
