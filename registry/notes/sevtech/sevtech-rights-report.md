# SevTech: Ages 3.2.3 — rights resolution of the WP0 `absent` inputs

## WP7 correction of mutable WP0 inputs

The original triage below is retained as a dated investigation, not current clearance. Rechecking the cited raw
commit bytes found two Windows-1252 licence files whose original SHA-256 values had been computed after UTF-8
transcoding: Blood Magic and Nature's Compass. The corrected receipts hash the raw response bytes and bind the
Git blob identity. Repository links without artifact-specific attribution are unresolved rather than inferred.
Ten Modrinth observations are fresh receipts, not reconstructed historical responses.
Licence-file grants additionally require fresh commit-and-path GitHub contents receipts whose raw response,
path and Git blob identity match the cited bytes. Restrictive clauses absent from a claimed licence declaration
refuse clearance; a repository redirect alone is not artifact attribution.

The producer also recounts surface evidence from actual jar entries and rebinds graph evidence to the recorded
reference ledger bytes. These are corrections to open WP0 inputs; no closed milestone results are rewritten.
Regenerated source-use totals are **31 permitted, 38 conditional, 18 prohibited and 172 unresolved (259 total)**.
Full byte reconciliation requires the acquired fixtures; the offline gate checks vocabulary, policy, ledger
identity and derived totals only.

## Files changed

- `registry/inventory/sevtech-ages-3.2.3/rights.json`: rewritten to the ATM10 rights-triage shape. Each entry has
  exactly the ATM10 keys ((command below)),

  ```console
  acquisition, artifact, class, licence{assetsReservedSeparately, declared, family}, mods, publication, reference, rights, sourceUse}
  ```

  and the top level has the ATM10 keys ((command below)).

  ```console
  artifacts, derivation{decompiled, producedBy, reads, restrictedContentRead, statusVocabulary}, fixture{artifactSet{count, sha256}, id}, inherited, inventoryKind, schemaVersion, totals
  ```

  It contains no local paths. Size: 804,076 B before, **289,085 B** after.

- `registry/inventory/sevtech-ages-3.2.3/rights-evidence.json` (new): **607,608 B**, one line per record. It is keyed by
  graph node id: each artifact's first `graph.json` node in graph order, with `aliases` mapping the 14 secondary node
  ids of multi-mod jars. It holds everything that moved out of rights.json (artifact SHA-256, acquisition URL/status,
  surface counts, the WP0 jar legal-entry and cfwidget digests, now jar-relative rather than fixture paths). It also
  holds WP0 derivation limitations, the artifact-set digest definition, per-surface tallies, and for every
  formerly-absent input either a resolution citation or the search trail behind the absent finding. All 273 keys and
  aliases match current `graph.json` node ids.
- `registry/provenance/sevtech-ages-3.2.3.json`: rights-source locator/use, per-surface `basis` counts, and publication
  notes updated. Aggregate statuses are unchanged and were asserted equal to the worst per-input status.

Nothing was committed. No other file was touched.

## Method

For each of the 235 inputs whose jar declares no licence:

1. **Repository discovery**: URLs in the jar's mcmod.info; repository links in the cfwidget description; Modrinth
   `POST /v2/version_files` by jar SHA-1 (71 matched); GitHub repository search by mod name and targeted queries;
   user/org repository listings; GitHub code search for the mod id; redirects followed (e.g.
   `TehNut/Harvest → TehNut-Mods/Harvest`, `thraaawn/BonsaiTrees → davenonymous/BonsaiTrees`).
   `https://www.curseforge.com/api/v1/mods/{id}` returned **HTTP 403 for all 72 still-absent projects** (re-probed
   2026-10-16). The `api.curse.tools` mirror answers with a Cloudflare 302 challenge.
2. **Ref closest to the pinned jar**, in this order: a tag containing the jar version at a version boundary (31
   resolved); otherwise the newest commit no later than 12 h after the CurseForge file's `dateCreated`, preferring
   `*1.12*` branches (120); the first commit after the upload, only on a 1.12-line branch, when no earlier commit exists
   (1: SpartanShields, Apache-2.0 on the `1.12.2` branch, pushed 2023); the branch head for the bundled override
   SevTweaksNPC, which has no publisher date (1).
3. **Grant**:
   - the LICENSE/LICENCE/COPYING at that commit (GitHub `/license?ref=`, then a root listing), with path, blob SHA,
     content SHA-256 and a commit-pinned URL (137);
   - otherwise a README licence statement at the same commit (16);
   - otherwise the Modrinth project licence when Modrinth serves the **exact** jar by SHA-1 (10: BetterBedrock,
     environmentaltech, simplegenerators, valkyrielib → ARR; EnchantmentDescriptions, Wawla → LGPL-2.1-only; Neat →
     CC-BY-NC-SA-3.0; TinkersComplement, mcjtylib → MIT; walljump → GPL-3.0-or-later).
4. **Classification**: the existing `bucket()`/`code_status()`/`asset_status()` rules from
   `registry/checks/rights-model.sh`, reimplemented and checked against every row of `licence-families.tsv` (0
   mismatches). The 24 inputs that already had a jar declaration were reproduced byte-for-byte in statuses. One hand
   reading departs from `bucket()`: `antiqueatlas-1.12.2-4.5.1.jar` (GPL-3.0-or-later code, CC BY-NC-SA 3.0 textures) is
   `strong-copyleft` with `assetsReservedSeparately: true`, so code is permission-required and the textures are
   prohibited. `bucket()` on the combined string would have made the code prohibited too.

**Not counted as grants**:

- Forge MDK licence files (EmberRootZoo, spatialservermod, CompactMachines 3).
- Licence files that only appear after the pinned version. These are recorded as `laterLicence`: Ceramics, Farseek,
  Streams, Placebo, RecurrentComplex, bdlib, generators → MIT; MouseTweaks → BSD-3-Clause; mobends, QuickLeafDecay → MIT
  and simpletomb → LGPL-2.1 on the same branch head.
- Repositories whose closest commit is a different Minecraft line from years later: armoreablemobs (1.19, 2022),
  bonsaitrees (1.21.1, 2025), Uppers (1.20).
- Repository-level licence metadata at HEAD.

## Tallies

| Licence family                 | Before (WP0) |   After |
| ------------------------------ | -----------: | ------: |
| absent                         |          235 |      72 |
| permissive                     |            0 |      91 |
| weak-copyleft                  |           16 |      46 |
| strong-copyleft                |            0 |      16 |
| attribution-required           |            0 |       2 |
| noncommercial-or-noderivatives |            5 |      12 |
| all-rights-reserved            |            1 |      10 |
| custom                         |            2 |       9 |
| reference-only                 |            0 |       1 |
| source-restricted              |            0 |       0 |
| **Total**                      |      **259** | **259** |

Of the 163 resolved inputs: 91 permissive, 30 weak-copyleft, 16 strong-copyleft, 9 all-rights-reserved, 7
noncommercial-or-noderivatives, 7 custom, 2 attribution-required, 1 reference-only.

| Category | Before | After |
| --- | --- | --- |
| sourceUse | 0 permitted, 16 conditional, 6 prohibited, 237 unresolved | **31 permitted, 38 conditional, 18 prohibited, 172 unresolved** |
| code | 16 permission-required, 6 prohibited, 237 unresolved | 31 permitted, 38 conditional, 18 prohibited, 172 unresolved |
| textures | 96 n/a, 6 permission-required, 5 prohibited, 152 unresolved | 64 cleared, 96 n/a, 38 permission-required, 18 prohibited, 43 unresolved |
| models | 121 n/a, 4 permission-required, 3 prohibited, 131 unresolved | 52 cleared, 121 n/a, 32 permission-required, 13 prohibited, 41 unresolved |
| sounds | 208 n/a, 3 prohibited, 48 unresolved | 22 cleared, 208 n/a, 8 permission-required, 6 prohibited, 15 unresolved |
| names / branding / behaviour | 259 unresolved / 199 n/a, 60 prohibited / 259 cleared | unchanged |
| publication | 0 cleared, 64 blocked:prohibited, 195 blocked:unresolved | 0 cleared, **75 blocked:prohibited, 184 blocked:unresolved** |

Publication stays 0 cleared because `names` is unresolved for every input, which is the inherited rule.

## Unresolved: still `absent` (72)

Each one's full search trail is in `rights-evidence.json` under `resolution.trail`: mcmod.info URLs, cfwidget links,
CurseForge API status, Modrinth hash result, GitHub repository/user/code searches with their results, and any
repository/ref examined.

| Artifact                                          | Why still absent                                                                                                                                                             |
| ------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A Block of Charcoal-1.2.jar                       | `TheTurkeyDev/A-Block-Of-Charcoal` @ `dev_1.12` `7b4906693b` (last-commit-before-file-upload): no licence file or README statement                                           |
| ActuallyBaubles-1.12-1.1.jar                      | `JackyyTV/ActuallyBaubles` @ `1.12-1.1` `858ebbbc46` (tag-matching-jar-version): no licence file or README statement                                                         |
| Animalium-0.3.8.jar                               | `vadis365/Animalium` @ `1.12` `070086e63f` (first-commit-after-file-upload): no licence file or README statement                                                             |
| Aroma1997Core-1.12.2-2.0.0.2.jar                  | no source repository found (mcmod.info, cfwidget, Modrinth hash, repo/user/code search)                                                                                      |
| AromaBackup-1.12.2-2.1.1.4.jar                    | no source repository found (mcmod.info, cfwidget, Modrinth hash, repo/user/code search)                                                                                      |
| BNBGamingCore-1.12.2-0.12.0.jar                   | no source repository found (mcmod.info, cfwidget, Modrinth hash, repo/user/code search)                                                                                      |
| BNBGamingLib-1.12.2-2.17.6.jar                    | no source repository found (mcmod.info, cfwidget, Modrinth hash, repo/user/code search)                                                                                      |
| BetterBuildersWands-1.12.2-0.13.2.271+5997513.jar | `portablejim/BetterBuildersWands` @ `1.12` `59975138a3` (last-commit-before-file-upload): no licence file or README statement                                                |
| BetterWithLib-1.12-1.5.jar                        | no repository carries the `betterwithmods/library` package; upstream BetterWithMods org is HTTP 404                                                                          |
| Ceramics-1.12-1.3.7.jar                           | `KnightMiner/Ceramics` @ `1.3.7` `d7a0cd8faf` (tag-matching-jar-version): no licence file or README statement; a MIT licence file appears only later                         |
| DeathCompass-0.0.3.jar                            | no source repository found (mcmod.info, cfwidget, Modrinth hash, repo/user/code search)                                                                                      |
| DimensionalControl-1.12.2-2.13.0.jar              | no source repository found (mcmod.info, cfwidget, Modrinth hash, repo/user/code search)                                                                                      |
| EmberRootZoo-1.12.2-1.3.10.jar                    | only Forge MDK files (LICENSE-new.txt, CREDITS-fml.txt, Paulscode notices) at the resolved commit; those license Forge, not this mod                                         |
| Farseek-1.12-2.5.jar                              | `delvr/Farseek` @ `v1.12-2.5` `31f786bb2e` (tag-matching-jar-version): no licence file or README statement; a MIT licence file appears only later                            |
| FatCat-0.0.5.jar                                  | no source repository found (mcmod.info, cfwidget, Modrinth hash, repo/user/code search)                                                                                      |
| Ferdinand's Flowers-1.12.x-1.10.4.1.jar           | no source repository found (mcmod.info, cfwidget, Modrinth hash, repo/user/code search)                                                                                      |
| GobbleCore-1.12-0.1.6.35.jar                      | `TheTurkeyDev/GobbleCore` @ `mc_1.12` `54995540ab` (last-commit-before-file-upload): no licence file or README statement                                                     |
| ImprovedBackpacks-1.12.2-1.5.0.0.jar              | no source repository found (mcmod.info, cfwidget, Modrinth hash, repo/user/code search)                                                                                      |
| JustAFewFish-1.7_for_1.12.jar                     | no source repository found (mcmod.info, cfwidget, Modrinth hash, repo/user/code search)                                                                                      |
| JustEnoughPetroleum-0.1.jar                       | `DaedalusGame/JustEnoughPetroleum` @ `master` `1e4ee0ffcb` (first-commit-after-file-upload): no licence file or README statement                                             |
| KleeSlabs_1.12.2-5.4.12.jar                       | `TwelveIterations/KleeSlabs` @ `1.12` `0110955058` (last-commit-before-file-upload): no licence file or README statement; a NOASSERTION licence file appears only later      |
| LibEx-1.12.2-1.0.8.jar                            | `LogicTechCorp/LibraryEx` @ `1.12.x` `84699d4880` (last-commit-before-file-upload): no licence file or README statement                                                      |
| LoadingScreens-1.12.2-0.3.1.jar                   | no source repository found (mcmod.info, cfwidget, Modrinth hash, repo/user/code search)                                                                                      |
| MPUtils-1.12.2-1.5.7.jar                          | `GenDeathrow/MPUtils` @ `1.12.x` `69db0a1a38` (last-commit-before-file-upload): no licence file or README statement                                                          |
| MagmaMonsters-0.3.0.jar                           | `vadis365/MagmaMonsters` @ `1.12` `f363cc1d8a` (last-commit-before-file-upload): no licence file or README statement                                                         |
| Mercurius-1.12.2.jar                              | no source repository found (mcmod.info, cfwidget, Modrinth hash, repo/user/code search)                                                                                      |
| MobGrindingUtils-0.3.13.jar                       | `vadis365/Mob-Grinding-Utils` @ `MC1.12` `cc802f171f` (last-commit-before-file-upload): no licence file or README statement                                                  |
| Morpheus-1.12.2-3.5.106.jar                       | `Quetzi/Morpheus` @ `master` `7e32e345b6` (last-commit-before-file-upload): no licence file or README statement                                                              |
| MouseTweaks-2.10-mc1.12.2.jar                     | `YaLTeR/MouseTweaks` @ `minecraft-1.12.2` `ad6f36c544` (last-commit-before-file-upload): no licence file or README statement; a BSD-3-Clause licence file appears only later |
| NetherEx-1.12.2-2.0.8.jar                         | `LogicTechCorp/NetherEx-Archive` @ `1.12.x` `3c49180e91` (last-commit-before-file-upload): no licence file or README statement                                               |
| OreExcavation-1.4.150.jar                         | `Funwayguy/OreExcavation` exists but is an empty repository                                                                                                                  |
| Placebo-1.12.2-1.6.0.jar                          | `Shadows-of-Fire/Placebo` @ `1.12` `9f62f38ab6` (last-commit-before-file-upload): no licence file or README statement; a MIT licence file appears only later                 |
| PrimalTech-0.3.5.jar                              | `vadis365/PrimalTech` @ `1.12` `18f8fbc580` (last-commit-before-file-upload): no licence file or README statement                                                            |
| ProgressionTweaks-1.12.2-0.3.40.jar               | `TheTurkeyDev/Progression-Tweaks` @ `1.12` `1d1acd25f4` (last-commit-before-file-upload): no licence file or README statement                                                |
| Quark-r1.5-158.jar                                | `VazkiiMods/Quark` @ `1.12` `2ae18e8059` (last-commit-before-file-upload): no licence file or README statement; a NOASSERTION licence file appears only later                |
| QuickLeafDecay-MC1.12.1-1.2.4.jar                 | `lumien231/QuickLeafDecay` @ `master` `78047acaa4` (first-commit-after-file-upload): no licence file or README statement; a MIT licence file appears only later              |
| RealisticItemDrops-1.2.14.jar                     | `Funwayguy/RealisticItemDrops` @ `1.12` `f3ed2df80c` (last-commit-before-file-upload): no licence file or README statement                                                   |
| RecurrentComplex-1.4.8.2.jar                      | `Ivorforce/RecurrentComplex` @ `1.12` `81be8b834a` (last-commit-before-file-upload): no licence file or README statement; a MIT licence file appears only later              |
| ResourceLoader-MC1.12.1-1.5.3.jar                 | `lumien231/Resource-Loader` @ `master` `e82500eb51` (last-commit-before-file-upload): no licence file or README statement                                                    |
| SmoothFont-mc1.12.2-2.1.2.jar                     | no source repository found (mcmod.info, cfwidget, Modrinth hash, repo/user/code search)                                                                                      |
| Streams-1.12-0.4.8.jar                            | `delvr/Streams` @ `v1.12-0.4.8` `b1b580e497` (tag-matching-jar-version): no licence file or README statement; a MIT licence file appears only later                          |
| The Beneath-1.12.2-1.5.1.jar                      | `Shinoow/The-Beneath` @ `1.12.2` `0604c96947` (last-commit-before-file-upload): no licence file or README statement                                                          |
| Triumph-1.12.2-3.19.2.jar                         | no source repository found (mcmod.info, cfwidget, Modrinth hash, repo/user/code search)                                                                                      |
| Uppers-0.0.6.jar                                  | closest commit is on the 1.20 branch, not the 1.12 line, and carries no licence file                                                                                         |
| ViesCraft-1.12.2-5.9.16.jar                       | no source repository found (mcmod.info, cfwidget, Modrinth hash, repo/user/code search)                                                                                      |
| WaterControlExtreme-1.0.2.jar                     | `InnovativeOnlineIndustries/WaterControlExtreme` @ `master` `58cf18706f` (first-commit-after-file-upload): no licence file or README statement                               |
| WaterStrainer-1.12-3.2.0.jar                      | no source repository found (mcmod.info, cfwidget, Modrinth hash, repo/user/code search)                                                                                      |
| WildCrops-1.12-1.0.1.jar                          | no source repository found (mcmod.info, cfwidget, Modrinth hash, repo/user/code search)                                                                                      |
| [1.12]MoVillages-1.5.4.jar                        | no source repository found (mcmod.info, cfwidget, Modrinth hash, repo/user/code search)                                                                                      |
| armoreablemobs-1.12.2-1.1.8.jar                   | repository history starts 2022-04 on 1.19-MultiLoader (MIT, copyright 2022 Witixin1512); no 1.12 source or licence near the pinned 2019 file                                 |
| bdlib-1.14.3.12-mc1.12.2.jar                      | `bdew-minecraft/bdlib` @ `mc1.12` `c37db5723a` (last-commit-before-file-upload): no licence file or README statement; a MIT licence file appears only later                  |
| bonsaitrees-1.1.4-b170.jar                        | repository history starts 2025-05 on the 1.21.1 branch ("All rights reserved, Copyright (c) 2025 Davenonymous"); no 1.12 source near the pinned 2019 file                    |
| car-1.12.2-1.2.12.jar                             | `henkelmax/ultimate-car-mod` @ `outdated/1.12.2` `7c15a66510` (last-commit-before-file-upload): no licence file or README statement                                          |
| compactmachines3-1.12.2-3.0.18-b278.jar           | only Forge/FML licence files (LICENSE-fml.txt, MinecraftForge-License.txt) at the resolved commit; those license Forge, not this mod; README has no licence statement        |
| foamfix-0.10.10-1.12.2.jar                        | `asiekierka/FoamFix` @ `0.9.x-1.12.2` `aa40fffd4e` (last-commit-before-file-upload): no licence file or README statement                                                     |
| generators-0.9.20.12-mc1.12.2.jar                 | `bdew-minecraft/generators` @ `mc1.12` `2a14f2d63c` (last-commit-before-file-upload): no licence file or README statement; a MIT licence file appears only later             |
| immersivepetroleum-1.12.2-1.1.9.jar               | `Flaxbeard/ImmersivePetroleum` @ `1.12.2` `669f66e990` (last-commit-before-file-upload): no licence file or README statement                                                 |
| immersivetech-1.12-1.3.10.jar                     | `FerroO2000/Immersive-Tech` @ `Latest` `d55e118e0e` (last-commit-before-file-upload): no licence file or README statement                                                    |
| mobends-0.24_for_MC-1.12.jar                      | `mobends/MoBends` @ `1.X/forge-1.12` `4c807e89ca` (first-commit-after-file-upload): no licence file or README statement; a MIT licence file appears only later               |
| mpbasic-1.12.2-1.4.11.jar                         | `GenDeathrow/MPUtils` @ `1.12.x` `69db0a1a38` (last-commit-before-file-upload): no licence file or README statement                                                          |
| multiblockstages-1.2.0.jar                        | `The-Acronym-Coders/MultiBlock-Stages` @ `release/1.12` `c74e49b43b` (last-commit-before-file-upload): no licence file or README statement                                   |
| mystagradcompat-1.2.jar                           | `SenilePenguin/Mystical-Agradditions-Compat` @ `1.2` `d7682fd0d3` (tag-matching-jar-version): no licence file or README statement                                            |
| noRecipeBook_v1.2.2formc1.12.2.jar                | no source repository found (mcmod.info, cfwidget, Modrinth hash, repo/user/code search)                                                                                      |
| oeintegration-2.3.4.jar                           | `BloodyMods/OreExcavationIntegration` @ `1.12` `b142a65008` (last-commit-before-file-upload): no licence file or README statement                                            |
| playerskin-1.12.2-1.0.5.jar                       | `GenDeathrow/Player-Skin-Mobs` @ `master` `3cfdbe2bb6` (last-commit-before-file-upload): no licence file or README statement                                                 |
| roadrunner-1.0.1.jar                              | `Thretcha/RoadRunner` @ `master` `e3cb32815a` (last-commit-before-file-upload): no licence file or README statement                                                          |
| rustic-1.1.4.jar                                  | `cadaverous-eris/Rustic` @ `master` `cef65018dc` (last-commit-before-file-upload): no licence file or README statement                                                       |
| sevpatches-1.9-10.jar                             | `DarkPacks/SevPatches` @ `1.9` `afba1a4c27` (tag-matching-jar-version): no licence file or README statement                                                                  |
| simpleautorun-1.12.2-1.2.0.jar                    | no source repository found (mcmod.info, cfwidget, Modrinth hash, repo/user/code search)                                                                                      |
| simpletomb-1.12.2-1.0.0.jar                       | `Lothrazar/SimpleTomb` @ `trunk/1.12` `67475bb413` (first-commit-after-file-upload): no licence file or README statement; a LGPL-2.1 licence file appears only later         |
| spatialservermod-1.3.jar                          | only Forge MDK files (LICENSE-new.txt, CREDITS-fml.txt, Paulscode notices) at the resolved commit; those license Forge, not this mod                                         |
| zenloot-0.1-1.jar                                 | no source repository found (mcmod.info, cfwidget, Modrinth hash, repo/user/code search)                                                                                      |

## Caveats worth a reviewer's eye

- **Modrinth disagreements** (repo grant kept, Modrinth recorded as `modrinthCrossCheck`): ClientTweaks repo MIT vs
  Modrinth ARR; Waddles repo GPL-3.0 vs Modrinth ARR; RecipeStages repo LGPL-2.1 vs Modrinth MIT; BetterAdvancements
  same licence under a different id. Modrinth metadata is current, not historical.
- **Weaker citations**:
  - SpartanShields: the 1.12.2 branch was pushed in 2023, 4.6 years after the jar.
  - SevTweaksNPC: branch head; the bundled override has no upload date.
  - BetterWithMods: cited from `VanKaikai/BetterWithMods`, an independent copy of upstream history, because
    `BetterWithMods/BetterWithMods` is 404. Its 1.12 commit precedes the upload by nine minutes.
  - mpbasic and MPUtils both resolve to `GenDeathrow/MPUtils` (code search places the `mpbasic` id there), and both
    remain absent.
- **README-sourced grants (16)**: AbyssalCraft Integration, reauth, RedstoneFlux → ARR; Baubles, ExtraPlanets,
  GalacticraftTweaker, MJRLegendsLib → noncommercial; Just A Raft Mod, Mantle, Mekanism, TConstruct, Modular Routers,
  B.A.S.E → MIT; Iron Chests → GPLv3; The Betweenlands → custom terms; AutoRegLib → reference-only (per-class
  Botania/Psi/Quark/WTFPL pointers).
- **Declared string**: `licence.declared` for resolved inputs is the cited grant (SPDX id or exact licence title), not
  the jar's own string. The jar-level declaration (`""`) is kept in `rights-evidence.json` under `hostDeclaration`. This
  keeps declared→family a function, but `verify-rights.sh --fixture`'s "declared is the artifact's own declaration"
  check (blocked anyway on the Prism tree) would need the WP7 legacy model to read declarations from the evidence file.
- `restrictedContentRead` stays false. Only LICENSE/README files, repository metadata, and search result names/paths
  were read; no source files.

## Validation exercised (same commands as WP0)

| Command                                                                                                                 | Result                                                                                                                                                                                                             |
| ----------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `mise exec -- check-jsonschema --schemafile schemas/provenance.schema.json registry/provenance/sevtech-ages-3.2.3.json` | **exit 0**, `ok -- validation done`                                                                                                                                                                                |
| `mise exec -- env RAVEL_FIXTURE_ID=sevtech-ages-3.2.3 bash registry/checks/verify-inventory.sh --fixture`               | **exit 0**, `inventory reconciles: 254/254 pinned references, fixture digests bound`                                                                                                                               |
| `mise exec -- env RAVEL_FIXTURE_ID=sevtech-ages-3.2.3 bash registry/checks/verify-rights.sh --offline`                  | **exit 1**, same known global-decision-table issue as WP0, now narrowed from five unused families to one: `source-restricted -> prohibited` is stale for this fixture. No status or surface vocabulary violations. |
| `mise exec -- env RAVEL_FIXTURE_ID=sevtech-ages-3.2.3 bash registry/checks/verify-rights.sh --fixture`                  | **exit 1**, same Prism artifact-tree prerequisite as WP0                                                                                                                                                           |
| `mise exec -- env RAVEL_FIXTURE_ID=sevtech-ages-3.2.3 bash registry/checks/verify-provenance.sh --offline`              | **exit 1**, same missing `surfaces.json` as WP0                                                                                                                                                                    |
| `mise exec -- env RAVEL_FIXTURE_ID=atm10-tts-2.0.2 bash registry/checks/verify-rights.sh --offline`                     | **exit 0**, ATM10 unaffected                                                                                                                                                                                       |

Record-level jq equivalents of the `--fixture` checks that do not need the Prism tree passed on the new rights.json:
totals arithmetic, sourceUse follows code status, every record covers the seven surfaces, entry key set equals ATM10's,
every (family, code status) pair is decided in `licence-status.tsv`.

**Pending WP7 table rows.** These (declared, family, reserved) tuples are not yet in
`registry/checks/licence-families.tsv` (a shared file not in my scope). The `--fixture` family check will want them once
its Prism prerequisite is fixed:

- New from this pass: `GPL-3.0`, `GPL-3.0-or-later`, `LGPL-2.1`, `LGPL-2.1-only`, `LGPL-2.1-or-later`, `Unlicense`,
  `CC-BY-4.0`, `CC-BY-NC-SA-3.0`, `CC BY-NC-ND 4.0`, the natura/antiqueatlas split-licence strings,
  `All rights reserved (modified visible-source licence)`, `Angry Pixel Terms and Conditions`, and the AutoRegLib
  `See README…` pointer.
- Already missing at WP0: BiblioCraft, Galacticraft and the two Creative Commons long titles.
