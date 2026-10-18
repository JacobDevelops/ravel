"""Offline grant-text and derived rights/provenance reconciliation regressions."""
import json
import runpy
import unittest

from wp7_test_support import CHECKS, ROOT, RightsEvidenceFixture, sha


class RightsReconciliation(RightsEvidenceFixture):
    """Keep grant semantics and every derived permission bound to evidence."""

    def test_restrictive_grant_terms_cannot_be_dropped(self):
        matches = runpy.run_path(str(CHECKS / "rights-evidence.py"))["grant_text_matches"]
        mit = "Permission is hereby granted free of charge without restriction."
        cases = (
            ("CC-BY-4.0", "Creative Commons Attribution NonCommercial NoDerivatives 4.0"),
            ("CC-BY-4.0", "Creative Commons Attribution ShareAlike 4.0"),
            ("MIT", mit + " All Rights Reserved (assets)."),
            ("MIT", mit + " Non-commercial use only."),
            ("MIT", mit + " Commons Clause applies."),
            ("GPL", "GNU General Public License version 3"),
            ("GPL-3.0", "GNU Lesser General Public License version 3 with GNU General Public License references."),
        )
        for declared, text in cases:
            with self.subTest(declared=declared, text=text):
                self.assertFalse(matches(text, declared))
        self.assertTrue(matches("Redistribution and use in source and binary forms. Neither the name. All rights reserved.", "BSD-3-Clause"))

    def test_real_gpl_and_unlicense_texts_preserve_their_grants(self):
        matches = runpy.run_path(str(CHECKS / "rights-evidence.py"))["grant_text_matches"]
        fixtures = (
            ("gpl-2.0.txt", "GPL-2.0", "db296f2f7f35bca3a174efb0eb392b3b17bd94b341851429a3dff411b1c2fc73"),
            ("gpl-3.0.txt", "GPL-3.0", "c53a65c2fd561c87eaabf1072ef5dcab8653042bc15308465f52413585eb6271"),
            ("unlicense.txt", "Unlicense", "ca2abdf695884c77ea4b4a5b64ca7b732d9d9dbade4eebc1c2e76c53e9e3bc83"),
        )
        for filename, declared, digest in fixtures:
            content = (ROOT / "tests/fixtures/descriptors/licences" / filename).read_bytes()
            with self.subTest(declared=declared):
                self.assertEqual(sha(content), digest)
                self.assertTrue(matches(content.decode(), declared))
                if declared.startswith("GPL"):
                    self.assertFalse(matches(content.decode(), declared.replace("GPL", "LGPL")))

    def test_every_recorded_permission_and_total_is_reconciled(self):
        module = runpy.run_path(str(CHECKS / "rights-evidence.py"))
        self.record["surfaceEvidence"] = {"counts": {"textures": 1, "models": 0, "sounds": 0}}
        grants, acquisition = ["example.jar\tMIT\tpermissive\tfalse"], ["example.jar\tautomated-curseforge"]
        surfaces = ("code", "names", "behaviour", "branding", "textures", "models", "sounds")
        original = {"artifacts": [{"artifact": "example.jar", "rights": [{"surface": s} for s in surfaces]}]}
        expected = module["derived_rights"](original, grants, acquisition, self.evidence, {"example.jar": True})
        path = self.root / "rights.json"
        for field in (*surfaces, "sourceUse", "publication", "totals"):
            changed = json.loads(json.dumps(expected))
            if field in surfaces:
                next(r for r in changed["artifacts"][0]["rights"] if r["surface"] == field)["status"] = "forged"
            elif field == "totals":
                changed["totals"]["artifacts"] = 0
            else:
                changed["artifacts"][0][field] = "forged"
            path.write_text(json.dumps(changed))
            with self.subTest(field=field), self.assertRaisesRegex(SystemExit, "recorded (code permission|surface permissions)"):
                module["verify_recorded_rights"](path, grants, acquisition, self.evidence, {"example.jar": True})

    def test_provenance_summaries_are_regenerated_and_bound(self):
        module = runpy.run_path(str(CHECKS / "rights-evidence.py"))
        provenance = {
            "rights": [{"surface": "code", "status": "prohibited", "basis": "stale"}],
            "sources": [{"locator": "/rights-evidence.json", "use": "1 of the 1 jar-undeclared inputs resolved to a cited grant; 0 remain absent."}],
            "publication": {"notes": "permitted for 9 inputs under a cited permissive grant, conditional for 9, prohibited for 9 and unresolved for 9"},
        }
        records = {
            "artifacts": [{"rights": [{"surface": "code", "status": "cleared"}]}],
            "totals": {"sourceUse": {"permitted": 1, "conditional": 0, "prohibited": 0, "unresolved": 0}},
        }
        path = self.root / "provenance.json"
        path.write_text(json.dumps(provenance))
        reconcile = module["reconcile_provenance"]
        reconcile(path, records, self.evidence, True)
        updated = json.loads(path.read_text())
        self.assertEqual(updated["rights"][0]["status"], "cleared")
        self.assertIn("permitted for 1 inputs", updated["publication"]["notes"])
        self.assertIn("0 of the 1 jar-undeclared inputs", updated["sources"][0]["use"])
        for field in ("rights", "sources", "publication"):
            changed = json.loads(json.dumps(updated))
            if field == "rights":
                changed[field][0]["basis"] = "stale"
            elif field == "sources":
                changed[field][0]["use"] = changed[field][0]["use"].replace("0 of the 1", "1 of the 1")
            else:
                changed[field]["notes"] = changed[field]["notes"].replace("permitted for 1", "permitted for 9")
            path.write_text(json.dumps(changed))
            with self.subTest(field=field), self.assertRaisesRegex(SystemExit, "provenance rights summaries differ"):
                reconcile(path, records, self.evidence, False)


if __name__ == "__main__":
    unittest.main()
