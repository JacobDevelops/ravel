"""Offline preparation isolation and artifact integrity checks."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
import zipfile

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("bench_prepare", HERE / "prepare.py")
prepare = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(prepare)


class PreparationTests(unittest.TestCase):
    def test_publisher_overrides_reset_without_touching_worlds_or_mods(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            game = root / "game"
            for name in ("config", "scripts", "mods", "saves"):
                (game / name).mkdir(parents=True)
            (game / "config/generated.cfg").write_text("runtime mutation")
            (game / "scripts/pack.zs").write_text("runtime mutation")
            (game / "mods/pack.jar").write_bytes(b"verified original")
            (game / "saves/world.dat").write_bytes(b"preserved world")
            outside = root / "outside"
            outside.mkdir()
            (outside / "keep").write_text("untouched")
            (game / "resources").symlink_to(outside, target_is_directory=True)
            archive = root / "pack.zip"
            with zipfile.ZipFile(archive, "w") as source:
                source.writestr("overrides/config/pack.cfg", "publisher config")
                source.writestr("overrides/scripts/pack.zs", "publisher script")
                source.writestr("overrides/resources/pack.txt", "publisher resource")
                source.writestr("overrides/mods/pack.jar", b"not recopied")
                source.writestr("overrides/saves/world.dat", b"not recopied")
            prepare.reset_overrides(archive, game)
            self.assertFalse((game / "config/generated.cfg").exists())
            self.assertEqual((game / "config/pack.cfg").read_text(), "publisher config")
            self.assertEqual((game / "scripts/pack.zs").read_text(), "publisher script")
            self.assertFalse((game / "resources").is_symlink())
            self.assertEqual((outside / "keep").read_text(), "untouched")
            self.assertEqual((game / "mods/pack.jar").read_bytes(), b"verified original")
            self.assertEqual((game / "saves/world.dat").read_bytes(), b"preserved world")
            prepare.reset_overrides(archive, game)
            self.assertEqual((game / "scripts/pack.zs").read_text(), "publisher script")
            linked = root / "linked-game"
            linked.symlink_to(game, target_is_directory=True)
            with self.assertRaises(ValueError):
                prepare.reset_overrides(archive, linked)

    def test_shared_cache_links_become_private_copies(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            prism, shared = root / "prism", root / "shared"
            prism.mkdir()
            for name in ("libraries", "assets", "meta"):
                (shared / name).mkdir(parents=True)
                (shared / name / "input").write_text("original")
                (prism / name).symlink_to(shared / name, target_is_directory=True)
            prepare.configure_caches(prism, shared)
            for name in ("libraries", "assets", "meta"):
                self.assertFalse((prism / name).is_symlink())
                (prism / name / "input").write_text("private")
                self.assertEqual((shared / name / "input").read_text(), "original")
            prepare.configure_caches(prism, shared)
            self.assertEqual((prism / "meta/input").read_text(), "private")

    def test_artifacts_immutable_but_metadata_refreshes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            baseline = Path(directory)
            prism = baseline / "prism"
            for name in ("libraries", "assets", "meta"):
                (prism / name).mkdir(parents=True)
            artifact = prism / "libraries/mod.jar"
            artifact.write_bytes(b"pinned bytes")
            metadata = prism / "meta/index.json"
            metadata.write_text('{"url": "https://example.test/v1", "version": 1}')
            prepare.pin_downloads(baseline, prism, {})
            metadata.write_text('{"url": "https://example.test/v2", "version": 2}')
            prepare.pin_downloads(baseline, prism, {})
            pins = prepare.tomllib.loads((baseline / "inputs.toml").read_text())
            self.assertEqual(pins["metadata_sha256"]["prism/meta/index.json"], prepare.digest(metadata))
            self.assertEqual(pins["metadata_observed_urls"]["prism/meta/index.json"], ["https://example.test/v2"])
            artifact.write_bytes(b"changed bytes")
            with self.assertRaises(ValueError):
                prepare.pin_downloads(baseline, prism, {})

    def test_forge_extraction_directories_require_embedded_original_bytes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            game = Path(directory)
            mods = game / "mods"
            repository = mods / "memory_repo"
            repository.mkdir(parents=True)
            nested = prepare.io.BytesIO()
            manifest = b"Manifest-Version: 1.0\r\nMaven-Artifact: test:nested:1\r\n"
            with zipfile.ZipFile(nested, "w") as child:
                child.writestr("META-INF/MANIFEST.MF", manifest)
            host = mods / "host.jar"
            with zipfile.ZipFile(host, "w") as archive:
                archive.writestr("META-INF/libraries/nested.jar", nested.getvalue())
            extracted = repository / "nested.jar"
            extracted.write_bytes(nested.getvalue())
            (repository / "nested.jar.meta").write_bytes(manifest)
            artifacts = {"host.jar": (host, prepare.digest(host))}
            prepare.verify_mods(game, artifacts)
            extracted.write_bytes(b"modified jar")
            with self.assertRaises(ValueError):
                prepare.verify_mods(game, artifacts)
            extracted.write_bytes(nested.getvalue())
            repository.rename(mods / "1.12.2")
            prepare.verify_mods(game, artifacts)
            (mods / "1.12.2/nested.jar").write_bytes(b"modified legacy jar")
            with self.assertRaises(ValueError):
                prepare.verify_mods(game, artifacts)

    def test_cache_copy_rejects_unexpected_file_without_mutating_source(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            prism, shared = root / "prism", root / "shared"
            prism.mkdir()
            (shared / "libraries").mkdir(parents=True)
            (shared / "libraries/input").write_text("original")
            (prism / "libraries").write_text("unexpected")
            with self.assertRaises(ValueError):
                prepare.configure_caches(prism, shared)
            self.assertEqual((prism / "libraries").read_text(), "unexpected")
            self.assertEqual((shared / "libraries/input").read_text(), "original")

    def test_standard_options_disable_snooper(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            game = Path(directory)
            options = game / "options.txt"
            options.write_text("snooperEnabled:true\ncustomOption:keep\n")
            (game / ".benchmark-publisher-options.txt").write_text("publisherOption:keep\n")
            values = prepare.configure_options(game)
            self.assertEqual(values["snooperEnabled"], "false")
            self.assertIn("snooperEnabled:false\n", options.read_text())
            self.assertNotIn("customOption:keep\n", options.read_text())
            self.assertIn("publisherOption:keep\n", options.read_text())
            self.assertNotIn("snooperEnabled:true", options.read_text())
