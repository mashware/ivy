import hashlib
import json
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
