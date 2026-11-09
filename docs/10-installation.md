# Install and release Ivy

## First Linux alpha

The first packaging delivery targets **Ubuntu 24.04 and 26.04 on x86_64**. It builds a Debian
package without requiring Rust on the machine that installs it. Adding packaging to the repository
does not itself publish a download: use an explicitly published GitHub prerelease, or build the
package below. The alpha is experimental and unsigned.
Prereleases do not appear in the stable-only [in-app update checker](09-releases.md).

Download the `.deb`, `release-manifest.json` and `SHA256SUMS` from the same release into an empty
directory. Verify them there, then install the package shown in that release:

```sh
sha256sum --ignore-missing --check SHA256SUMS
sudo apt install ./ivy_0.1.0-alpha.2_amd64.deb
ivy
```

The selected `.deb` and `release-manifest.json` must both be downloaded and reported as `OK`.
Other package formats in the shared checksum list may be absent; `--ignore-missing` skips only
files you did not download. Do not install a package whose checksum failed.
The filename above illustrates workspace version `0.1.0-alpha.2`; use the actual release filename.
The prerelease separator becomes `~` in the Debian package version. Checksums detect changed
or mismatched downloads; they are not a publisher signature.

You can also open **Ivy** from the applications menu. The package installs the application and
MCP bridge under `/usr/lib/ivy-terminal/`, with `/usr/bin/ivy` and `/usr/bin/panel-mcp` as public
commands. Its desktop entry and icons are package-owned files. GPLv3, copyright and bundled third-party
notices are installed under `/usr/share/doc/ivy-terminal/`. It does not register itself as
your default terminal, change agent settings, or install agents for you.

To upgrade, close Ivy after saving your work and install the newer `.deb` in the same way. Existing
programs are not resumed on reopening: the saved layout starts fresh programs. Configuration and
layout remain in `$XDG_CONFIG_HOME/agent-terminal/`, or `~/.config/agent-terminal/` when the variable
is unset. The installer owns neither location. To remove the package:

```sh
sudo apt remove ivy-terminal
```

Removing the package preserves your configuration and layout. Delete those files yourself only
if you intend to reset Ivy. There is no APT repository or automatic updater in this delivery.

## Connect an MCP producer

Configure your agent's MCP client to launch **`/usr/bin/panel-mcp`**, then start the agent inside
Ivy so it inherits its console's identity and endpoint. The installed bridge replaces the
source-build path `target/release/panel-mcp` in the [README example](../README.md#connect-an-mcp-producer).
No provider API key is needed by this bridge. A client launched outside Ivy does not inherit a
console to control.

The terminal remains useful without MCP. A producer may publish
[`panel.json`](04-panel-contract.md) using the console's `PANEL_DIR` instead.

## Build a Linux package

Build on Ubuntu 24.04 to establish that runtime baseline; a binary compiled against newer system
libraries is not evidence that it can run on 24.04. The repository's pinned Rust toolchain and
lockfile are used for the binaries. Install a compiler/linker, Python 3.11 or later, and Debian
packaging tools:

```sh
sudo apt update
sudo apt install build-essential pkg-config libssl-dev libxkbcommon-dev dpkg-dev binutils desktop-file-utils python3
cargo build --locked --release -p app -p panel-control --bins
cargo metadata --locked --filter-platform x86_64-unknown-linux-gnu --format-version 1 > target/release-metadata.json
python3 scripts/release.py notices --metadata target/release-metadata.json --directory target/release/notices
version=$(python3 scripts/release.py version)
scripts/package-linux.sh --binary-dir target/release --output-dir dist --version "$version"
scripts/test-linux-package.sh dist/*.deb "$version" target/release/notices
python3 scripts/release.py generate --directory dist --commit "$(git rev-parse HEAD)" --built-on ubuntu-24.04
(cd dist && sha256sum --check SHA256SUMS)
```

These commands use Bash. `--built-on` must name the environment actually used to compile the
binaries; it is recorded, not inferred. Keep `dist/` limited to this release's package and metadata.
The package builder inspects the actual ELF binaries, computes their linked-library dependencies,
and declares the graphics/window/font libraries that are opened dynamically. A graphics-capable
X11 or Wayland desktop is still required to use the terminal.

Release versions use `MAJOR.MINOR.PATCH`, optionally followed by `-alpha.N`, `-beta.N` or `-rc.N`,
where `N` is a nonnegative integer without leading zeroes. Other prerelease labels and SemVer build
metadata (`+...`) are unsupported. This subset keeps SemVer and Debian ordering consistent:
`alpha.2` precedes `alpha.10`; labels such as `rc10` and `alpha01` would not preserve that ordering
or uniqueness under a direct Debian conversion.

## Isolated installation checks

`scripts/test-linux-package.sh` inspects an archive without changing the machine. It checks metadata,
executable architecture, launch commands, desktop entry, icons and absence of user-data ownership
or installer removal scripts. Python release tests check version validation, missing/extra artifacts
and hashes over the actual output bytes:

```sh
python3 -m unittest scripts/test_release.py -v
```

`packaging/linux/test-install.sh` is an **installation test for disposable containers only**. It
requires root and `IVY_DISPOSABLE_CONTAINER=yes`, installs an older fixture package, upgrades to
the candidate, removes it, and checks that user configuration/layout survive. Before removal, its runtime helper launches the
installed terminal under Xvfb and exercises the installed MCP bridge, including publication/readback
from that console. These synthetic checks remain headless evidence. Never run it as
an ordinary test on a workstation; use a clean Ubuntu 24.04 or 26.04 container with the package's
dependencies installed, plus Python 3, Xvfb, xauth and xdotool.

Archive inspection, container installation and headless application launches are separate from
real-desktop tests. They cannot prove desktop keyboard, clipboard, launcher icon association,
GPU rendering or restoration of the visible layout.

## Build artifacts and publishing

The **Ivy Linux package** GitHub Actions workflow builds on `ubuntu-24.04` for pull requests and
manual runs. Its default manual run only builds; it uploads the package, manifest and checksums
as a workflow artifact named `ivy-linux-x86_64`. Separate jobs test installation, upgrade and
removal in disposable Ubuntu 24.04 and 26.04 containers using that exact package. These checks
are separate from a public GitHub Release and from real-desktop validation.

For a release, approve the exact version, commit, target set and notes first. Commit the intended
workspace version and lockfile update, create its matching `v<VERSION>` tag, and run the workflow
on that tag with `publish=true`. The workflow rejects a tag/version mismatch, builds the declared
Linux package, requires both Ubuntu installation checks, and uses a dependent publication job. It creates a draft prerelease, uploads all
assets, then makes that complete draft visible. A failed build publishes nothing; a failed upload
leaves an unpublished draft to inspect. Existing release assets must not be overwritten silently.
Before creating the draft and before publishing it, the current remote tag must still resolve to
the approved commit; a moved or removed tag stops publication. If publication fails, inspect the
draft before retrying; there is no automatic replacement path.

The workflow uses read-only repository access for builds and write access only in its manual
publication job. It does not use signing credentials. Release metadata records the workspace
version, commit, exact asset name, architecture target, build environment, unsigned status and
SHA-256 digest. A full cross-platform release will declare each additional target when its own
packaging delivery is ready.

## Platform evidence

A package is not a claim that every platform behavior has been validated. Record each trial's
OS version, architecture, source revision, package SHA-256, commands and observations alongside
the release. Mark missing evidence explicitly; do not turn CI success into desktop proof.

| Consumer target | Packaging in this delivery | Required before a desktop-tested claim |
|---|---|---|
| Ubuntu 24.04 x86_64 | Linux `.deb` | Install, menu launch/icon, shell input/output, panes/tabs, copy/paste, settings/layout restoration, upgrade and removal |
| Ubuntu 26.04 x86_64 | Same baseline-built `.deb` | Same checks against the exact same package |
| macOS 15+ ARM64 and Intel | Planned next delivery | Native bundle/DMG, architecture checks and real desktop trials; signing status declared |
| Windows 11 x86_64 | Planned next delivery | Per-user installer, persistence and ordinary-user desktop trials |

This table defines the evidence still to collect, not completed tests. The release notes or work
validation artifact must record the actual results. macOS and Windows downloads are not supplied
by the Linux delivery. The Windows control channel and MCP are unavailable until F2.7 implements
and proves its native listener; merely packaging `panel-mcp` does not enable it. The portability
milestone remains open independently of installer work.

## Arch Linux (x86_64, rolling)

Download `ivy-terminal-0.1.0alpha2-1-x86_64.pkg.tar.zst`, `release-manifest.json` and `SHA256SUMS`
from the same prerelease into one directory, then verify and install:

```bash
sha256sum --ignore-missing --check SHA256SUMS
sudo pacman -U ./ivy-terminal-0.1.0alpha2-1-x86_64.pkg.tar.zst
```

The selected Arch package and `release-manifest.json` must both be reported as `OK`; an absent
Ubuntu package is allowed. Do not install after a failed checksum check.

Open Ivy from the application menu or run `ivy`. MCP uses the same `/usr/bin/panel-mcp` command as Ubuntu. A Rust compiler is not required on the tester machine. Update Arch normally with `sudo pacman -Syu` before installing; do not perform a partial system upgrade.

To remove the package, run `sudo pacman -R ivy-terminal`. Existing preferences and layouts are preserved. This is a direct experimental package download, not an AUR entry or an official Arch repository package. The package is unsigned; local package signature requirements still apply.

The Arch build uses native `makepkg` around the exact Linux binary/resource payload built on Ubuntu24.04. It supplies Arch dependency names independently; Debian dependency metadata is not reused. Both binaries, icons and original license notices are compared byte for byte. Arch's ordinary build user runs makepkg; only disposable-container installation tests run as root.

For a local package build, prepare the payload on the baseline build machine:

```bash
dpkg-deb -x dist/*.deb payload
tar -cf payload.tar -C payload usr
```

Then on Arch, as an ordinary user with base-devel and the dependencies in `packaging/arch/PKGBUILD` installed:

```bash
scripts/package-arch.sh /absolute/path/payload.tar dist 0.1.0-alpha.2
packaging/arch/test-package.sh dist/*.pkg.tar.zst /absolute/path/payload.tar 0.1.0alpha2
```

`packaging/arch/test-install.sh PACKAGE SEMVER` is destructive test machinery for a disposable Arch container only, guarded by root plus `IVY_DISPOSABLE_CONTAINER=yes`. It installs an older fixture, upgrades, exercises the installed Ivy/MCP own-pane roundtrip under Xvfb, removes the package and verifies unchanged config/layout bytes. It is never a workstation installation command.

CI builds the Ubuntu baseline, checks that artifact's checksums, packages and tests Arch independently, then uploads `ivy-arch-x86_64`. Publication requires both Ubuntu installation jobs and the Arch job. The final manifest/checksums require both package formats (`generate --include-arch`); partial target sets cannot be published.

Arch version mapping removes the SemVer hyphen and prerelease separator: `0.1.0-alpha.2` becomes `0.1.0alpha2-1`; stable versions remain unchanged. Native `vercmp` checks transitions through alpha, beta, rc and stable versions. Download names use safe SemVer characters; the Debian control version retains `~`, but its asset filename does not because GitHub rewrites that character during upload.

Current Arch evidence is container installation/runtime evidence. Real-desktop launcher, clipboard, fonts/themes and restored layout still need an owner trial with the exact package. macOS and Windows remain separate deliveries.
