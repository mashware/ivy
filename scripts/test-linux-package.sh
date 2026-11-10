#!/usr/bin/env bash
# Inspect the exact package; optionally compare all prepared dependency notices byte for byte.
set -euo pipefail
[[ $# == 2 || $# == 3 ]] || { echo 'Usage: scripts/test-linux-package.sh PACKAGE VERSION [NOTICES_DIR]' >&2; exit 2; }
package=$(realpath "$1")
version=${2/-/\~}
[[ $(basename "$package") == "ivy_${2}_amd64.deb" ]]
[[ $(dpkg-deb -f "$package" Package) == ivy-terminal ]]
[[ $(dpkg-deb -f "$package" Version) == "$version" ]]
[[ $(dpkg-deb -f "$package" Architecture) == amd64 ]]
depends=$(dpkg-deb -f "$package" Depends)
[[ $depends == *libc6* && $depends == *libfontconfig1* && $depends == *libwayland-client0* ]]
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
dpkg-deb -x "$package" "$tmp/root"
dpkg-deb -e "$package" "$tmp/control"
for binary in app panel-mcp ivy-session ivy-codex; do
    [[ -x $tmp/root/usr/lib/ivy-terminal/$binary ]]
    readelf -h "$tmp/root/usr/lib/ivy-terminal/$binary" | grep -q 'Advanced Micro Devices X86-64'
done
[[ $(readlink "$tmp/root/usr/bin/ivy") == /usr/lib/ivy-terminal/app ]]
[[ $(readlink "$tmp/root/usr/bin/panel-mcp") == /usr/lib/ivy-terminal/panel-mcp ]]
[[ $(readlink "$tmp/root/usr/bin/ivy-session") == /usr/lib/ivy-terminal/ivy-session ]]
[[ $(readlink "$tmp/root/usr/bin/ivy-codex") == /usr/lib/ivy-terminal/ivy-codex ]]
repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
diff -r "$repo/integrations/session-recovery" "$tmp/root/usr/lib/ivy-terminal/session-recovery"
[[ -s $tmp/control/md5sums ]]
(cd "$tmp/root" && md5sum -c "$tmp/control/md5sums")
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
for file in "$doc"/cargo/epaint_default_fonts-*/fonts/{OFL.txt,UFL.txt,Hack-Regular.txt,emoji-icon-font-mit-license.txt}; do
    [[ -s $file ]]
done
if [[ $# == 3 ]]; then diff -r "$3" "$doc/cargo"; fi
# User data is never package-owned or removed by a maintainer script.
[[ ! -e $tmp/root/home && ! -e $tmp/root/root ]]
for script in preinst postinst prerm postrm; do [[ ! -e $tmp/control/$script ]]; done
echo "Package checks passed: $package"
