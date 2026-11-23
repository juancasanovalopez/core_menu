import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

import install_release as installer


class InstallerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.keys = tempfile.TemporaryDirectory()
        cls.key = Path(cls.keys.name, "key")
        subprocess.run(["ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", str(cls.key)], check=True)
        cls.public_key = cls.key.with_suffix(".pub").read_text().strip()

    @classmethod
    def tearDownClass(cls):
        cls.keys.cleanup()

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.binary = b"test binary"

    def signed(self, target="x86_64-unknown-linux-musl", tag="v1.2.3", **overrides):
        manifest = {
            "schema": 1, "version": tag[1:], "target": target,
            "asset": "core_menu-{}-{}".format(tag, target),
            "size": len(self.binary), "sha256": hashlib.sha256(self.binary).hexdigest(),
        }
        manifest.update(overrides)
        data = json.dumps(manifest, separators=(",", ":")).encode()
        path = self.root / "manifest"
        path.write_bytes(data)
        signature = self.root / "manifest.sig"
        signature.unlink(missing_ok=True)
        subprocess.run(["ssh-keygen", "-Y", "sign", "-f", str(self.key), "-n",
                        "core_menu-release", str(path)], check=True, stdout=subprocess.DEVNULL,
                       stderr=subprocess.DEVNULL)
        return data, signature.read_bytes()

    def test_public_key_matches_updater(self):
        actual = Path(__file__).resolve().parents[1].joinpath("config/update-public-key.pub").read_text().split()
        self.assertEqual(installer.PUBLIC_KEY.split(), actual[:2])

    def test_signed_manifests_for_both_architectures_and_corruption(self):
        for target in ("x86_64-unknown-linux-musl", "aarch64-unknown-linux-musl"):
            with self.subTest(target=target):
                data, signature = self.signed(target)
                manifest = installer.verify_manifest(data, signature, "v1.2.3", target, self.public_key)
                installer.verify_binary(self.binary, manifest)
                with self.assertRaises(installer.InstallError):
                    installer.verify_binary(self.binary + b"bad", manifest)
                with self.assertRaises(installer.InstallError):
                    installer.verify_manifest(data + b" ", signature, "v1.2.3", target, self.public_key)
                with self.assertRaises(installer.InstallError):
                    installer.verify_manifest(data, b"bad", "v1.2.3", target, self.public_key)

    def test_signed_incompatible_fields_are_rejected(self):
        for field, value in (("schema", True), ("version", "1.2.4"), ("target", "other"),
                             ("asset", "other"), ("size", 0), ("size", installer.MAX_BINARY + 1),
                             ("sha256", "F" * 64), ("extra", 1)):
            with self.subTest(field=field, value=value):
                data, signature = self.signed(**{field: value})
                with self.assertRaises(installer.InstallError):
                    installer.verify_manifest(data, signature, "v1.2.3",
                                              "x86_64-unknown-linux-musl", self.public_key)

    def test_release_metadata_and_downloads_are_checked(self):
        target = installer.TARGETS["x86_64"]
        tag = "v1.2.3"
        data, signature = self.signed(target)
        names = {
            "update-manifest-{}.json".format(target): data,
            "update-manifest-{}.sig".format(target): signature,
            "core_menu-{}-{}".format(tag, target): self.binary,
        }
        prefix = "https://github.com/{}/releases/download/{}/".format(installer.REPOSITORY, tag)
        release = {"draft": False, "prerelease": False, "tag_name": tag, "assets": [
            {"name": name, "size": len(content), "browser_download_url": prefix + name}
            for name, content in names.items()
        ]}

        def fake_download(url, limit, deadline):
            if url.startswith("https://api.github.com/"):
                return json.dumps(release).encode()
            return names[url.removeprefix(prefix)]

        with mock.patch.object(installer, "download", side_effect=fake_download), \
             mock.patch.object(installer, "PUBLIC_KEY", self.public_key):
            self.assertEqual(installer.fetch_release(target)[3], self.binary)
            release["assets"].pop()
            with self.assertRaises(installer.InstallError):
                installer.fetch_release(target)
            release["assets"].append({"name": "core_menu-{}-{}".format(tag, target),
                                      "size": len(self.binary), "browser_download_url": "https://evil.invalid/binary"})
            with self.assertRaises(installer.InstallError):
                installer.fetch_release(target)

    def test_latest_release_404_is_distinct_from_other_http_errors(self):
        target = installer.TARGETS["x86_64"]
        missing = installer.urllib.error.HTTPError("https://api.github.com/", 404, "Not Found", None, None)
        with mock.patch.object(installer, "download", side_effect=missing):
            with self.assertRaisesRegex(installer.InstallError, "release estable accesible"):
                installer.fetch_release(target)

        forbidden = installer.urllib.error.HTTPError("https://api.github.com/", 403, "Forbidden", None, None)
        with mock.patch.object(installer, "download", side_effect=forbidden):
            with self.assertRaises(installer.urllib.error.HTTPError) as caught:
                installer.fetch_release(target)
            self.assertEqual(caught.exception.code, 403)

        tag = "v1.2.3"
        names = ["update-manifest-{}.json".format(target), "update-manifest-{}.sig".format(target)]
        prefix = "https://github.com/{}/releases/download/{}/".format(installer.REPOSITORY, tag)
        release = {"draft": False, "prerelease": False, "tag_name": tag, "assets": [
            {"name": name, "size": 1, "browser_download_url": prefix + name} for name in names
        ]}
        asset_missing = installer.urllib.error.HTTPError(prefix + names[0], 404, "Not Found", None, None)
        with mock.patch.object(installer, "download", side_effect=[json.dumps(release).encode(), asset_missing]):
            with self.assertRaises(installer.urllib.error.HTTPError) as caught:
                installer.fetch_release(target)
            self.assertEqual(caught.exception.url, prefix + names[0])

    def test_signed_release_installs_without_rust_for_both_targets(self):
        for machine in ("x86_64", "aarch64"):
            target = installer.TARGETS[machine]
            tag = "v1.2.3"
            data, signature = self.signed(target)
            files = {
                "update-manifest-{}.json".format(target): data,
                "update-manifest-{}.sig".format(target): signature,
                "core_menu-{}-{}".format(tag, target): self.binary,
            }
            prefix = "https://github.com/{}/releases/download/{}/".format(installer.REPOSITORY, tag)
            release = {"draft": False, "prerelease": False, "tag_name": tag, "assets": [
                {"name": name, "size": len(content), "browser_download_url": prefix + name}
                for name, content in files.items()
            ]}

            def fake_download(url, limit, deadline):
                if url.startswith("https://api.github.com/"):
                    return json.dumps(release).encode()
                return files[url[len(prefix):]]

            home = self.root / machine
            home.mkdir()
            with self.subTest(machine=machine), \
                 mock.patch.dict(os.environ, {"PATH": "/usr/bin:/bin"}), \
                 mock.patch.object(installer.Path, "home", return_value=home), \
                 mock.patch.object(installer.platform, "machine", return_value=machine), \
                 mock.patch.object(installer, "package_managed", return_value=False), \
                 mock.patch.object(installer, "PUBLIC_KEY", self.public_key), \
                 mock.patch.object(installer, "download", side_effect=fake_download), \
                 mock.patch.object(sys, "argv", ["install_release.py"]), \
                 mock.patch("builtins.input", return_value="1"):
                installer.main()
                self.assertEqual((home / ".local/bin/core_menu").read_bytes(), self.binary)

    def test_versions_and_redirect_origin(self):
        for tag in ("v01.0.0", "v1.0.0-rc.1", "v1.0.0+build", "1.0.0", "v1.2"):
            with self.subTest(tag=tag), self.assertRaises(installer.InstallError):
                installer.version(tag)
        self.assertGreater(installer.version("v1.10.0"), installer.version("v1.9.0"))
        for url in ("http://github.com/x", "https://github.com.evil.invalid/x",
                    "https://user@github.com/x", "https://github.com:443/x"):
            self.assertFalse(installer.approved_url(url))
        with self.assertRaises(installer.InstallError):
            installer.SafeRedirect().redirect_request(None, None, 302, "", None,
                                                       "https://evil.invalid/binary")

    def test_manifest_size_limits_are_checked_before_signature(self):
        for manifest, signature in ((b"x" * (installer.MAX_MANIFEST + 1), b"x"),
                                    (b"x", b"x" * (installer.MAX_SIGNATURE + 1)),
                                    (b"", b"x"), (b"x", b"")):
            with self.subTest(manifest_size=len(manifest), signature_size=len(signature)):
                with self.assertRaises(installer.InstallError):
                    installer.verify_manifest(manifest, signature, "v1.2.3",
                                              installer.TARGETS["x86_64"], self.public_key)

    def test_atomic_install_backup_symlink_and_changed_destination(self):
        path = self.root / "core_menu"
        path.write_bytes(b"old")
        with mock.patch.object(installer, "package_managed", return_value=False):
            expected = installer.fingerprint(path, os.geteuid())
            backup = installer.install(path, expected, self.binary, os.geteuid())
            self.assertEqual(path.read_bytes(), self.binary)
            self.assertEqual(backup.read_bytes(), b"old")
            self.assertEqual(path.stat().st_mode & 0o777, 0o755)
            with self.assertRaises(installer.InstallError):
                installer.install(path, expected, b"other", os.geteuid())
            self.assertEqual(path.read_bytes(), self.binary)
            link = self.root / "link"
            link.symlink_to(path)
            with self.assertRaises(installer.InstallError):
                installer.fingerprint(link, os.geteuid())

    def test_failed_or_concurrent_install_preserves_file(self):
        path = self.root / "core_menu"
        path.write_bytes(b"old")
        with mock.patch.object(installer, "package_managed", return_value=False):
            expected = installer.fingerprint(path, os.geteuid())
            with mock.patch.object(installer.os, "replace", side_effect=OSError("failure")):
                with self.assertRaises(OSError):
                    installer.install(path, expected, self.binary, os.geteuid())
            self.assertEqual(path.read_bytes(), b"old")
            lock_path = self.root / ".core_menu.update.lock"
            with lock_path.open("r+b") as lock:
                installer.fcntl.flock(lock, installer.fcntl.LOCK_EX | installer.fcntl.LOCK_NB)
                with self.assertRaises(BlockingIOError):
                    installer.install(path, expected, self.binary, os.geteuid())
            self.assertEqual(path.read_bytes(), b"old")

    def test_sync_failure_restores_previous_binary(self):
        path = self.root / "core_menu"
        path.write_bytes(b"old")
        real_fsync = os.fsync
        calls = 0

        def fail_after_replacement(fd):
            nonlocal calls
            calls += 1
            if calls == 4:
                raise OSError("simulated directory sync failure")
            return real_fsync(fd)

        with mock.patch.object(installer, "package_managed", return_value=False):
            expected = installer.fingerprint(path, os.geteuid())
            with mock.patch.object(installer.os, "fsync", side_effect=fail_after_replacement):
                with self.assertRaises(installer.InstallError):
                    installer.install(path, expected, self.binary, os.geteuid())
            self.assertEqual(path.read_bytes(), b"old")

    def test_initial_user_install_and_confirmed_downgrade(self):
        home = self.root / "home"
        home.mkdir()
        path = home / ".local/bin/core_menu"
        existing = b'#!/bin/sh\nprintf "core_menu 9.0.0\\n"\n'
        release = ("v1.2.3", b"manifest", b"signature", self.binary)
        with mock.patch.object(installer.Path, "home", return_value=home), \
             mock.patch.object(installer, "package_managed", return_value=False), \
             mock.patch.object(installer, "fetch_release", return_value=release), \
             mock.patch.object(installer.os, "geteuid", return_value=os.geteuid() or 1000), \
             mock.patch.object(sys, "argv", ["install_release.py"]), \
             mock.patch("builtins.input", side_effect=["1"]):
            installer.main()
            self.assertEqual(path.read_bytes(), self.binary)
        path.write_bytes(existing)
        path.chmod(0o755)
        with mock.patch.object(installer.Path, "home", return_value=home), \
             mock.patch.object(installer, "package_managed", return_value=False), \
             mock.patch.object(installer, "fetch_release", return_value=release), \
             mock.patch.object(sys, "argv", ["install_release.py"]), \
             mock.patch("builtins.input", side_effect=["1", "s", "n"]):
            with self.assertRaises(installer.InstallError):
                installer.main()
            self.assertEqual(path.read_bytes(), existing)
        with mock.patch.object(installer.Path, "home", return_value=home), \
             mock.patch.object(installer, "package_managed", return_value=False), \
             mock.patch.object(installer, "fetch_release", return_value=release), \
             mock.patch.object(sys, "argv", ["install_release.py"]), \
             mock.patch("builtins.input", side_effect=["1", "s", "s"]):
            installer.main()
            self.assertEqual(path.read_bytes(), self.binary)

    def test_system_helper_rechecks_signed_contents(self):
        data, signature = self.signed()
        for name, content in (("manifest", data), ("signature", signature), ("binary", self.binary)):
            (self.root / name).write_bytes(content)
        with mock.patch.object(installer.os, "geteuid", return_value=0), \
             mock.patch.object(installer.platform, "machine", return_value="x86_64"), \
             mock.patch.object(installer, "system_directory"), \
             mock.patch.object(installer, "install", return_value=None) as install, \
             mock.patch.dict(os.environ), \
             mock.patch.object(installer, "PUBLIC_KEY", self.public_key):
            installer.helper(str(self.root), "v1.2.3", installer.TARGETS["x86_64"], "null")
            install.assert_called_once()
            (self.root / "binary").write_bytes(b"changed")
            with self.assertRaises(installer.InstallError):
                installer.helper(str(self.root), "v1.2.3", installer.TARGETS["x86_64"], "null")
            install.assert_called_once()

    def test_system_install_uses_isolated_helper(self):
        release = ("v1.2.3", b"manifest", b"signature", self.binary)
        with mock.patch.object(installer, "system_directory"), \
             mock.patch.object(installer, "fingerprint", return_value=None), \
             mock.patch.object(installer, "fetch_release", return_value=release), \
             mock.patch.object(installer, "helper_source", return_value="helper source"), \
             mock.patch.object(installer.shutil, "which", return_value="/usr/bin/sudo"), \
             mock.patch.object(installer.subprocess, "run") as run, \
             mock.patch.object(sys, "argv", ["install_release.py"]), \
             mock.patch("builtins.input", return_value="2"):
            run.return_value.returncode = 0
            installer.main()
            args = run.call_args.args[0]
            self.assertEqual(args[:6], ["sudo", "--", "/usr/bin/python3", "-I", "-c", "helper source"])
            self.assertEqual(args[6], "--system-helper")

    def test_package_managed_destination_is_not_replaced(self):
        path = self.root / "core_menu"
        path.write_bytes(b"old")
        with mock.patch.object(installer, "package_managed", return_value=True):
            with self.assertRaises(installer.InstallError):
                installer.fingerprint(path, os.geteuid())
        self.assertEqual(path.read_bytes(), b"old")

    def test_dependency_install_requires_confirmation(self):
        tools = self.root / "tools"
        tools.mkdir()
        for name, contents in (("uname", "echo Linux"), ("apt-get", "exit 0")):
            path = tools / name
            path.write_text("#!/bin/sh\n" + contents + "\n")
            path.chmod(0o755)
        script = Path(__file__).with_name("install.sh")
        result = subprocess.run(["/bin/sh", str(script)], input="n\n", text=True,
                                capture_output=True, env={"PATH": str(tools)})
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Instalar", result.stdout)
        self.assertIn("cancelada", result.stderr)

    def test_dependency_install_arguments_and_recheck(self):
        cases = (
            ("apt-get", ["install", "python3", "curl", "openssh-client", "coreutils", "grep"]),
            ("dnf", ["install", "python3", "curl", "openssh-clients", "coreutils", "grep"]),
            ("pacman", ["-S", "--needed", "python", "curl", "openssh", "coreutils", "grep"]),
        )
        for manager, arguments in cases:
            with self.subTest(manager=manager):
                tools = self.root / manager
                tools.mkdir()
                for name, contents in (
                    ("uname", "echo Linux"), ("id", "echo 1000"),
                    (manager, "exit 0"), ("sudo", 'printf "%s\\n" "$@" > "$TEST_LOG"'),
                ):
                    path = tools / name
                    path.write_text("#!/bin/sh\n" + contents + "\n")
                    path.chmod(0o755)
                log = self.root / (manager + "-args")
                script = Path(__file__).with_name("install.sh")
                result = subprocess.run(["/bin/sh", str(script)], input="s\n", text=True,
                                        capture_output=True, env={"PATH": str(tools), "TEST_LOG": str(log)})
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(log.read_text().splitlines(), ["--", manager] + arguments)
                self.assertIn("siguen faltando", result.stderr)

    def test_staged_file_limits_and_links(self):
        path = self.root / "stage"
        path.write_bytes(b"1234")
        self.assertEqual(installer.read_staged(path, 4), b"1234")
        with self.assertRaises(installer.InstallError):
            installer.read_staged(path, 3)
        link = self.root / "link"
        link.symlink_to(path)
        with self.assertRaises(OSError):
            installer.read_staged(link, 4)

    def test_system_helper_source_rejects_writable_or_symlinked_script(self):
        path = self.root / "installer.py"
        path.write_text("test source")
        path.chmod(0o644)
        with mock.patch.object(installer, "__file__", str(path)):
            self.assertEqual(installer.helper_source(), "test source")
            path.chmod(0o664)
            with self.assertRaises(installer.InstallError):
                installer.helper_source()
        link = self.root / "link"
        link.symlink_to(path)
        with mock.patch.object(installer, "__file__", str(link)):
            with self.assertRaises(OSError):
                installer.helper_source()

    def test_isolated_helper_requires_elevation(self):
        source = Path(installer.__file__).read_text()
        result = subprocess.run(
            ["/usr/bin/python3", "-I", "-c", source, "--system-helper", str(self.root),
             "v1.2.3", installer.TARGETS["x86_64"], "null"],
            text=True, capture_output=True,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("requiere privilegios", result.stderr)


if __name__ == "__main__":
    unittest.main()
