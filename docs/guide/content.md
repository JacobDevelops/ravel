# Declaring content

Content is declared in one place and everything else is derived from that place.
Write a `ContentDeclaration`, return it from `content_declaration`, and generate
the pack from the same one. The default `register` claims every declared entry
during the registration window, so registered blocks and tag references cannot drift apart.

```rust
impl CopperLanterns {
    fn declaration() -> Result<ContentDeclaration, ModError> {
        let mut declaration = ContentDeclaration::new(Self::declared_id().clone());
        let block = declaration
            .entry(ContentKind::Block, "copper_lantern")
            .map_err(ModError::from_display)?;
        declaration
            .entry(ContentKind::Item, "copper_lantern")
            .map_err(ModError::from_display)?;
        declaration
            .tag(&lanterns(ContentKind::Block), &block)
            .map_err(ModError::from_display)?;
        Ok(declaration)
    }
}

#[declare_mod(id = "copper_lanterns")]
impl Mod for CopperLanterns {
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        let _ = state;
        Ok(())
    }

    fn content_declaration(
        &self,
    ) -> Result<Option<std::borrow::Cow<'_, ContentDeclaration>>, ModError> {
        Self::declaration().map(std::borrow::Cow::Owned).map(Some)
    }
}
```

`lanterns` there is the mod's own helper naming a tag in the convention
namespace; [`examples/guide-snippets`](../../examples/guide-snippets) writes it
out.

Declaration builders return `Result` so malformed or conflicting packaged content
is reported through the fallible hook, rather than panicking during loading.

## The three kinds of declaration behave differently, on purpose

- **A registry entry has one owner.** A second claim on `block:you:copper_lantern`
  is a conflict the barrier refuses, and the refusal names who owns it.
- **A tag has no owner.** Every mod that makes an ingot puts it in `c:ingots`, and
  the members merge. Tags are the only place a mod legitimately writes outside
  its own namespace: the convention namespace is `c`, and a tag there is not a
  claim on it.
  Launch merges generated and supplied `data/<namespace>/tags/<registry>/*.json`
  documents in resolved load order. A validated pack retains all members using
  first occurrences across the resolved `Side::ALL` plans as its canonical
  cross-side order; common members therefore follow their resolved dependencies.
  Both paths apply the same rule: appending keeps existing member order and adds
  only values not already present. A supplied `replace: true` discards earlier
  members; later contributions still append, and the merged document retains that
  flag for the game's lower-priority packs. Unknown document fields come from the
  last contributor.
  Explicit replacements name discarded contributors in the launch
  `tag_replacements()` report and the validated pack's override report; ordinary
  tag appends are not overrides. A pack's own explicitly supplied file remains a
  complete override of member tags. Invalid tag JSON or tag field types are
  attributed to the supplying mod: required launch failures stop collection, while
  optional failures before world mutation discard that mod's entire declaration.
- **A resource path has one supplier.** Two mods supplying
  `assets/…/lang/en_us.json` is an override, not a conflict — the mod later in the
  resolved load order supplies it, and the mods it shadowed are recorded, because
  a player whose translation vanished is owed the name of the mod that took it.
  `content_declaration` above is how Ravel gets them: it asks every mod for its
  declaration once the set is admitted and layers the files in load order, so the
  bytes you supply reach the game without your mod naming a path on disk or a
  loader type. A mod that claims its names and never hands the declaration back
  registers content the game has no files for.

## The registration window is a window

The loader calls `register` once and closes the barrier afterwards. A mod that
claims nothing there claims nothing ever, and a mod that stashes the registrar
and claims later is refused as *late* rather than as contested. Claim in
`register` or not at all.

The default `register` uses `claim_all` and treats a conflict as fatal, because
content you could not claim is content you cannot then use. A mod with a genuine
fallback overrides `register`, claims key by key through `Registrar::claim`, and
decides each refusal itself.

## What is generated, and what is yours

Generated paths are derived, never spelled. A tag lands at
`data/<namespace>/tags/<registry>/<path>.json` and a locale at
`assets/<namespace>/lang/<locale>.json`. You cannot misfile your own generated
data, and two declarations that would write one file are refused when the
declaration is made rather than when the file is written.

What is *not* generated is anything whose shape a Minecraft version decides: a
recipe body, a model, a translation key's spelling. Those you supply as bytes
through `ContentDeclaration::resource`, and they travel in the same output, so a
mod's pack is one set rather than a generated half and a hand-written half that
can disagree about a path.

Generation is deterministic: the same declaration produces the same bytes, and
the same content declared in a different order produces the same bytes too.

## Ravel writes the files your content needs and you did not write

A block is three files, not one. Beside the model you wrote, a version wants a
blockstates file mapping the block's one variant to that model, an item model so
the block can be held and seen in an inventory, and a loot table so breaking it
drops something. None of those has a choice in it for a block with one state, so
Ravel writes them from the declaration: a blockstates file pointing at
`<you>:block/<name>`, an item model parented at that block for an item sharing
its name and a flat sprite for one that does not, and a loot table that drops the
block itself.

Supply any of those paths yourself and yours is the one in the pack. That is the
same rule as everywhere else here — you declare, Ravel derives, and anything you
write by hand wins — and `ContentDeclaration::supplied_defaults` names the ones
you took back.

A recipe is the exception, and it is the exception on purpose. Ravel writes no
recipe body, because the shape of one is a Minecraft version's to decide and the
versions disagree about it: an ingredient is an object in one and a bare name in
the next. So `ContentKind::Recipe` claims the name and you supply the bytes
through `ContentDeclaration::server_entry`, which takes a server-data kind, a name
and the bytes. A recipe you declare and never write is a name nothing stands behind.

## The registries that exist

`ContentKind` is a closed set: `Block`, `Item`, `Recipe`, `Command`. If your mod
needs an entity, a fluid, a block entity or a biome, the SDK has no name for it
yet and no workaround will produce one.

**An item that shares a block's name places that block.** Declaring
`ContentKind::Block` and `ContentKind::Item` under one name, as the example
above does, is how you get a block you can hold: Ravel registers the item as the
block's placing item rather than as a second thing that happens to be named the
same, so right clicking it puts the block down. An item under a name no block
claims is a plain item and places nothing.

**A command belongs in the declaration like anything else.**
`ContentDeclaration::entry(ContentKind::Command, "lanterns")` is how you name one,
the default `register` claims it with the rest, and the mod that claimed the root literal is
the one `run_command` reaches — see
[The lifecycle, events, sides and packets](running.md). Two things differ from a
block. A command generates no file, because nothing about a command's behaviour
is data Ravel writes; the entry exists to claim the name. And a command cannot be
tagged: `TagId::new(ContentKind::Command, …)` is refused, because a tag groups
content something can look up and a command is not that.
