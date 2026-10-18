#!/usr/bin/env python3
"""Dependency-free WP7 binding regressions; all mutations stay in temporary trees.

Run with: python3 -m unittest discover -s registry/sevtech -p 'test_wp7_*.py'
"""
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
import zipfile
from wp7_test_support import CHECKS, CHECK_TIMEOUT_SECONDS, jar_bytes, sha


class StandaloneArtifacts(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.fixture = "binding-test"
        self.local = self.root / "tests/fixtures/local" / self.fixture
        checks = self.root / "registry/checks"
        checks.mkdir(parents=True)
        helpers = self.root / "registry/sevtech"
        helpers.mkdir()
        for helper in ("artifact-set.sh", "artifact-set.py"):
            shutil.copyfile(CHECKS / helper, helpers / helper)
        # Only repository discovery is supplied by the miniature tree. The real
        # artifact enumerator and every byte/hash reconciliation run unchanged.
        (checks / "public-repo.sh").write_text(
            'repo_root="${WP7_TEST_ROOT}"\nregistry_root="$repo_root/registry"\nFIXTURE_ID=binding-test\n'
        )
        self.script = helpers / "artifact-set.sh"
        self.acquired = self.local / "jars/acquired.jar"
        self.bundled = self.local / "extracted/overrides/mods/bundled.jar"
        self.manifest = self.local / "extracted/manifest.json"
        self.archive = self.local / "source/binding-test.zip"
        for path in (self.acquired, self.bundled, self.manifest, self.archive):
            path.parent.mkdir(parents=True, exist_ok=True)
        content = jar_bytes()
        self.acquired.write_bytes(content)
        self.bundled.write_bytes(content)
        self.manifest.write_text(json.dumps({"files": [{"projectID": 17, "fileID": 23, "required": True, "isLocked": False}]}))
        with zipfile.ZipFile(self.archive, "w") as archive:
            archive.writestr("overrides/mods/bundled.jar", content)
        self.data = {
            "fixture": {"id": self.fixture, "archive": self.pin(self.archive), "manifest": self.pin(self.manifest)},
            "references": [{"projectId": 17, "fileId": 23, "required": True, "locked": False}],
            "acquiredArtifacts": [dict(self.artifact(self.acquired), projectId=17, fileId=23)],
            "bundledArtifacts": [dict(self.artifact(self.bundled), archiveEntry="overrides/mods/bundled.jar")],
        }
        self.ledger = self.root / "registry/inventory/binding-test/references.json"
        self.ledger.parent.mkdir(parents=True)

    def pin(self, path: Path) -> dict[str, str]:
        return {"path": str(path.relative_to(self.root)), "sha256": sha(path.read_bytes())}

    def artifact(self, path: Path) -> dict[str, str | int]:
        content = path.read_bytes()
        return dict(self.pin(path), artifact=path.name, status="fetched", size=len(content), sha1=sha(content, "sha1"))

    def run_binding(self) -> subprocess.CompletedProcess[str]:
        self.ledger.write_text(json.dumps(self.data))
        import os
        environment = dict(os.environ, WP7_TEST_ROOT=str(self.root))
        return subprocess.run(
            ["bash", "-c", 'source "$1"; test "$acquisition_mode" = standalone; test -s "$artifact_paths_tsv"', "binding-test", str(self.script)],
            env=environment, capture_output=True, text=True, check=False,
            timeout=CHECK_TIMEOUT_SECONDS,
        )

    def assert_refused(self) -> None:
        result = self.run_binding()
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_complete_standalone_population_is_accepted(self):
        result = self.run_binding()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_archive_hash_mismatch_is_refused(self):
        self.data["fixture"]["archive"]["sha256"] = "0" * 64
        self.assert_refused()

    def test_manifest_hash_mismatch_is_refused(self):
        self.data["fixture"]["manifest"]["sha256"] = "0" * 64
        self.assert_refused()

    def test_missing_acquired_jar_is_refused(self):
        self.acquired.unlink()
        self.assert_refused()

    def test_missing_bundled_jar_is_refused(self):
        self.bundled.unlink()
        self.assert_refused()

    def test_unrecorded_jar_is_refused(self):
        (self.acquired.parent / "extra.jar").write_bytes(jar_bytes())
        self.assert_refused()

    def test_reference_pair_mismatch_is_refused(self):
        self.data["acquiredArtifacts"][0]["fileId"] = 99
        self.assert_refused()


if __name__ == "__main__":
    unittest.main()
