#!/usr/bin/env bash
# Run only inside a disposable container, with package dependencies already installed.
set -euo pipefail
[[ $# == 1 && -f $1 ]] || { echo 'Usage: test-install.sh PACKAGE (inside a disposable container)' >&2; exit 2; }
[[ ${IVY_DISPOSABLE_CONTAINER:-} == yes && $(id -u) == 0 ]] || {
    echo 'Requires root in a disposable container and IVY_DISPOSABLE_CONTAINER=yes' >&2; exit 1;
}
package=$(realpath "$1")
[[ $(dpkg-deb -f "$package" Package) == ivy-terminal ]]
version=$(dpkg-deb -f "$package" Version)
# Version 0~fixture sorts before every valid release version.
dpkg --compare-versions 0~fixture lt "$version"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/old/DEBIAN"
dpkg-deb -x "$package" "$tmp/old"
dpkg-deb -e "$package" "$tmp/old/DEBIAN"
sed -i 's/^Version:.*/Version: 0~fixture/' "$tmp/old/DEBIAN/control"
dpkg-deb --root-owner-group --build "$tmp/old" "$tmp/old.deb" >/dev/null
config=${XDG_CONFIG_HOME:-$HOME/.config}/agent-terminal
[[ ! -e $config ]] || { echo 'Disposable test requires a fresh Ivy configuration directory' >&2; exit 1; }
mkdir -p "$config"
printf 'fixture settings\n' > "$config/config"
printf 'fixture layout\n' > "$config/layout.json"
(cd "$config" && sha256sum config layout.json) > "$tmp/expected"
check_data() { (cd "$config" && sha256sum -c "$tmp/expected"); }
check_commands() {
    [[ $(readlink -f /usr/bin/ivy) == /usr/lib/ivy-terminal/app ]]
    [[ $(readlink -f /usr/bin/panel-mcp) == /usr/lib/ivy-terminal/panel-mcp ]]
    [[ $(readlink -f /usr/bin/ivy-session) == /usr/lib/ivy-terminal/ivy-session ]]
    [[ -x /usr/bin/ivy && -x /usr/bin/panel-mcp && -x /usr/bin/ivy-session ]]
    diff -r "$(dirname "$(realpath "$0")")/../../integrations/session-recovery" /usr/lib/ivy-terminal/session-recovery
    [[ -z $(dpkg -V ivy-terminal) ]]
}
dpkg -i "$tmp/old.deb"
check_data
check_commands
dpkg -i "$package"
[[ $(dpkg-query -W -f='${Version}' ivy-terminal) == "$version" ]]
check_data
check_commands
"$(dirname "$(realpath "$0")")/test-runtime.sh"
dpkg -r ivy-terminal
check_data
[[ ! -e /usr/bin/ivy && ! -e /usr/bin/panel-mcp && ! -e /usr/bin/ivy-session ]]
[[ ! -e /usr/lib/ivy-terminal/session-recovery ]]
echo 'Install, upgrade and removal preserved user data.'
