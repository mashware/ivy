import hashlib
import json
import shutil
from pathlib import Path
from runpy import run_path
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).with_name("release.py")


class ReleaseTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.cargo = self.root / "Cargo.toml"
        self.cargo.write_text('[workspace.package]\nversion = "0.1.0-alpha.1"\n')
        self.package = self.root / "ivy_0.1.0-alpha.1_amd64.deb"
        self.package.write_bytes(b"actual package contents")

    def run_release(self, *extra):
        return subprocess.run(["python3", str(SCRIPT), "generate", "--cargo", str(self.cargo),
                               "--directory", str(self.root), "--commit", "a" * 40,
                               "--built-on", "test-builder", *extra],
                              capture_output=True, text=True)

    def test_notices_preserve_nested_originals_and_reject_missing(self):
        source = self.root / "source"
        (source / "fonts").mkdir(parents=True)
        (source / "Cargo.toml").touch()
        (source / "LICENSE-MIT").write_bytes(b"original license\r\n")
        (source / "fonts" / "OFL.txt").write_bytes(b"font terms\n")
        (source / "special.txt").write_bytes(b"declared license\n")
        packages = [
            {"id": name, "name": name, "source": None} for name in ("app", "panel-control")
        ] + [{"id": "dep", "name": "dep", "version": "1.0.0", "source": "registry+x",
              "manifest_path": str(source / "Cargo.toml"), "license_file": "special.txt"}]
        metadata = {"packages": packages, "workspace_members": ["app", "panel-control"],
                    "resolve": {"nodes": [
                        {"id": "app", "deps": [{"pkg": "dep", "dep_kinds": [{"kind": None}]}]},
                        {"id": "panel-control", "deps": []}, {"id": "dep", "deps": []}]}}
        path = self.root / "metadata.json"
        path.write_text(json.dumps(metadata))
        output = self.root / "notices"
        command = ["python3", str(SCRIPT), "notices", "--metadata", str(path), "--directory", str(output)]
        result = subprocess.run(command, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        for name in ("LICENSE-MIT", "fonts/OFL.txt", "special.txt"):
            self.assertEqual((output / "dep-1.0.0" / name).read_bytes(), (source / name).read_bytes())
        for name in ("LICENSE-MIT", "fonts/OFL.txt", "special.txt"):
            (source / name).unlink()
        result = subprocess.run(command[:-1] + [str(self.root / "missing")], capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing notice for dep 1.0.0", result.stderr)

    def test_notices_use_explicit_apache_fallback_and_keep_original_notice(self):
        source = self.root / "fallback-source"
        source.mkdir()
        (source / "Cargo.toml").touch()
        (source / "NOTICE").write_bytes(b"upstream notice\r\n")
        package = {"id": "dep", "name": "dep", "version": "2.0.0", "source": "registry+x",
                   "manifest_path": str(source / "Cargo.toml"), "license": "MIT OR Apache-2.0",
                   "repository": "https://example.test/dep", "authors": ["Author"]}
        packages = [{"id": name, "name": name, "source": None} for name in ("app", "panel-control")] + [package]
        metadata = {"packages": packages, "workspace_members": ["app", "panel-control"],
                    "resolve": {"nodes": [
                        {"id": "app", "deps": [{"pkg": "dep", "dep_kinds": [{"kind": None}]}]},
                        {"id": "panel-control", "deps": []}, {"id": "dep", "deps": []}]}}
        path = self.root / "metadata.json"
        path.write_text(json.dumps(metadata))
        output = self.root / "notices"
        command = ["python3", str(SCRIPT), "notices", "--metadata", str(path), "--directory", str(output)]
        result = subprocess.run(command, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        dest = output / "dep-2.0.0"
        self.assertEqual((dest / "NOTICE").read_bytes(), b"upstream notice\r\n")
        canonical = SCRIPT.parent.parent / "vendor/egui-winit/LICENSE-APACHE"
        self.assertEqual((dest / "SPDX-FALLBACK-APACHE-2.0").read_bytes(), canonical.read_bytes())
        attribution = (dest / "SPDX-FALLBACK-ATTRIBUTION.txt").read_text()
        for value in ("dep", "2.0.0", "MIT OR Apache-2.0", "https://example.test/dep", "Author"):
            self.assertIn(value, attribution)
        package["license"] = "Unrecognized"
        path.write_text(json.dumps(metadata))
        result = subprocess.run(command[:-1] + [str(self.root / "unknown")], capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing notice for dep 2.0.0", result.stderr)

    def test_manifest_and_checksums_cover_actual_bytes(self):
        self.assertEqual(self.run_release("--tag", "v0.1.0-alpha.1").returncode, 0)
        manifest = json.loads((self.root / "release-manifest.json").read_text())
        self.assertEqual(manifest["version"], "0.1.0-alpha.1")
        self.assertEqual(manifest["commit"], "a" * 40)
        self.assertEqual(manifest["artifacts"][0]["built_on"], "test-builder")
        self.assertEqual(manifest["artifacts"][0]["sha256"], hashlib.sha256(self.package.read_bytes()).hexdigest())
        lines = (self.root / "SHA256SUMS").read_text().splitlines()
        self.assertEqual(len(lines), 2)
        for line in lines:
            checksum, name = line.split("  ")
            self.assertEqual(checksum, hashlib.sha256((self.root / name).read_bytes()).hexdigest())
        self.package.write_bytes(b"tampered")
        self.assertNotEqual(lines[0].split("  ")[0], hashlib.sha256(self.package.read_bytes()).hexdigest())

    def test_verified_mac_notice_fallback_preserves_original_text_and_rejects_new_versions(self):
        source = self.root / "mac-source"
        source.mkdir()
        original = b'[package]\nname="block2"\nlicense="MIT"\n'
        (source / "Cargo.toml").write_bytes(original)
        package = {"id": "mac", "name": "block2", "version": "0.5.1", "source": "registry+x",
                   "manifest_path": str(source / "Cargo.toml"), "license": "MIT",
                   "repository": "https://github.com/madsmtm/objc2", "authors": ["Steven Sheldon"]}
        data = {"packages": [{"id": name, "name": name, "source": None}
                             for name in ("app", "panel-control")] + [package],
                "workspace_members": ["app", "panel-control"], "resolve": {"nodes": [
                    {"id": "app", "deps": [{"pkg": "mac", "dep_kinds": [{"kind": None}]}]},
                    {"id": "panel-control", "deps": []}, {"id": "mac", "deps": []}]}}
        metadata = self.root / "mac.json"
        metadata.write_text(json.dumps(data))
        collect = run_path(str(SCRIPT))["notices"]
        out = self.root / "mac-notices"
        collect(metadata, out)
        dest = out / "block2-0.5.1"
        self.assertEqual(hashlib.sha256((dest / "MIT-LICENSE.txt").read_bytes()).hexdigest(),
                         "e353f37b12aefbb9f9b29490e837cfee05d9bda70804b3562839a3285c1df1e5")
        self.assertEqual((dest / "UPSTREAM-Cargo.toml").read_bytes(), original)
        self.assertIn("Steven Sheldon", (dest / "FALLBACK-ATTRIBUTION.txt").read_text())
        self.assertEqual(hashlib.sha256((dest / "UPSTREAM-PROJECT-LICENSING.md").read_bytes()).hexdigest(),
                         "7f976f7e9cb2d87df7230606feb932c3f21ac0e664045a775b600046ff850c54")
        package["version"] = "99.0.0"
        metadata.write_text(json.dumps(data))
        with self.assertRaisesRegex(ValueError, "missing notice"):
            collect(metadata, self.root / "unverified")

    def test_embedded_font_terms_are_preserved_separately_from_crate_license(self):
        source = self.root / "fonts-source"
        (source / "fonts").mkdir(parents=True)
        names = ("OFL.txt", "UFL.txt", "Hack-Regular.txt", "emoji-icon-font-mit-license.txt")
        for name in names:
            (source / "fonts" / name).write_bytes(name.encode() + b" original terms\r\n")
        package = {"id": "fonts", "name": "epaint_default_fonts", "version": "0.36.1",
                   "source": "registry+x", "manifest_path": str(source / "Cargo.toml"),
                   "license": "(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0"}
        metadata = {"packages": [{"id": name, "name": name, "source": None}
                                 for name in ("app", "panel-control")] + [package],
                    "workspace_members": ["app", "panel-control"], "resolve": {"nodes": [
                        {"id": "app", "deps": [{"pkg": "fonts", "dep_kinds": [{"kind": None}]}]},
                        {"id": "panel-control", "deps": []}, {"id": "fonts", "deps": []}]}}
        collect = run_path(str(SCRIPT))["notices"]
        path = self.root / "metadata.json"
        path.write_text(json.dumps(metadata))
        output = self.root / "notices"
        collect(path, output)
        dest = output / "epaint_default_fonts-0.36.1"
        for name in names:
            self.assertEqual((dest / "fonts" / name).read_bytes(), (source / "fonts" / name).read_bytes())
        canonical = SCRIPT.parent.parent / "vendor/egui-winit/LICENSE-APACHE"
        self.assertEqual((dest / "SPDX-FALLBACK-APACHE-2.0").read_bytes(), canonical.read_bytes())
        (source / "fonts" / "UFL.txt").unlink()
        with self.assertRaisesRegex(ValueError, "missing notice for epaint_default_fonts"):
            collect(path, self.root / "missing")

    def test_wrong_tag_missing_empty_and_extra_artifacts_fail(self):
        self.assertNotEqual(self.run_release("--tag", "v0.1.0").returncode, 0)
        self.package.unlink()
        self.assertNotEqual(self.run_release().returncode, 0)
        self.package.touch()
        self.assertNotEqual(self.run_release().returncode, 0)
        self.package.write_bytes(b"data")
        (self.root / "unexpected.deb").write_bytes(b"data")
        self.assertNotEqual(self.run_release().returncode, 0)
        self.assertFalse((self.root / "SHA256SUMS").exists())

    def test_combined_ubuntu_arch_release_requires_both_and_hashes_their_bytes(self):
        arch = self.root / "ivy-terminal-0.1.0alpha1-1-x86_64.pkg.tar.zst"
        self.assertNotEqual(self.run_release("--include-arch").returncode, 0)
        arch.write_bytes(b"native Arch package")
        self.assertNotEqual(self.run_release().returncode, 0)
        self.assertEqual(self.run_release("--include-arch").returncode, 0)
        manifest = json.loads((self.root / "release-manifest.json").read_text())
        self.assertEqual([a["name"] for a in manifest["artifacts"]], [self.package.name, arch.name])
        self.assertEqual(manifest["artifacts"][1]["packaged_on"], "Arch Linux (rolling)")
        for line in (self.root / "SHA256SUMS").read_text().splitlines():
            checksum, name = line.split("  ")
            self.assertEqual(checksum, hashlib.sha256((self.root / name).read_bytes()).hexdigest())
        (self.root / "unexpected.pkg.tar.zst").write_bytes(b"extra")
        self.assertNotEqual(self.run_release("--include-arch").returncode, 0)

    def test_download_names_survive_github_asset_name_normalization(self):
        self.assertEqual(self.run_release().returncode, 0)
        # Public upload of '~' was observed to replace it with '.', breaking SHA256SUMS.
        name = json.loads((self.root / "release-manifest.json").read_text())["artifacts"][0]["name"]
        self.assertEqual(name, "ivy_0.1.0-alpha.1_amd64.deb")
        self.assertRegex(name, r"^[A-Za-z0-9._-]+$")

    def test_macos_release_requires_both_architectures_and_records_native_targets(self):
        arch = self.root / "ivy-terminal-0.1.0alpha1-1-x86_64.pkg.tar.zst"
        arch.write_bytes(b"Arch fixture")
        arm = self.root / "Ivy-0.1.0-alpha.1-macos-arm64.dmg"
        intel = self.root / "Ivy-0.1.0-alpha.1-macos-x86_64.dmg"
        args = ("--include-arch", "--include-macos")
        self.assertNotEqual(self.run_release(*args).returncode, 0)
        arm.write_bytes(b"Apple Silicon DMG fixture")
        self.assertNotEqual(self.run_release(*args).returncode, 0)
        intel.write_bytes(b"Intel DMG fixture")
        self.assertEqual(self.run_release(*args).returncode, 0)
        manifest = json.loads((self.root / "release-manifest.json").read_text())
        self.assertEqual(len(manifest["artifacts"]), 4)
        for entry, expected in zip(manifest["artifacts"][2:], (
                (arm, "aarch64-apple-darwin", "macos-15"),
                (intel, "x86_64-apple-darwin", "macos-15-intel"))):
            path, target, runner = expected
            self.assertEqual((entry["name"], entry["target"], entry["built_on"], entry["signing"]),
                             (path.name, target, runner, "unsigned"))
            self.assertEqual(entry["sha256"], hashlib.sha256(path.read_bytes()).hexdigest())
        self.assertEqual(len((self.root / "SHA256SUMS").read_text().splitlines()), 5)
        self.assertNotEqual(self.run_release("--include-arch").returncode, 0)
        (self.root / "unexpected.dmg").write_bytes(b"extra")
        self.assertNotEqual(self.run_release(*args).returncode, 0)

    def test_windows_release_requires_installer_and_hashes_actual_bytes(self):
        installer = self.root / "Ivy-0.1.0-alpha.1-windows-x86_64-setup.exe"
        self.assertNotEqual(self.run_release("--include-windows").returncode, 0)
        installer.write_bytes(b"Windows installer fixture")
        self.assertNotEqual(self.run_release().returncode, 0)
        self.assertEqual(self.run_release("--include-windows").returncode, 0)
        manifest = json.loads((self.root / "release-manifest.json").read_text())
        entry = manifest["artifacts"][1]
        self.assertEqual((entry["name"], entry["target"], entry["built_on"], entry["signing"]),
                         (installer.name, "x86_64-pc-windows-msvc", "windows-2025", "unsigned"))
        self.assertEqual(entry["sha256"], hashlib.sha256(installer.read_bytes()).hexdigest())
        for line in (self.root / "SHA256SUMS").read_text().splitlines():
            checksum, name = line.split("  ")
            self.assertEqual(checksum, hashlib.sha256((self.root / name).read_bytes()).hexdigest())
        (self.root / "unexpected.exe").write_bytes(b"extra")
        self.assertNotEqual(self.run_release("--include-windows").returncode, 0)

    def test_arch_versions_keep_numbered_identifiers_without_forbidden_hyphens(self):
        convert = run_path(str(SCRIPT))["arch_version"]
        for semver, arch in (("0.1.0-alpha.1", "0.1.0alpha1"),
                             ("0.1.0-alpha.10", "0.1.0alpha10"),
                             ("0.1.0-beta.2", "0.1.0beta2"),
                             ("0.1.0-rc.10", "0.1.0rc10"), ("0.1.0", "0.1.0")):
            self.assertEqual(convert(semver), arch)

    def test_invalid_semver_is_rejected(self):
        for value in ("0.1.0-alpha.01", "0.1.0+metadata", "01.1.0", "0.1.0-rc10", "0.1.0-alpha01"):
            with self.subTest(version=value):
                self.cargo.write_text(f'[workspace.package]\nversion = "{value}"\n')
                result = subprocess.run(
                    ["python3", str(SCRIPT), "version", "--cargo", str(self.cargo)],
                    capture_output=True, text=True,
                )
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("unsupported workspace release version", result.stderr)

    def test_package_builder_enforces_release_version_boundary(self):
        binaries = self.root / "binaries"
        binaries.mkdir()
        for value, accepted in (
            ("0.1.0-rc10", False), ("0.1.0-alpha01", False),
            ("0.1.0-alpha.01", False), ("01.1.0", False),
            ("0.1.0+metadata", False), ("0.1.0", True),
            ("0.1.0-alpha.0", True), ("0.1.0-beta.2", True),
            ("0.1.0-rc.10", True),
        ):
            with self.subTest(version=value):
                result = subprocess.run(
                    ["bash", str(SCRIPT.with_name("package-linux.sh")),
                     "--binary-dir", str(binaries), "--output-dir", str(self.root / "dist"),
                     "--version", value],
                    capture_output=True, text=True,
                )
                self.assertNotEqual(result.returncode, 0)
                if accepted:
                    # Reaching the missing-input check proves the version was accepted.
                    self.assertIn("Missing executable:", result.stderr)
                    self.assertNotIn("Unsupported release version:", result.stderr)
                else:
                    self.assertIn(f"Unsupported release version: {value}", result.stderr)
                    self.assertNotIn("Missing executable:", result.stderr)


    @unittest.skipUnless(all(shutil.which(tool) for tool in ("dpkg-deb", "dpkg-shlibdeps", "readelf", "true")),
                         "Linux package fixture requires Debian packaging tools")
    def test_linux_package_requires_helper_and_binds_complete_adapter_bytes(self):
        binaries = self.root / "fixture-binaries"
        binaries.mkdir()
        native_fixture = shutil.which("true")
        for name in ("app", "panel-mcp"):
            shutil.copy2(native_fixture, binaries / name)
        fonts = binaries / "notices/epaint_default_fonts-fixture/fonts"
        fonts.mkdir(parents=True)
        for name in ("OFL.txt", "UFL.txt", "Hack-Regular.txt", "emoji-icon-font-mit-license.txt"):
            (fonts / name).write_bytes(b"prepared fixture font terms\r\n")
        output = self.root / "linux-dist"
        builder = ["bash", str(SCRIPT.with_name("package-linux.sh")), "--binary-dir", str(binaries),
                   "--output-dir", str(output), "--version", "0.1.0-alpha.1"]
        missing = subprocess.run(builder, capture_output=True, text=True)
        self.assertNotEqual(missing.returncode, 0)
        self.assertIn("Missing executable:", missing.stderr)
        self.assertIn("ivy-session", missing.stderr)
        shutil.copy2(native_fixture, binaries / "ivy-session")
        missing = subprocess.run(builder, capture_output=True, text=True)
        self.assertIn("ivy-codex", missing.stderr)
        shutil.copy2(native_fixture, binaries / "ivy-codex")
        built = subprocess.run(builder, capture_output=True, text=True)
        self.assertEqual(built.returncode, 0, built.stderr)
        package = output / self.package.name
        validator = ["bash", str(SCRIPT.with_name("test-linux-package.sh")), str(package),
                     "0.1.0-alpha.1", str(binaries / "notices")]
        valid = subprocess.run(validator, capture_output=True, text=True)
        self.assertEqual(valid.returncode, 0, valid.stdout + valid.stderr)
        extracted = self.root / "altered-payload"
        subprocess.run(["dpkg-deb", "-R", str(package), str(extracted)], check=True, capture_output=True)
        sums = dict(line.split("  ", 1)[::-1]
                    for line in (extracted / "DEBIAN/md5sums").read_text().splitlines())
        payload = extracted / "usr/lib/ivy-terminal"
        checked = [payload / name for name in ("app", "panel-mcp", "ivy-session", "ivy-codex")]
        checked.extend(path for path in (payload / "session-recovery").rglob("*") if path.is_file())
        for path in checked:
            relative = path.relative_to(extracted).as_posix()
            self.assertEqual(sums[relative], hashlib.md5(path.read_bytes(), usedforsecurity=False).hexdigest())
        adapter = payload / "session-recovery/.claude-plugin/plugin.json"
        original = adapter.read_bytes()
        adapter.write_bytes(original + b"altered adapter content")
        subprocess.run(["dpkg-deb", "--root-owner-group", "--build", str(extracted), str(package)],
                       check=True, capture_output=True)
        self.assertNotEqual(subprocess.run(validator, capture_output=True).returncode, 0)
        adapter.write_bytes(original)
        (extracted / "usr/lib/ivy-terminal/ivy-session").unlink()
        subprocess.run(["dpkg-deb", "--root-owner-group", "--build", str(extracted), str(package)],
                       check=True, capture_output=True)
        self.assertNotEqual(subprocess.run(validator, capture_output=True).returncode, 0)


    def test_supported_versions_preserve_debian_upgrade_order(self):
        convert = run_path(str(SCRIPT))["deb_version"]
        versions = [
            "0.1.0-alpha.0", "0.1.0-alpha.1", "0.1.0-alpha.2", "0.1.0-alpha.10",
            "0.1.0-beta.0", "0.1.0-beta.2", "0.1.0-rc.0", "0.1.0-rc.2",
            "0.1.0-rc.10", "0.1.0", "0.1.1-alpha.0", "0.1.1", "1.0.0",
        ]
        for older, newer in zip(versions, versions[1:]):
            with self.subTest(older=older, newer=newer):
                result = subprocess.run(
                    ["dpkg", "--compare-versions", convert(older), "lt", convert(newer)],
                    capture_output=True, text=True,
                )
                self.assertEqual(result.returncode, 0, result.stderr)


    def test_remote_tag_must_still_name_the_approved_commit(self):
        local = self.root / "checkout"
        remote = self.root / "remote.git"

        def git(*args):
            return subprocess.check_output(
                ["git", "-c", "user.name=Package test", "-c", "user.email=test@example.invalid",
                 "-c", "core.hooksPath=/dev/null", "-c", "commit.gpgSign=false", *args],
                stderr=subprocess.DEVNULL, text=True,
            ).strip()

        git("init", "--quiet", "--object-format=sha1", str(local))
        git("-C", str(local), "commit", "--quiet", "--allow-empty", "-m", "first")
        first = git("-C", str(local), "rev-parse", "HEAD")
        git("-C", str(local), "tag", "--no-sign", "-a", "v0.1.0-alpha.1", "-m", "alpha")
        git("-C", str(local), "commit", "--quiet", "--allow-empty", "-m", "second")
        second = git("-C", str(local), "rev-parse", "HEAD")
        git("clone", "--quiet", "--bare", str(local), str(remote))
        git("-C", str(local), "remote", "add", "origin", str(remote))
        command = ["bash", str(SCRIPT.with_name("verify-release-tag.sh")), "v0.1.0-alpha.1", first]
        self.assertEqual(subprocess.run(command, cwd=local, capture_output=True).returncode, 0)
        git("--git-dir", str(remote), "update-ref", "refs/tags/v0.1.0-alpha.1", second)
        self.assertNotEqual(subprocess.run(command, cwd=local, capture_output=True).returncode, 0)
        git("--git-dir", str(remote), "update-ref", "-d", "refs/tags/v0.1.0-alpha.1")
        self.assertNotEqual(subprocess.run(command, cwd=local, capture_output=True).returncode, 0)



if __name__ == "__main__":
    unittest.main()
