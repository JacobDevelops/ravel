"""Run synthetic metadata tests: python3 -B -m unittest discover -s registry/waves."""

import sys

sys.dont_write_bytecode = True

import contextlib
import importlib.util
import pathlib
import tempfile
from io import StringIO
import unittest
from wave_test_support import patched_attribute

spec = importlib.util.spec_from_file_location(
    "modmeta", pathlib.Path(__file__).resolve().parents[1] / "checks" / "modmeta.py",
)
modmeta = importlib.util.module_from_spec(spec)
spec.loader.exec_module(modmeta)


class MetadataBoundaries(unittest.TestCase):
    def test_declaration_spellings_preserve_all_dependency_rows(self) -> None:
        header = b'[[mods]]\nmodId = "owner"\nversion = "1"\n'
        declarations = (
            b'[[dependencies.owner]]\nmodId = "target"\ntype = "required"\nside = "both"\n',
            b'[dependencies]\nowner = [{modId = "target", type = "required", side = "both"}]\n',
            b'[[dependencies]]\nmodId = "target"\ntype = "required"\nside = "both"\n',
        )
        for declaration in declarations:
            with self.subTest(declaration=declaration):
                out = modmeta.records(header + declaration, "synthetic")
                self.assertEqual(out.produced, 2)
                self.assertEqual(out.rows[-1], ("DEP", "owner", "target", "required", "", "BOTH"))

    def test_ownerless_dependency_is_refused_when_owner_is_ambiguous(self) -> None:
        with self.assertRaisesRegex(modmeta.Unreadable, "declares 2 mods"):
            modmeta.records(
                b'[[mods]]\nmodId = "a"\n[[mods]]\nmodId = "b"\n'
                b'[[dependencies]]\nmodId = "target"\n',
                "synthetic",
            )

    def test_malformed_dependency_shapes_fail_instead_of_dropping_rows(self) -> None:
        for declaration in (
            b'dependencies = 1\n',
            b'[dependencies]\nowner = 1\n',
            b'[dependencies]\nowner = [1]\n',
            b'[[dependencies.owner]]\nversionRange = "*"\n',
        ):
            with self.subTest(declaration=declaration):
                with self.assertRaises(modmeta.Unreadable):
                    modmeta.records(declaration, "synthetic")

    def test_mixed_case_fields_and_scalar_values_keep_loader_semantics(self) -> None:
        out = modmeta.records(
            b'MODLOADER = "javafml"\nLICENSE = "MIT"\n[[MODS]]\nMODID = "owner"\nVERSION = 1\n'
            b'[[DEPENDENCIES.owner]]\nMODID = "target"\nTYPE = "REQUIRED"\nSIDE = "client"\n',
            "synthetic",
        )
        self.assertEqual(out.produced, 4)
        self.assertEqual(out.rows[2], ("MOD", "owner", "1", "", "", ""))
        self.assertEqual(out.rows[3], ("DEP", "owner", "target", "required", "", "CLIENT"))

    def test_optional_declaration_lookup_is_not_a_suppressed_archive_failure(self) -> None:
        class Archive:
            def read(self, name: str) -> bytes:
                if name == modmeta.DECLARATION_NAMES[0]:
                    raise KeyError(name)
                return b"declared"

        self.assertEqual(modmeta.declaration(Archive()), b"declared")

        class BrokenArchive:
            def read(self, name: str) -> bytes:
                raise OSError("broken archive")

        with self.assertRaisesRegex(OSError, "broken archive"):
            modmeta.declaration(BrokenArchive())

    def test_nested_declarations_account_for_every_row(self) -> None:
        meta, ledger = StringIO(), StringIO()
        modmeta.emit_nested_declaration(
            b'modLoader = "javafml"\nlicense = "MIT"\n[[mods]]\nmodId = "inner"\n'
            b'[[mixins]]\nconfig = "inner.json"\n'
            b'[[dependencies.inner]]\nmodId = "target"\ntype = "required"\n',
            "outer!inner.jar", "outer", "inner.jar", meta, ledger,
        )
        self.assertEqual(meta.getvalue().splitlines(), [
            "outer\tNESTED\tinner.jar\tinner\t\t\t",
            "outer\tNESTEDDEP\tinner\ttarget\trequired\t\t",
        ])
        accounting = ledger.getvalue().splitlines()
        self.assertEqual(accounting[0], "PARSED\touter\touter!inner.jar\t5")
        self.assertEqual([line.split("\t")[3] for line in accounting[1:]], ["LOADER", "LICENSE", "MIXIN"])

    def test_instance_layout_preserves_artifact_kind_order(self) -> None:
        rows = [["resourcepack", "r.zip"], ["mod", "m.jar"], ["shaderpack", "s.zip"]]
        outputs = [StringIO() for _ in range(4)]
        calls = []

        def reader(path: str, filename: str, *streams: object) -> None:
            calls.append((path, filename))

        with (
            patched_attribute(modmeta, "read_mod", reader),
            patched_attribute(modmeta, "read_shaderpack", reader),
            patched_attribute(modmeta, "read_resourcepack", reader),
        ):
            modmeta.read_artifacts(rows, "/instance", *outputs)
            self.assertEqual(calls, [
                ("/instance/mods/m.jar", "m.jar"),
                ("/instance/shaderpacks/s.zip", "s.zip"),
                ("/instance/resourcepacks/r.zip", "r.zip"),
            ])

    def test_cli_reads_instance_layout_and_refuses_empty_artifact_table(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            artifacts = root / "artifacts.tsv"
            artifacts.write_text("mod\tm.jar\tproject\tversion\turl\tsha\n")
            argv = ["modmeta.py", str(artifacts), "/instance"] + [
                str(root / name) for name in ("meta.tsv", "entries.tsv", "ledger.tsv", "nested.tsv")
            ]
            calls = []

            def reader(path: str, filename: str, *streams: object) -> None:
                calls.append((path, filename))

            with patched_attribute(modmeta, "read_mod", reader):
                self.assertEqual(modmeta.main(argv), 0)
            self.assertEqual(calls, [("/instance/mods/m.jar", "m.jar")])
            artifacts.write_text("")
            diagnostic = StringIO()
            with contextlib.redirect_stderr(diagnostic):
                self.assertEqual(modmeta.main(argv), 1)
            self.assertIn("no acquired artifacts", diagnostic.getvalue())


if __name__ == "__main__":
    unittest.main()
