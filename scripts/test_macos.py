import os
from pathlib import Path
import plistlib
import shutil
from runpy import run_path
import tempfile
import unittest

MAC = run_path(str(Path(__file__).with_name("package-macos.py")))


class MacPackageTest(unittest.TestCase):
    def test_bundle_metadata_names_existing_executables_and_preserves_exact_release(self):
        expected = MAC["info"]("0.1.0-alpha.3")
        self.assertEqual(expected["CFBundleIdentifier"], "io.github.mashware.ivy")
        self.assertEqual(expected["CFBundleExecutable"], "app")
        self.assertEqual(expected["IvyReleaseVersion"], "0.1.0-alpha.3")
        self.assertEqual(expected["LSMinimumSystemVersion"], "15.0")
        self.assertEqual(expected["CFBundleShortVersionString"], "0.1.0")
        for value in ("0.1.0-alpha.01", "0.1.0-rc10", "../invalid", "0.1.0+build"):
            with self.assertRaises(ValueError):
                MAC["info"](value)

    def test_wrong_release_missing_bridge_or_notice_cannot_pass_content_check(self):
        with tempfile.TemporaryDirectory() as temp:
            bundle = Path(temp) / "Ivy.app"
            contents = bundle / "Contents"
            (contents / "MacOS").mkdir(parents=True)
            with (contents / "Info.plist").open("wb") as file:
                plistlib.dump(MAC["info"]("0.1.0-alpha.3"), file)
            for name in MAC["EXECUTABLES"]:
                path = contents / "MacOS" / name
                path.write_bytes(b"portable content fixture, not Mach-O")
                path.chmod(0o755)
            resources = contents / "Resources"
            for name in ("Ivy.icns", "licenses/LICENSE", "licenses/COPYRIGHT", "licenses/THIRD-PARTY.md",
                         "licenses/vendor/egui-winit/LICENSE-MIT", "licenses/vendor/egui-winit/LICENSE-APACHE",
                         "licenses/themes/catppuccin-LICENSE", "licenses/themes/github-LICENSE",
                         "licenses/themes/nord-LICENSE", "licenses/themes/rose-pine-LICENSE", "licenses/cargo/dep/LICENSE"):
                path = resources / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b"fixture original text\r\n")
            shutil.copytree(Path(__file__).resolve().parent.parent / "integrations/session-recovery",
                            resources / "session-recovery")
            check = MAC["check_contents"]
            check(bundle, "0.1.0-alpha.3")
            with self.assertRaises(ValueError):
                check(bundle, "0.1.0-alpha.4")
            helper = contents / "MacOS/ivy-session"
            helper.unlink()
            with self.assertRaisesRegex(ValueError, "missing executable: ivy-session"):
                check(bundle, "0.1.0-alpha.3")
            helper.write_bytes(b"portable helper fixture, not Mach-O")
            helper.chmod(0o755)
            metadata = resources / "session-recovery/.claude-plugin/plugin.json"
            original = metadata.read_bytes()
            metadata.write_bytes(original + b"tampered")
            with self.assertRaisesRegex(ValueError, "session recovery resources differ"):
                check(bundle, "0.1.0-alpha.3")
            metadata.write_bytes(original)
            metadata.unlink()
            with self.assertRaisesRegex(ValueError, "session recovery resources differ"):
                check(bundle, "0.1.0-alpha.3")
            metadata.write_bytes(original)
            check(bundle, "0.1.0-alpha.3")
            bridge = contents / "MacOS/panel-mcp"
            bridge.chmod(0o644)
            with self.assertRaisesRegex(ValueError, "missing executable"):
                check(bundle, "0.1.0-alpha.3")
            bridge.chmod(0o755)
            (resources / "licenses/COPYRIGHT").unlink()
            with self.assertRaisesRegex(ValueError, "missing bundle resource"):
                check(bundle, "0.1.0-alpha.3")

    def test_native_load_command_contract_rejects_wrong_os_baseline_and_homebrew_dependency(self):
        command = "Load command 1\n cmd LC_BUILD_VERSION\n cmdsize 32\n platform MACOS\n minos 15.0\n sdk 15.5\n"
        libraries = "app:\n /usr/lib/libSystem.B.dylib (compatibility version 1.0.0)\n"
        check = MAC["check_load_commands"]
        check(command, libraries)
        for bad in (command.replace("MACOS", "IOS"), command.replace("15.0", "14.0"), ""):
            with self.assertRaises(ValueError):
                check(bad, libraries)
        with self.assertRaisesRegex(ValueError, "external runtime dependency"):
            check(command, libraries + " /opt/homebrew/lib/libtest.dylib (compatibility version 1.0.0)\n")
        if os.uname().sysname != "Darwin":
            with self.assertRaisesRegex(ValueError, "require macOS"):
                MAC["check_native"](Path("missing.app"), "0.1.0-alpha.3", "arm64")


if __name__ == "__main__":
    unittest.main()
