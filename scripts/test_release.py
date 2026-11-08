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
        self.package = self.root / "ivy_0.1.0~alpha.1_amd64.deb"
        self.package.write_bytes(b"actual package contents")

    def run_release(self, *extra):
        return subprocess.run(["python3", str(SCRIPT), "generate", "--cargo", str(self.cargo),
                               "--directory", str(self.root), "--commit", "a" * 40,
                               "--built-on", "test-builder", *extra],
                              capture_output=True, text=True)

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
