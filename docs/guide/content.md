# Declaring content

Content is declared in one place and everything else is derived from that place.
Write a `ContentDeclaration`, claim from it during the registration window, and
generate the pack from the same one, so the block you register and the block your
tag names cannot drift apart.

```rust
impl CopperLanterns {
    fn declaration() -> ContentDeclaration {
        let mut declaration = ContentDeclaration::new(Self::declared_id().clone());
        let block = declaration
            .entry(ContentKind::Block, "copper_lantern")
            .expect("legal");
        declaration
            .entry(ContentKind::Item, "copper_lantern")
            .expect("legal");
        declaration
            .tag(&lanterns(ContentKind::Block), &block)
            .expect("blocks are taggable");
        declaration
    }
}

#[declare_mod(id = "copper_lanterns")]
impl Mod for CopperLanterns {
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        let _ = state;
        Ok(())
    }

    fn register(&mut self, registrar: &mut dyn Registrar) -> Result<(), ModError> {
        claim_all(&Self::declaration(), registrar)
    }

    fn content_declaration(&self) -> Option<ContentDeclaration> {
        Some(Self::declaration())
    }
}
```

`lanterns` there is the mod's own helper naming a tag in the convention
namespace; [`examples/guide-snippets`](../../examples/guide-snippets) writes it
out.

## The three kinds of declaration behave differently, on purpose

- **A registry entry has one owner.** A second claim on `block:you:copper_lantern`
  is a conflict the barrier refuses, and the refusal names who owns it.
- **A tag has no owner.** Every mod that makes an ingot puts it in `c:ingots`, and
  the members merge. Tags are the only place a mod legitimately writes outside
  its own namespace: the convention namespace is `c`, and a tag there is not a
  claim on it.
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

`claim_all` treats a conflict as fatal, because content you could not claim is
content you cannot then use. A mod with a genuine fallback claims key by key
through `Registrar::claim` and decides each refusal itself.

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
`claim_all` claims it with the rest, and the mod that claimed the root literal is
the one `run_command` reaches — see
[The lifecycle, events, sides and packets](running.md). Two things differ from a
block. A command generates no file, because nothing about a command's behaviour
is data Ravel writes; the entry exists to claim the name. And a command cannot be
tagged: `TagId::new(ContentKind::Command, …)` is refused, because a tag groups
content something can look up and a command is not that.
