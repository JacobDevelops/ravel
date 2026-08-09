# Settings, world data and services

Three kinds of state outlive a single call, and they are separate because they
belong to different people: settings belong to the player and the pack, world
data belongs to one save, and a service belongs to another mod.
[`examples/stateful-mod`](../../examples/stateful-mod) has all three.

## Settings are a document, never a screen

Configuration is declared in data and every Ravel-owned screen is drawn in Java.
You write JSON and read resolved values; you never write a widget.

```json
{
  "schemaVersion": 1,
  "owner": "copper_lanterns",
  "version": 1,
  "settings": [
    {
      "key": "lanterns.capacity",
      "title": "Lantern capacity",
      "description": "How many lanterns a stack holds.",
      "valueSchema": { "type": "integer", "minimum": 1.0, "maximum": 64.0 },
      "default": 16,
      "side": "common",
      "restartRequired": false
    }
  ]
}
```

`valueSchema` is typed in the schema as a bare object, which says nothing about
what may go in it. It is a closed set, and this is all of it: `"type"` is
`"boolean"`, `"integer"`, `"number"` or `"string"`, or absent; `"enum"` lists the
exact values permitted; `"minimum"` and `"maximum"` bound a number inclusively;
`"minLength"` and `"maxLength"` bound a string's length. **There is no array,
object or null setting** — a mod that wants a list makes it a string it parses
itself, or several settings. A keyword that does not apply to the value's type
constrains nothing, so a `"minimum"` beside a string says nothing about it.

`"integer"` means a JSON integer literal, narrower than JSON Schema's rule: `2`
is one and `2.0` is not. The bounds are the other way about — `"minimum": 1` and
`"minimum": 1.0` are the same bound and both are accepted, because a bound is
read as a number whatever the setting's type is. The examples here write `1.0`
for that reason and not because `1` would be refused.

Three layers resolve into one answer, in order: your `default`, then a pack's
override, then the player's file. Implement `configuration_declaration` to hand
the document over and `configure` to read the result. A player value that names
an unknown key or an illegal value has already been dropped into
`LayeredConfiguration::rejected` before you see it, so what you receive has been
validated against your own declaration.

**If you declare settings you must implement `configure`.** The default refuses,
deliberately: settings nothing reads are settings a player changes with no
effect, and the loud failure is the useful one.

That makes `ConfigurationDeclaration::parse(SETTINGS).ok()` the quietest failure
this API has, and it is the shape you will reach for first. `.ok()` turns a
document that does not parse into `None`, which the loader reads as "this mod
declares no settings" — so `configure` is never called, the refusal above never
happens, and your mod runs on its own defaults with nothing said to anybody. The
document is an `include_str!` constant, so a typo in it is your bug rather than a
player's; refuse on it in `enter`, where a refusal is heard:

```rust
const SETTINGS: &str = include_str!("../configuration.json");
const CAPACITY: &str = "lanterns.capacity";

#[declare_mod(id = "copper_lanterns")]
impl Mod for CopperLanterns {
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        // `configuration_declaration` can only answer `Some` or `None`, so a
        // typo in the document becomes `None`, which reads as "declares no
        // settings": `configure` is never called and the mod runs on its own
        // defaults in silence. Refusing here makes that loud.
        ConfigurationDeclaration::parse(SETTINGS).map_err(|error| ModError::Refused {
            mod_id: Self::declared_id().clone(),
            state,
            detail: error.to_string(),
        })?;
        Ok(())
    }

    fn configuration_declaration(&self) -> Option<ConfigurationDeclaration> {
        ConfigurationDeclaration::parse(SETTINGS).ok()
    }

    fn configure(&mut self, resolved: &LayeredConfiguration) -> Result<(), ModError> {
        let key: SettingKey = CAPACITY.parse().expect("the key this mod declared");
        self.capacity = resolved
            .value(&key)
            .and_then(JsonValue::as_u64)
            .ok_or_else(|| ModError::Refused {
                mod_id: Self::declared_id().clone(),
                state: LifecycleState::Constructed,
                detail: "`lanterns.capacity` resolved to something that is not a count"
                    .to_owned(),
            })?;
        Ok(())
    }
}
```

A resolved value is a `ravel_sdk::JsonValue`, because a setting's shape belongs to
the mod that declared it. `serde_json` is not a dependency of your crate and
cannot be named from it, so `use ravel_sdk::JsonValue;` is how you write the type
down — in a signature, or as the receiver of an associated function like
`JsonValue::as_u64` above. The closure form `.and_then(|value|
value.as_u64())` needs no import at all and is equally correct.

`json!` builds a value. To read one out of *text* — a file your mod read, a
string a player typed at a command — use `str::parse`, because `JsonValue`
implements `FromStr`: `let value: JsonValue = text.parse()?;`. There is no
`from_str` function to import and none is needed. The error is
`ravel_sdk::JsonError`, named in the facade so you can carry the failure in your
own signatures as well as report it.

## World data is versioned, and you cross the versions

Declare every version this build can read. The oldest is what still opens; the
highest is what you write.

```rust
fn read_entries(current: &SavedPayload) -> Vec<String> {
    current
        .get("entries")
        .and_then(|value| value.as_array())
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn v(raw: u32) -> SavedDataVersion {
    SavedDataVersion::new(raw).expect("every declared version is non-zero")
}

/// Version 1 stored one entry; version 2 stores a list.
fn to_two(payload: &SavedPayload) -> Result<SavedPayload, MigrationFailed> {
    let entry = payload
        .get("entry")
        .and_then(SavedPayload::as_str)
        .ok_or_else(|| MigrationFailed::new("version 1 held no `entry` string"))?;
    Ok(json!({ "entries": [entry] }))
}

#[declare_mod(id = "copper_lanterns")]
impl Mod for CopperLanterns {
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        let _ = state;
        Ok(())
    }

    fn saved_data_declaration(&self) -> Option<SavedDataDeclaration> {
        SavedDataDeclaration::new(Self::declared_id().clone(), [v(1), v(2)]).ok()
    }

    fn saved_data_migrations(&self) -> Option<MigrationChain> {
        Some(MigrationChain::new(Self::declared_id().clone()).reaching(v(2), to_two))
    }

    fn open_saved_data(
        &mut self,
        envelope: Option<&SavedDataEnvelope>,
        plan: &SavedDataPlan,
    ) -> Result<(), ModError> {
        let Some(envelope) = envelope else {
            self.entries.clear();
            return Ok(());
        };
        // The payload is at whatever version the world stored, so the chain
        // crosses the steps the plan names before anything is read.
        let current = self
            .saved_data_migrations()
            .expect("this mod declares a chain")
            .apply(plan, envelope.payload())
            .map_err(|failed| ModError::Refused {
                mod_id: Self::declared_id().clone(),
                state: LifecycleState::Registered,
                detail: failed.to_string(),
            })?;
        self.entries = read_entries(&current);
        Ok(())
    }

    fn save_saved_data(&self) -> Result<Option<SavedDataEnvelope>, ModError> {
        Ok(Some(SavedDataEnvelope::current(
            Self::declared_id().clone(),
            v(2),
            json!({ "entries": self.entries }),
        )))
    }
}
```

`SavedDataEnvelope::current` is the answer a mod that owns its data alone gives:
itself, the version this build writes, and the payload. `SavedDataEnvelope::new`
takes two more and is what to reach for when either is real.

The first is a `Vec<RequiredPackage>`, and a `RequiredPackage` is two public
fields: `id`, a package, and `version`, the version of it. It is a claim about
the payload — "this world cannot be read back without that package at that
version" — so a mod whose data stands alone passes an empty vector, which is what
`current` does for it. Order does not matter; `new` sorts the list so two runs
that name the same packages write the same bytes. Coming back in, a package that
is absent is refused as `SavedDataError::MissingPackage`, and one at the wrong
version as `SavedDataError::IncompatiblePackage`, which carries a
`PackageConflict` naming the owner, the package, the version required and the
version installed. You never build a `PackageConflict`; you read one out of a
refusal you are reporting.

The second is an `Option<String>`, a name you publish for your own payload's
shape so another reader can recognise it. Ravel does not interpret it and nothing
validates against it. Pass `None` if you publish no such name.

A `MigrationStep` is a plain `fn(&SavedPayload) -> Result<SavedPayload,
MigrationFailed>` — a function item, not a closure — so `to_two` is passed by
name. `MigrationChain::covers` checks, before any world is opened, that every
declared version above the oldest has a step; call it in your own tests.

The loader resolves your declaration against what the world holds and produces a
`SavedDataPlan`: the version is current, or there is an ordered list of steps, or
the world is refused. A refusal withholds the world rather than opening it
half-applied — a save that will not open is recoverable, and a save that opened
empty is not.

**The mod crosses the steps, not the host.** The envelope you are handed is at
whatever version the world stored, which is any version your declaration admits.
Read `SavedDataPlan::steps` and apply your chain to the payload. A host is free
to hand over an already-current payload and one of them does, but that is a
guarantee stronger than the contract, not the contract: the launch path hands
over the stored envelope and the steps, and a mod written to the stronger reading
is silently wrong in a running game, losing every field a later version added and
writing the loss back stamped current.

Every step or none. A chain that refuses partway leaves the world holding exactly
what it held.

You never read or write the world file. You answer with one
`SavedDataEnvelope`, and the loader owns the aggregate those envelopes go into —
which is what makes `MAX_AGGREGATE_BYTES` a property of the whole world rather
than of your own good behaviour. A value too large to cross is refused and blames
no mod.

## Services are declared, not discovered

A service is a named call one mod answers and others make — the only thing here
that crosses between mods at run time, so it is deliberately narrow. A
`ServiceId` is namespaced by the mod that answers it, so exactly one mod can, and
you may only offer to answer names in your own namespace.

Declare both halves in `service_declaration`: what you answer, and what you need
answered. The loader orders providers before consumers and refuses a set whose
services do not resolve — a service nothing answers, or two mods requiring each
other — naming the offender, before any mod starts. The `ServiceDirectory` you
are handed in `use_services` reaches only providers already started, so the
ordering guarantee is a property of what you receive rather than a rule you
follow.

A provider parked on this side is a service nothing answers. If you require a
service from a client-only mod, your mod does not start on a dedicated server.

Requests and answers are JSON, not a Rust type both mods name. A mod is
distributed as source, so a consumer naming the provider's Rust type would be
pinned to the provider's build instead of to the version range its manifest
declares. A provider owns the shape of its answer and versions it like anything
else it publishes.
