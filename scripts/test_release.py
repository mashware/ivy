import hashlib
import json
from pathlib import Path
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
        for value in ("0.1.0-alpha.01", "0.1.0+metadata", "01.1.0"):
            with self.subTest(version=value):
                self.cargo.write_text(f'[workspace.package]\nversion = "{value}"\n')
                result = subprocess.run(
                    ["python3", str(SCRIPT), "version", "--cargo", str(self.cargo)],
                    capture_output=True, text=True,
                )
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("invalid workspace SemVer", result.stderr)



if __name__ == "__main__":
    unittest.main()
