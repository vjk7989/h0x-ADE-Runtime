from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
INSTALLER = REPO_ROOT / "distribution" / "install.sh"
REQUIRED_COMMANDS = ("awk", "cat", "chmod", "cp", "mkdir", "mktemp", "mv", "rm")


@unittest.skipUnless(os.name == "posix", "Unix installer requires a POSIX host")
class UnixInstallerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp_dir = tempfile.TemporaryDirectory(prefix="h0x-installer-test-")
        self.root = Path(self.temp_dir.name)
        self.bin_dir = self.root / "bin"
        self.bin_dir.mkdir()
        self.install_dir = self.root / "install"
        self.payload = self.root / "payload"
        # `update --verify <file>` is how the installer checks the release signature.
        self.payload.write_bytes(
            b'#!/bin/sh\n[ "$1 $2" = "update --verify" ] && exit "${FAKE_VERIFY_EXIT:-0}"\nexit 0\n'
        )
        self.expected_sha256 = hashlib.sha256(self.payload.read_bytes()).hexdigest()

        for command in REQUIRED_COMMANDS:
            path = shutil.which(command)
            if path is None:
                self.fail(f"test host is missing required command: {command}")
            (self.bin_dir / command).symlink_to(path)

        self._write_executable(
            "uname",
            """#!/bin/sh
case "$1" in
  -s) echo Linux ;;
  -m) echo x86_64 ;;
  -o)
    [ "${FAKE_UNAME_OS_UNAVAILABLE:-}" != "1" ] || exit 1
    echo "${FAKE_UNAME_OS:-GNU/Linux}"
    ;;
  *) exit 1 ;;
esac
""",
        )
        self._write_executable(
            "curl",
            """#!/bin/sh
: > "$FAKE_CURL_MARKER"
out=""
previous=""
for argument in "$@"; do
  if [ "$previous" = "-o" ]; then
    out="$argument"
    break
  fi
  previous="$argument"
done
case "$out" in
  *.minisig) [ -z "${FAKE_NO_SIGNATURE:-}" ] || exit 22 ;;
esac
if [ -n "$out" ]; then
  cp "$FAKE_PAYLOAD" "$out"
else
  cat "$FAKE_MANIFEST"
fi
""",
        )

    def tearDown(self) -> None:
        self.temp_dir.cleanup()

    def _write_executable(self, name: str, content: str) -> None:
        path = self.bin_dir / name
        path.write_text(content, encoding="utf-8")
        path.chmod(0o755)

    def _select_checksum_tool(self, tool: str) -> None:
        if tool == "sha256sum":
            path = shutil.which("sha256sum")
            if path is None:
                self.fail("test host is missing sha256sum")
            (self.bin_dir / "sha256sum").symlink_to(path)
            return

        if tool == "shasum":
            sha256sum = shutil.which("sha256sum")
            if sha256sum is None:
                self.fail("test host is missing sha256sum for the shasum fixture")
            self._write_executable(
                "shasum",
                f"""#!/bin/sh
[ "$1" = "-a" ] && [ "$2" = "256" ] || exit 2
shift 2
exec {sha256sum} "$@"
""",
            )
            return

        if tool == "openssl":
            path = shutil.which("openssl")
            if path is None:
                self.fail("test host is missing openssl")
            (self.bin_dir / "openssl").symlink_to(path)
            return

        self.fail(f"unknown checksum tool fixture: {tool}")

    def _write_manifest(self, checksum: str | None) -> Path:
        manifest: dict[str, object] = {
            "version": "9.9.9",
            "assets": {
                "linux-x86_64": "https://example.invalid/h0x-linux-x86_64"
            },
        }
        if checksum is not None:
            manifest["sha256"] = {"linux-x86_64": checksum}
        path = self.root / "latest.json"
        path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
        return path

    def _run_installer(
        self,
        checksum: str | None,
        tool: str = "sha256sum",
        extra_env: dict[str, str] | None = None,
    ) -> subprocess.CompletedProcess[str]:
        self._select_checksum_tool(tool)
        manifest = self._write_manifest(checksum)
        env = {
            **os.environ,
            "PATH": str(self.bin_dir),
            "FAKE_MANIFEST": str(manifest),
            "FAKE_PAYLOAD": str(self.payload),
            "FAKE_CURL_MARKER": str(self.root / "curl-called"),
            "H0X_INSTALL_DIR": str(self.install_dir),
            **(extra_env or {}),
        }
        return subprocess.run(
            ["/bin/sh", str(INSTALLER)],
            env=env,
            capture_output=True,
            text=True,
            check=False,
        )

    def test_valid_download_uses_each_supported_checksum_tool(self) -> None:
        for tool in ("sha256sum", "shasum", "openssl"):
            with self.subTest(tool=tool):
                shutil.rmtree(self.install_dir, ignore_errors=True)
                for name in ("sha256sum", "shasum", "openssl"):
                    (self.bin_dir / name).unlink(missing_ok=True)

                result = self._run_installer(self.expected_sha256.upper(), tool)

                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual((self.install_dir / "h0x").read_bytes(), self.payload.read_bytes())

    def test_android_is_rejected_before_replacing_existing_binary(self) -> None:
        self.install_dir.mkdir()
        installed = self.install_dir / "h0x"
        installed.write_bytes(b"existing-h0x\n")

        result = self._run_installer(
            self.expected_sha256,
            extra_env={"FAKE_UNAME_OS": "Android"},
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Android/Termux is not currently supported", result.stderr)
        self.assertFalse((self.root / "curl-called").exists())
        self.assertEqual(installed.read_bytes(), b"existing-h0x\n")

    def test_missing_uname_operating_system_flag_keeps_linux_supported(self) -> None:
        result = self._run_installer(
            self.expected_sha256,
            extra_env={"FAKE_UNAME_OS_UNAVAILABLE": "1"},
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.install_dir / "h0x").read_bytes(), self.payload.read_bytes())

    def test_checksum_mismatch_does_not_replace_existing_binary(self) -> None:
        self.install_dir.mkdir()
        installed = self.install_dir / "h0x"
        installed.write_bytes(b"existing-h0x\n")

        result = self._run_installer("0" * 64)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("checksum did not match", result.stderr)
        self.assertEqual(installed.read_bytes(), b"existing-h0x\n")

    def test_missing_checksum_fails_without_replacing_existing_binary(self) -> None:
        self.install_dir.mkdir()
        installed = self.install_dir / "h0x"
        installed.write_bytes(b"existing-h0x\n")

        result = self._run_installer(None)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("valid SHA-256 checksum", result.stderr)
        self.assertEqual(installed.read_bytes(), b"existing-h0x\n")

    def test_release_without_a_signature_does_not_replace_existing_binary(self) -> None:
        self.install_dir.mkdir()
        installed = self.install_dir / "h0x"
        installed.write_bytes(b"existing-h0x\n")

        result = self._run_installer(
            self.expected_sha256, extra_env={"FAKE_NO_SIGNATURE": "1"}
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("no signature for h0x-linux-x86_64", result.stderr)
        self.assertEqual(installed.read_bytes(), b"existing-h0x\n")

    def test_bad_signature_does_not_replace_existing_binary(self) -> None:
        self.install_dir.mkdir()
        installed = self.install_dir / "h0x"
        installed.write_bytes(b"existing-h0x\n")

        result = self._run_installer(
            self.expected_sha256, extra_env={"FAKE_VERIFY_EXIT": "1"}
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("is not signed by the h0x release key", result.stderr)
        self.assertEqual(installed.read_bytes(), b"existing-h0x\n")

    def test_installed_verifier_is_used_and_the_download_never_runs(self) -> None:
        # The download is an attacker binary that would vouch for itself.
        downloaded_ran = self.root / "downloaded-ran"
        self.payload.write_bytes(
            f'#!/bin/sh\n: > "{downloaded_ran}"\nexit 0\n'.encode()
        )
        payload_sha256 = hashlib.sha256(self.payload.read_bytes()).hexdigest()
        verified = self.root / "verified-file"
        for verify_exit in (1, 0):
            with self.subTest(verify_exit=verify_exit):
                shutil.rmtree(self.install_dir, ignore_errors=True)
                self.install_dir.mkdir()
                installed = self.install_dir / "h0x"
                installed_script = (
                    "#!/bin/sh\n"
                    'case "$1 $2" in\n'
                    '  "update --help") echo "usage: h0x update [--handoff | --verify <file>]" >&2; exit 2 ;;\n'
                    f'  "update --verify") printf %s "$3" > "{verified}"; exit {verify_exit} ;;\n'
                    "esac\n"
                    "exit 3\n"
                )
                installed.write_text(installed_script, encoding="utf-8")
                installed.chmod(0o755)
                verified.unlink(missing_ok=True)
                (self.bin_dir / "sha256sum").unlink(missing_ok=True)

                result = self._run_installer(payload_sha256)

                self.assertFalse(downloaded_ran.exists(), result.stderr)
                self.assertNotIn("trusted on first use", result.stdout)
                # The signature is checked for the release's own file name.
                self.assertEqual(Path(verified.read_text()).name, "h0x-linux-x86_64")
                if verify_exit:
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn("is not signed by the h0x release key", result.stderr)
                    self.assertEqual(installed.read_text(encoding="utf-8"), installed_script)
                else:
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(installed.read_bytes(), self.payload.read_bytes())
                    self.assertFalse(downloaded_ran.exists())

    def test_older_installed_binary_without_verify_falls_back_and_still_fails_closed(self) -> None:
        self.install_dir.mkdir()
        installed = self.install_dir / "h0x"
        older = '#!/bin/sh\necho "usage: h0x update [--handoff]" >&2\nexit 2\n'
        installed.write_text(older, encoding="utf-8")
        installed.chmod(0o755)

        result = self._run_installer(
            self.expected_sha256, extra_env={"FAKE_VERIFY_EXIT": "1"}
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("trusted on first use", result.stdout)
        self.assertIn("is not signed by the h0x release key", result.stderr)
        self.assertEqual(installed.read_text(encoding="utf-8"), older)

    def test_asset_url_query_and_fragment_are_not_part_of_the_verified_file_name(self) -> None:
        verified = self.root / "verified-file"
        self.payload.write_bytes(
            b'#!/bin/sh\n[ "$1 $2" = "update --verify" ] || exit 0\n'
            + f'printf %s "$3" > "{verified}"\nexit 0\n'.encode()
        )
        manifest = self.root / "query-latest.json"
        sha256 = hashlib.sha256(self.payload.read_bytes()).hexdigest()
        manifest.write_text(
            json.dumps(
                {
                    "version": "9.9.9",
                    "assets": {
                        "linux-x86_64": "https://example.invalid/v9.9.9/h0x-linux-x86_64?download=1#top"
                    },
                    "sha256": {"linux-x86_64": sha256},
                },
                indent=2,
            )
            + "\n",
            encoding="utf-8",
        )

        result = self._run_installer(sha256, extra_env={"FAKE_MANIFEST": str(manifest)})

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(Path(verified.read_text()).name, "h0x-linux-x86_64")
        self.assertEqual(
            (self.install_dir / "h0x").read_bytes(), self.payload.read_bytes()
        )


if __name__ == "__main__":
    unittest.main()
