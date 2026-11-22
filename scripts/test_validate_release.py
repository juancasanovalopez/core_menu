import json
from pathlib import Path
import tempfile
import unittest

from validate_release import validate_release


class ReleaseValidationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.write_versions("1.10.0")

    def write_versions(
        self,
        version: str,
        *,
        locked: str | None = None,
        manifested: str | None = None,
    ) -> None:
        (self.root / "Cargo.toml").write_text(
            f'[package]\nname = "core_menu"\nversion = "{version}"\n'
        )
        (self.root / "Cargo.lock").write_text(
            '[[package]]\nname = "core_menu"\n'
            f'version = "{locked if locked is not None else version}"\n'
        )
        (self.root / ".release-please-manifest.json").write_text(
            json.dumps({".": manifested if manifested is not None else version})
        )

    def test_numeric_progression_ignores_current_and_nonstable_tags(self) -> None:
        validate_release(
            self.root,
            "v1.10.0",
            ["v1.9.0", "v1.10.0", "v2.0.0-rc.1", "v2.0.0+build", "other"],
        )

    def test_bootstrap_without_previous_tags(self) -> None:
        validate_release(self.root, "v1.10.0", [])

    def test_inconsistent_version_files(self) -> None:
        for overrides in [
            {"version": "1.9.0"},
            {"version": "1.10.0", "locked": "1.9.0"},
            {"version": "1.10.0", "manifested": "1.9.0"},
        ]:
            with self.subTest(overrides=overrides):
                self.write_versions(**overrides)
                with self.assertRaises(ValueError):
                    validate_release(self.root, "v1.10.0", [])

    def test_only_stable_canonical_tags_are_allowed(self) -> None:
        for tag in [
            "1.10.0", "v01.10.0", "v1.010.0", "v1.10.00",
            "v1.10.0-rc.1", "v1.10.0+build.2", "v1.10", "v1.10.0\n",
        ]:
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                validate_release(self.root, tag, [])

    def test_no_downgrades_even_when_tag_order_is_unsorted(self) -> None:
        with self.assertRaisesRegex(ValueError, "v2.0.0"):
            validate_release(self.root, "v1.10.0", ["v1.0.0", "v2.0.0", "v1.9.0"])

    def test_registry_package_cannot_replace_the_local_lock_entry(self) -> None:
        with (self.root / "Cargo.lock").open("a") as source:
            source.write('source = "registry+https://example.invalid"\n')
        with self.assertRaisesRegex(ValueError, "Cargo.lock"):
            validate_release(self.root, "v1.10.0", [])


if __name__ == "__main__":
    unittest.main()
