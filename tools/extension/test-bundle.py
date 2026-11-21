"""Integration checks for the locally distributed extension and setup helper.

Run pack.ps1 first, then: python tools/extension/test-bundle.py
"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
import zipfile

ROOT = Path(__file__).resolve().parents[2]
BUNDLE = ROOT / "target/extension/bundle"
BASH = shutil.which("bash") if os.name != "nt" else shutil.which("bash", path=r"C:\Program Files\Git\bin")


class BundleTests(unittest.TestCase):
    def test_offline_archive_contains_matching_browser_packages_and_helpers(self):
        self.assertTrue((ROOT / "target/extension/sdl-local.zip").is_file(), "Offline bundle has not been built")
        with zipfile.ZipFile(ROOT / "target/extension/sdl-local.zip") as archive:
            names = set(archive.namelist())
            for name in ("chromium/manifest.json", "sdl-firefox.xpi", "install-local.ps1",
                         "install-local.sh", "README.txt", "Set up browser extension.command"):
                self.assertIn(name, names)
            chromium = json.loads(archive.read("chromium/manifest.json"))
            self.assertEqual(chromium, json.loads((BUNDLE / "chromium/manifest.json").read_text("utf-8")))
        with zipfile.ZipFile(BUNDLE / "sdl-firefox.xpi") as firefox:
            manifest = json.loads(firefox.read("manifest.json"))
        self.assertEqual(chromium["version"], manifest["version"])
        self.assertIn("nativeMessaging", manifest["permissions"])

    @unittest.skipUnless(BASH, "requires Bash")
    def test_unix_inspection_does_not_register_host_or_launch_browser(self):
        with tempfile.TemporaryDirectory(prefix="swoopdl bundle ") as temp:
            package = Path(temp) / "extension"
            shutil.copytree(BUNDLE, package)
            result = subprocess.run([BASH, str(package / "install-local.sh"), "--check-only"],
                                    capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertIn("nothing launched or registered", result.stdout)

    @unittest.skipUnless(BASH, "requires Bash")
    def test_unix_setup_rejects_missing_packages_before_registering(self):
        with tempfile.TemporaryDirectory(prefix="swoopdl bundle ") as temp:
            script = Path(temp) / "install-local.sh"
            shutil.copyfile(BUNDLE / script.name, script)
            result = subprocess.run([BASH, str(script), "--check-only"], capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Missing bundled file", result.stderr)

    @unittest.skipUnless(BASH, "requires Bash")
    def test_unix_setup_rejects_unknown_browser_family(self):
        result = subprocess.run([BASH, str(BUNDLE / "install-local.sh"), "--browser", "unknown"],
                                capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Use auto, chromium, or firefox", result.stderr)

    @unittest.skipUnless(BASH, "requires Bash")
    def test_unix_setup_registers_local_host_and_opens_internal_browser_page(self):
        with tempfile.TemporaryDirectory(prefix="swoopdl bundle ") as temp:
            base = Path(temp)
            package = base / "extension"
            shutil.copytree(BUNDLE, package)
            log = base / "calls.txt"
            for name in ("swoopdl-nmhost", "google-chrome"):
                script = base / name
                script.write_text('#!/bin/sh\nprintf "%s\\n" "$0 $*" >> "$SWOOPDL_SETUP_TEST_LOG"\n', encoding="utf-8", newline="\n")
                script.chmod(0o755)
            env = dict(os.environ, SWOOPDL_SETUP_TEST_LOG=log.as_posix(), PATH=str(base) + os.pathsep + os.environ["PATH"])
            result = subprocess.run([BASH, str(package / "install-local.sh"), "--browser", "chromium", "--no-pause"],
                                    capture_output=True, text=True, env=env)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            calls = log.read_text("utf-8")
            self.assertIn("swoopdl-nmhost --register", calls)
            self.assertIn("google-chrome chrome://extensions", calls)
            self.assertNotIn("https://", calls)


if __name__ == "__main__":
    unittest.main()
