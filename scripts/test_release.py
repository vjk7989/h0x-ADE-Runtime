import os
import subprocess
import tempfile
import unittest
from pathlib import Path

from scripts import release


CARGO_TOML = '[package]\nname = "h0x"\nversion = "{version}"\n[dependencies]\nserde = "1"\n'
CARGO_LOCK = 'version = 4\n[[package]]\nname = "h0x"\nversion = "{version}"\n[[package]]\nname = "serde"\nversion = "1.0.0"\n'


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="h0x-release-", dir="/var/tmp" if os.name != "nt" else None)
        self.addCleanup(self.temp.cleanup)
        self.previous_cwd = Path.cwd()
        os.chdir(self.temp.name)
        self.addCleanup(os.chdir, self.previous_cwd)
        self.git("init", "-q", "-b", "main")
        self.git("config", "user.name", "Release Test")
        self.git("config", "user.email", "release@example.invalid")
        self.put("Cargo.toml", CARGO_TOML.format(version="1.0.0"))
        self.put("Cargo.lock", CARGO_LOCK.format(version="1.0.0"))
        self.put("src/main.rs", "fn main() {}\n")
        self.stable = self.commit("stable")
        self.git("tag", "v1.0.0")

    def git(self, *args):
        return subprocess.check_output(["git", *args], text=True, stderr=subprocess.STDOUT).strip()

    def put(self, path, text):
        target = Path(path)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding="utf-8")

    def commit(self, message):
        self.git("add", ".")
        self.git("commit", "-qm", message)
        return self.git("rev-parse", "HEAD")

    def test_tag_matching_cargo_version_is_accepted(self):
        self.assertEqual(release.validate_tag("v1.0.0"), self.stable)
        self.assertEqual(release.validate_version("HEAD", "1.0.0"), self.stable)
        self.git("tag", "-a", "v1.0.0-annotated", "-m", "annotated")
        self.put("Cargo.toml", CARGO_TOML.format(version="1.0.1"))
        self.put("Cargo.lock", CARGO_LOCK.format(version="1.0.1"))
        candidate = self.commit("release: v1.0.1")
        self.git("tag", "-a", "v1.0.1", "-m", "v1.0.1")
        self.assertEqual(release.validate_tag("v1.0.1"), candidate)

    def test_non_stable_tags_and_versions_are_rejected(self):
        for tag in ("v1.0", "1.0.0", "preview-2026-01-01", "v1.0.0-rc.1", ""):
            with self.subTest(tag=tag), self.assertRaisesRegex(ValueError, "stable vX.Y.Z tag"):
                release.validate_tag(tag)
        for version in ("1.0", "v1.0.0", "1.0.0-rc.1", ""):
            with self.subTest(version=version), self.assertRaisesRegex(ValueError, "invalid stable version"):
                release.validate_version("HEAD", version)

    def test_tag_version_must_match_both_cargo_files(self):
        self.git("tag", "v1.0.1")
        with self.assertRaisesRegex(ValueError, "version must match"):
            release.validate_tag("v1.0.1")
        self.put("Cargo.toml", CARGO_TOML.format(version="1.0.1"))
        self.commit("bump Cargo.toml only")
        self.git("tag", "-f", "v1.0.1")
        with self.assertRaisesRegex(ValueError, "version must match"):
            release.validate_tag("v1.0.1")
        with self.assertRaisesRegex(ValueError, "version must match"):
            release.validate_version("HEAD", "1.0.1")

    def test_missing_tag_or_commit_fails(self):
        with self.assertRaises(subprocess.CalledProcessError):
            release.validate_tag("v9.9.9")
        with self.assertRaises(subprocess.CalledProcessError):
            release.validate_version("does-not-exist", "1.0.0")

    def test_cargo_lock_must_contain_exactly_one_local_h0x_package(self):
        lock = CARGO_LOCK.format(version="1.0.0")
        self.assertEqual(release.cargo_version(lock, "Cargo.lock"), "1.0.0")
        self.assertEqual(release.cargo_version(CARGO_TOML.format(version="1.0.0"), "Cargo.toml"), "1.0.0")
        registry = lock.replace('name = "h0x"\nversion = "1.0.0"\n', 'name = "h0x"\nversion = "1.0.0"\nsource = "registry"\n')
        with self.assertRaisesRegex(ValueError, "exactly one local h0x package"):
            release.cargo_version(registry, "Cargo.lock")
        duplicated = lock + '[[package]]\nname = "h0x"\nversion = "2.0.0"\n'
        with self.assertRaisesRegex(ValueError, "exactly one local h0x package"):
            release.cargo_version(duplicated, "Cargo.lock")
        with self.assertRaisesRegex(ValueError, "exactly one local h0x package"):
            release.cargo_version('version = 4\n[[package]]\nname = "serde"\nversion = "1.0.0"\n', "Cargo.lock")


if __name__ == "__main__":
    unittest.main()
