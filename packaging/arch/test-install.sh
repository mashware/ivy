#!/usr/bin/env bash
# Run only in a disposable Arch container with desktop test dependencies installed.
set -euo pipefail
[[ $# == 2 && -f $1 ]] || { echo 'Usage: test-install.sh PACKAGE SEMVER' >&2; exit 2; }
[[ ${IVY_DISPOSABLE_CONTAINER:-} == yes && $(id -u) == 0 ]] || {
    echo 'Requires root in a disposable container and IVY_DISPOSABLE_CONTAINER=yes' >&2; exit 1;
}
package=$(realpath "$1")
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
config=${XDG_CONFIG_HOME:-$HOME/.config}/agent-terminal
[[ ! -e $config ]] || { echo 'Disposable test requires fresh configuration' >&2; exit 1; }
mkdir -p "$config" "$tmp/old"
printf 'fixture settings\n' > "$config/config"
printf 'fixture layout\n' > "$config/layout.json"
(cd "$config" && sha256sum config layout.json) > "$tmp/expected"
check_data() { (cd "$config" && sha256sum -c "$tmp/expected"); }
check_session_payload() {
    [[ $(readlink /usr/bin/ivy-session) == /usr/lib/ivy-terminal/ivy-session ]]
    [[ $(readlink /usr/bin/ivy-codex) == /usr/lib/ivy-terminal/ivy-codex ]]
    [[ -x /usr/bin/ivy-session ]]
    diff -r "$(dirname "$(realpath "$0")")/../../integrations/session-recovery" /usr/lib/ivy-terminal/session-recovery
}
bsdtar -xf "$package" -C "$tmp/old"
sed -i 's/^pkgver = .*/pkgver = 0-1/' "$tmp/old/.PKGINFO"
rm -f "$tmp/old/.MTREE"
bsdtar --zstd -cf "$tmp/old.pkg.tar.zst" -C "$tmp/old" .PKGINFO .BUILDINFO usr
pacman -U --noconfirm "$tmp/old.pkg.tar.zst"
check_data
check_session_payload
pacman -U --noconfirm "$package"
check_data
check_session_payload
for binary in /usr/bin/ivy /usr/bin/panel-mcp /usr/bin/ivy-session /usr/bin/ivy-codex; do
    [[ -x $binary ]]
    if ldd "$binary" | grep -q 'not found'; then echo "Unresolved library: $binary" >&2; exit 1; fi
done
IVY_EXPECTED_VERSION=$2 "$(dirname "$(realpath "$0")")/../linux/test-runtime.sh"
pacman -R --noconfirm ivy-terminal
check_data
[[ ! -e /usr/bin/ivy && ! -e /usr/bin/panel-mcp && ! -e /usr/bin/ivy-session && ! -e /usr/bin/ivy-codex ]]
[[ ! -e /usr/lib/ivy-terminal/session-recovery ]]
echo 'Arch install, upgrade, runtime and removal preserved user data.'
