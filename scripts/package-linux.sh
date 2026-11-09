#!/usr/bin/env bash
# Build the Ubuntu amd64 package from already-built release binaries.
set -euo pipefail

usage() {
    echo 'Usage: scripts/package-linux.sh --binary-dir DIR --output-dir DIR --version SEMVER' >&2
    exit 2
}
binary_dir='' output_dir='' version=''
export LC_ALL=C
while (($#)); do
    (($# >= 2)) || usage
    case "$1" in
        --binary-dir) binary_dir=$2 ;;
        --output-dir) output_dir=$2 ;;
        --version) version=$2 ;;
        *) usage ;;
    esac
    shift 2
done
[[ -n $binary_dir && -n $output_dir && -n $version ]] || usage
# Numbered prereleases preserve both SemVer and Debian ordering.
[[ $version =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-(alpha|beta|rc)\.(0|[1-9][0-9]*))?$ ]] || {
    echo "Unsupported release version: $version" >&2; exit 1;
}
deb_version=${version/-/\~}
dpkg --validate-version "$deb_version"
repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
binary_dir=$(cd "$binary_dir" && pwd)
mkdir -p "$output_dir"
output_dir=$(cd "$output_dir" && pwd)
for binary in app panel-mcp ivy-session; do
    path=$binary_dir/$binary
    [[ -f $path && -x $path ]] || { echo "Missing executable: $path" >&2; exit 1; }
    header=$(readelf -h "$path") || { echo "Not an ELF binary: $path" >&2; exit 1; }
    if ! grep -Eq 'Class:[[:space:]]+ELF64' <<<"$header" ||
        ! grep -Eq 'Machine:[[:space:]]+Advanced Micro Devices X86-64' <<<"$header"; then
        echo "Expected an x86-64 ELF binary: $path" >&2
        exit 1
    fi
done
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/debian" "$stage/root/DEBIAN" "$stage/root/usr/lib/ivy-terminal" \
    "$stage/root/usr/bin" "$stage/root/usr/share/applications" \
    "$stage/root/usr/share/icons/hicolor/512x512/apps" \
    "$stage/root/usr/share/icons/hicolor/scalable/apps"
install -m 755 "$binary_dir/app" "$binary_dir/panel-mcp" "$binary_dir/ivy-session" "$stage/root/usr/lib/ivy-terminal/"
cp -R "$repo/integrations/session-recovery" "$stage/root/usr/lib/ivy-terminal/session-recovery"
ln -s /usr/lib/ivy-terminal/app "$stage/root/usr/bin/ivy"
ln -s /usr/lib/ivy-terminal/panel-mcp "$stage/root/usr/bin/panel-mcp"
ln -s /usr/lib/ivy-terminal/ivy-session "$stage/root/usr/bin/ivy-session"
install -m 644 "$repo/packaging/linux/Ivy.desktop" "$stage/root/usr/share/applications/"
install -m 644 "$repo/resources/branding/ivy.png" "$stage/root/usr/share/icons/hicolor/512x512/apps/ivy.png"
install -m 644 "$repo/resources/branding/ivy.svg" "$stage/root/usr/share/icons/hicolor/scalable/apps/ivy.svg"
# Preserve upstream texts and attribution alongside the installed binaries.
doc="$stage/root/usr/share/doc/ivy-terminal"
[[ -d $binary_dir/notices ]] && [[ -n $(ls -A "$binary_dir/notices") ]] || {
    echo "Missing prepared dependency notices: $binary_dir/notices" >&2; exit 1;
}
mkdir -p "$doc/vendor/egui-winit" "$doc/resources/themes" "$doc/cargo"
install -m 644 "$repo/LICENSE" "$repo/COPYRIGHT" "$repo/THIRD-PARTY.md" "$doc/"
install -m 644 "$repo/vendor/egui-winit/LICENSE-MIT" "$repo/vendor/egui-winit/LICENSE-APACHE" "$doc/vendor/egui-winit/"
install -m 644 "$repo/resources/themes/README.md" "$repo/resources/themes/"*-LICENSE "$doc/resources/themes/"
cp -R "$binary_dir/notices/." "$doc/cargo/"
# dpkg-shlibdeps reads the source package's control file in its working directory.
cat > "$stage/debian/control" <<'CONTROL'
Source: ivy-terminal
Section: utils
Priority: optional
Maintainer: Alberto Vioque <mashware@gmail.com>

Package: ivy-terminal
Architecture: amd64
Description: Terminal for hosting agents
CONTROL
shlibs=$(cd "$stage" && dpkg-shlibdeps -O -e root/usr/lib/ivy-terminal/app -e root/usr/lib/ivy-terminal/panel-mcp -e root/usr/lib/ivy-terminal/ivy-session)
shlibs=${shlibs#shlibs:Depends=}
[[ -n $shlibs ]] || { echo 'Could not determine shared library dependencies' >&2; exit 1; }
# Window-system, graphics and font libraries are opened at runtime and therefore
# do not all appear in ELF DT_NEEDED. Ubuntu supplies these public runtime packages.
runtime='libfontconfig1, libx11-6, libx11-xcb1, libxcb1, libxcursor1, libxi6, libxrandr2, libxkbcommon0, libxkbcommon-x11-0, libwayland-client0, libwayland-cursor0, libwayland-egl1, libegl1, libgl1'
cat > "$stage/root/DEBIAN/control" <<CONTROL
Package: ivy-terminal
Version: $deb_version
Section: utils
Priority: optional
Architecture: amd64
Maintainer: Alberto Vioque <mashware@gmail.com>
Depends: $shlibs, $runtime
Description: Terminal for hosting agents
 Ivy hosts independent consoles with a status panel for each one.
CONTROL
# Include helper and adapter bytes in the installed package integrity checks.
(cd "$stage/root" && find usr -type f -print0 | sort -z | xargs -0 md5sum) > "$stage/root/DEBIAN/md5sums"
# SOURCE_DATE_EPOCH, when supplied by the build, fixes package entry timestamps.
if [[ -n ${SOURCE_DATE_EPOCH:-} ]]; then
    find "$stage/root" -exec touch -h -d "@$SOURCE_DATE_EPOCH" {} +
fi
artifact=$output_dir/ivy_${version}_amd64.deb
dpkg-deb --root-owner-group --build "$stage/root" "$artifact"
