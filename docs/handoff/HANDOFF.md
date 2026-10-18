# Ravel handoff

This file preserves cross-session context, not a second project contract or a gate result.
Read `AGENTS.md` for repository rules, `SPEC.md` for required behavior, `ROADMAP.md`
for work status, and `docs/architecture/dependency-rules.md` for crate boundaries.
Session-specific lane reports and integration checklists were removed because their
unfinished-work claims contradicted later integration results. Current verification
must come from the integration owner's results for the current tree, not old counts.

## Runtime ownership context

- Bootstrap owns `Composition` and its installed-package versions. Runtime construction
  requires `LaunchContext`; installation consumes the composition and profiler and
  returns `InstallError` on refusal.
- Loader owns saved-data admission and callback attribution, outward-request dispatch,
  and game-pack replacement paths (`replaced_paths`). Bootstrap consumes these decisions;
  interception attribution remains in bootstrap because loader interception has no
  failure environment. `Round::refused_by` supplies save-veto ownership.
- Failed owners' saved payloads remain opaque. Installed package metadata does not
  substitute for the mod's actual readable saved-data declarations.

## Durable decisions and evidence boundaries

- SevTech: Ages 3.2.3 targets Minecraft 1.12.2 on the pinned modern JDK, with native
  behavioral counterparts rather than hosting Forge jars. Feather mappings, LWJGL3,
  and a replacement renderer are the selected direction. Keep the other supported
  version targets and historical ATM10 evidence.
- The placeholder test mods (`dependency-base`, `dependency-consumer`, `failure-cases`)
  were retained because M4 binds their inventory. The rvl-146 singleton was retained
  because its removal would alter M6-bound counts. SDK crate-root reexports remain.
- Historical evidence results and package source digests have producer-specific binding
  rules; consult their manifests and producer before changing them. Rejected historical
  changes are not new deletion opportunities.

## Research pointers

- `registry/notes/sevtech/sevtech-plan.md`: work-package design and dependencies.
- `registry/notes/sevtech/sevtech-wp0-report.md` and `sevtech-rights-report.md` in that
  directory: input-specific provenance, rights research, and disposition history.
- `jvm-crate-audit.md`: dated JVM/library research at its stated snapshot, not current
  implementation status or an instruction to repeat already completed migrations.
