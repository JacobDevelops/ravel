//! Writes the synthetic pack whose compile cost is the measurement.
//!
//! M2-W8 generated one-struct mod crates and its own record said the figures
//! that came out were floors rather than predictions. The difference here is
//! the body: every generated mod carries a configuration type it deserializes
//! at construction, an enum it matches over, a generic rule trait instantiated
//! once per feature, registry claims, and per-feature logic. That is what
//! `test-mods/registries` looks like, scaled up, and it is chosen because
//! compile cost in Rust follows derives and monomorphization rather than line
//! count.
//!
//! It is still generated, so it is still a floor. What it is not is a floor set
//! by an empty struct.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// Feature blocks per generated mod.
///
/// Eight gives a mod eleven serde derive pairs and sixteen instantiations of
/// the two generic functions, which puts it well above `test-mods/registries`,
/// the largest tracked test mod. Bigger would measure a bigger synthetic mod
/// rather than a more honest one; the count and the resulting source size are
/// both recorded with the result so a reader can scale them.
pub const FEATURES_PER_MOD: usize = 8;

/// Length of the dependency spine under [`Shape::Chain`].
///
/// Sixteen, because it is the depth at which a modded Rust stack stops being a
/// flat fan: a mod on a library on a library on a core is four, and a real pack
/// stacks several of those. It is also the knob that decides how much of the
/// build cannot be parallelized, so it is named here and recorded with the
/// result rather than buried in the shape.
pub const SPINE_DEPTH: usize = 16;

/// How the generated mods depend on one another.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Shape {
    /// No mod depends on another, so every crate can compile at once.
    Independent,
    /// A linear spine of [`SPINE_DEPTH`] mods that every other mod sits on.
    ///
    /// `mod-000` has no dependency, each later spine mod depends on the one
    /// before it, and every mod off the spine depends on the spine's tip and on
    /// its midpoint. The critical path is therefore the spine plus one, and no
    /// two spine crates can compile at the same time.
    Chain,
}

impl Shape {
    /// The identifier the record uses.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Independent => "independent",
            Self::Chain => "chain",
        }
    }
}

/// What to generate.
#[derive(Clone, Copy, Debug)]
pub struct PackSpec {
    /// Mod crates to write.
    pub mods: usize,
    /// How they depend on one another.
    pub shape: Shape,
    /// Whether the workspace asks for whole-program link-time optimization.
    pub lto: bool,
    /// Directory suffix, so variants of one size do not overwrite each other.
    pub label: &'static str,
}

impl PackSpec {
    /// The plain independent pack the main curve measures.
    #[must_use]
    pub const fn plain(mods: usize) -> Self {
        Self {
            mods,
            shape: Shape::Independent,
            lto: false,
            label: "",
        }
    }
}

/// What one generated workspace holds.
#[derive(Clone, Debug)]
pub struct Workspace {
    /// Where the workspace was written.
    pub root: PathBuf,
    /// Mod crates it holds.
    pub mods: usize,
    /// Bytes of Rust source in one mod crate.
    pub bytes_per_mod: u64,
    /// Lines of Rust source in one mod crate.
    pub lines_per_mod: usize,
}

impl Workspace {
    /// The first mod's source.
    ///
    /// Under [`Shape::Chain`] this is the root of the spine, so changing it is
    /// the worst case: every other crate depends on it transitively.
    #[must_use]
    pub fn first_mod_source(&self) -> PathBuf {
        self.mod_source(0)
    }

    /// The last mod's source.
    ///
    /// Under [`Shape::Chain`] this is a leaf, so changing it is the best case:
    /// nothing depends on it.
    #[must_use]
    pub fn last_mod_source(&self) -> PathBuf {
        self.mod_source(self.mods.saturating_sub(1))
    }

    fn mod_source(&self, index: usize) -> PathBuf {
        self.root.join(format!("mods/mod-{index:03}/src/lib.rs"))
    }

    /// The binary a cold build produces.
    #[must_use]
    pub fn binary(&self) -> PathBuf {
        self.root.join("target/release/ravel-pack-host")
    }

    /// The build cache a compiled workspace keeps.
    #[must_use]
    pub fn cache(&self) -> PathBuf {
        self.root.join("target")
    }
}

/// The mods one generated mod depends on, under a shape.
fn dependencies_of(index: usize, count: usize, shape: Shape) -> Vec<usize> {
    match shape {
        Shape::Independent => Vec::new(),
        Shape::Chain => {
            let depth = SPINE_DEPTH.min(count);
            if index == 0 {
                Vec::new()
            } else if index < depth {
                // The spine: one edge each, so no two spine crates overlap.
                vec![index - 1]
            } else {
                // Everything else hangs off the finished spine. Depending on the
                // tip and the midpoint gives real edges without chaining the
                // mods off the spine into a second, accidental long pole.
                let tip = depth - 1;
                let middle = depth / 2;
                if tip == middle {
                    vec![tip]
                } else {
                    vec![middle, tip]
                }
            }
        }
    }
}

/// Writes a workspace holding `count` generated mod crates and one host binary.
///
/// Nothing written here is tracked and nothing production reads it, which is
/// why the mods are generated: the measurement is about the count, and 256
/// hand-written crates would be 256 files saying the same thing.
///
/// # Errors
///
/// Returns a message when the workspace cannot be written.
pub fn generate(root: &Path, work: &Path, spec: PackSpec) -> Result<Workspace, String> {
    let count = spec.mods;
    let workspace = work.join(format!("pack-{count}{}", spec.label));
    let mods = workspace.join("mods");
    if mods.exists() {
        fs::remove_dir_all(&mods)
            .map_err(|error| format!("cannot clear generated mods: {error}"))?;
    }
    // The directory exists even when the pack is empty, because the workspace's
    // `mods/*` member glob is an error against a missing directory and matches
    // nothing against an empty one.
    fs::create_dir_all(&mods).map_err(|error| format!("cannot create generated mods: {error}"))?;
    fs::create_dir_all(workspace.join("host/src"))
        .map_err(|error| format!("cannot create a generated workspace: {error}"))?;

    // The repository's lock file, so the generated workspace resolves to the
    // same dependency versions the loader itself was built against.
    let lock = fs::read(root.join("Cargo.lock"))
        .map_err(|error| format!("cannot read the workspace lock file: {error}"))?;
    write(&workspace.join("Cargo.lock"), &lock)?;
    // Cargo refuses a member glob that matches nothing, so an empty pack names
    // only the host.
    let members = if count == 0 {
        "\"host\""
    } else {
        "\"host\", \"mods/*\""
    };
    let mut root_manifest = WORKSPACE_MANIFEST.replace("MEMBERS", members);
    if spec.lto {
        root_manifest.push_str(LTO_PROFILE);
    }
    write(&workspace.join("Cargo.toml"), root_manifest.as_bytes())?;

    let mut dependencies = String::new();
    let mut uses = String::new();
    let mut pushes = String::new();
    let mut measured = (0, 0);
    for index in 0..count {
        let name = format!("mod-{index:03}");
        let edges = dependencies_of(index, count, spec.shape);
        measured = generate_mod(&mods.join(&name), &name, index, &edges, root)?;
        let _ = writeln!(dependencies, "{name} = {{ path = \"../mods/{name}\" }}");
        let _ = writeln!(uses, "use mod_{index:03}::PackMod as Mod{index:03};");
        let _ = writeln!(
            pushes,
            "    mods.push(Box::new(Mod{index:03}::new()) as Box<dyn HostMod>);"
        );
    }

    write(
        &workspace.join("host/Cargo.toml"),
        manifest(HOST_MANIFEST, root)
            .replace("MOD_DEPENDENCIES", &dependencies)
            .as_bytes(),
    )?;
    write(
        &workspace.join("host/src/main.rs"),
        HOST_SOURCE
            .replace("MOD_USES\n", &uses)
            .replace("MOD_PUSHES\n", &pushes)
            .as_bytes(),
    )?;

    Ok(Workspace {
        root: workspace,
        mods: count,
        bytes_per_mod: measured.0,
        lines_per_mod: measured.1,
    })
}

/// Writes one generated mod crate and reports its source size.
fn generate_mod(
    directory: &Path,
    name: &str,
    index: usize,
    edges: &[usize],
    root: &Path,
) -> Result<(u64, usize), String> {
    fs::create_dir_all(directory.join("src"))
        .map_err(|error| format!("cannot create a generated mod: {error}"))?;
    let mut dependencies = String::new();
    for edge in edges {
        let _ = writeln!(
            dependencies,
            "mod-{edge:03} = {{ path = \"../mod-{edge:03}\" }}"
        );
    }
    write(
        &directory.join("Cargo.toml"),
        manifest(MOD_MANIFEST, root)
            .replace("MOD_DEPENDENCIES", &dependencies)
            .replace("MOD_NAME", name)
            .as_bytes(),
    )?;
    let source = mod_source(name, index, edges);
    write(&directory.join("src/lib.rs"), source.as_bytes())?;
    Ok((source.len() as u64, source.lines().count()))
}

/// Builds one mod's source: a shared preamble, `FEATURES_PER_MOD` feature
/// blocks, and the `HostMod` implementation that drives them.
fn mod_source(name: &str, index: usize, edges: &[usize]) -> String {
    let mut features = String::new();
    let mut claim_calls = String::new();
    let mut run_calls = String::new();
    for feature in 0..FEATURES_PER_MOD {
        features.push_str(
            &FEATURE_BLOCK
                .replace("IDX", &format!("{feature:03}"))
                .replace("ORD", &feature.to_string()),
        );
        let _ = writeln!(
            claim_calls,
            "        keys.extend(claims_{feature:03}(&self.id));"
        );
        let _ = writeln!(
            run_calls,
            "        total = total.saturating_add(run_{feature:03}(&self.settings));"
        );
    }
    // Each dependency is called rather than only declared, so the edge is real
    // in the type graph and not just in the manifest.
    let mut dependency_sums = String::new();
    for edge in edges {
        let _ = writeln!(
            dependency_sums,
            "    let total = total.saturating_add(mod_{edge:03}::contribution());"
        );
    }
    format!("{MOD_PREAMBLE}{features}{MOD_TAIL}")
        .replace("CLAIM_CALLS\n", &claim_calls)
        .replace("RUN_CALLS\n", &run_calls)
        .replace("DEPENDENCY_SUMS\n", &dependency_sums)
        .replace("SELF_ORD", &index.to_string())
        .replace("MOD_NAME", name)
}

/// Points a generated manifest's path dependencies at this repository.
fn manifest(template: &str, root: &Path) -> String {
    template
        .replace(
            "CORE_PATH",
            &root.join("crates/ravel-core").display().to_string(),
        )
        .replace(
            "HOST_API_PATH",
            &root.join("crates/ravel-host-api").display().to_string(),
        )
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    fs::write(path, bytes).map_err(|error| format!("cannot write a generated file: {error}"))
}

const WORKSPACE_MANIFEST: &str = r#"[workspace]
resolver = "3"
members = [MEMBERS]
"#;

const MOD_MANIFEST: &str = r#"[package]
name = "MOD_NAME"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
ravel-core = { path = "CORE_PATH" }
ravel-host-api = { path = "HOST_API_PATH" }
serde = { version = "1.0.229", features = ["derive"] }
serde_json = "1.0.151"
MOD_DEPENDENCIES"#;

/// Whole-program optimization across every crate in the generated pack.
///
/// `fat` rather than `thin` because the question is what optimizing the pack as
/// one unit costs, and `codegen-units = 1` because leaving it at sixteen would
/// hand the linker sixteen partitions to merge and measure a milder thing than
/// the one being asked about.
const LTO_PROFILE: &str = r#"
[profile.release]
lto = "fat"
codegen-units = 1
"#;

const HOST_MANIFEST: &str = r#"[package]
name = "ravel-pack-host"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
ravel-core = { path = "CORE_PATH" }
ravel-host-api = { path = "HOST_API_PATH" }
MOD_DEPENDENCIES"#;

const MOD_PREAMBLE: &str = r##"#![forbid(unsafe_code)]

//! Generated stand-in for a real mod, written by
//! `ravel-spike-m3-pack-compile`. Nothing here is tracked.

use std::collections::BTreeMap;
use std::fmt;

use ravel_core::{ContentId, ContentKind, LifecycleState, ModId, RegistryKey};
use ravel_host_api::{HostError, HostMod, Registrar};
use serde::{Deserialize, Serialize};

/// The identity this mod is packaged under.
pub const MOD_ID: &str = "MOD_NAME";

/// The configuration document a packaged mod would ship with.
const DEFAULT_CONFIGURATION: &str = r#"{"enabled":true,"batch":8,"floor":3,"label":"MOD_NAME","tiers":["raw","refined","alloyed","catalysed"],"overrides":{}}"#;

/// What this mod offers a mod that depends on it.
///
/// Under the independent shape nothing calls this across a crate boundary and
/// the sum below is empty. Under the chain shape every dependent calls it, so
/// the dependency edge is real in the type graph rather than only declared in
/// the manifest.
#[must_use]
pub fn contribution() -> u32 {
    let total: u32 = SELF_ORD;
DEPENDENCY_SUMS
    total
}

/// How a feature grades the material it works on.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum Tier {
    /// Straight out of the world.
    #[default]
    Raw,
    /// Smelted once.
    Refined,
    /// Combined with another material.
    Alloyed,
    /// Processed with a catalyst.
    Catalysed,
}

impl Tier {
    /// Every tier, in grade order.
    pub const ALL: [Self; 4] = [Self::Raw, Self::Refined, Self::Alloyed, Self::Catalysed];

    /// The tier's canonical lowercase name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Raw => "raw",
            Self::Refined => "refined",
            Self::Alloyed => "alloyed",
            Self::Catalysed => "catalysed",
        }
    }

    /// What a tier multiplies a stock count by.
    #[must_use]
    pub const fn multiplier(self) -> u32 {
        match self {
            Self::Raw => 1,
            Self::Refined => 2,
            Self::Alloyed => 5,
            Self::Catalysed => 11,
        }
    }

    /// The next tier up, or `None` at the top.
    #[must_use]
    pub const fn next(self) -> Option<Self> {
        match self {
            Self::Raw => Some(Self::Refined),
            Self::Refined => Some(Self::Alloyed),
            Self::Alloyed => Some(Self::Catalysed),
            Self::Catalysed => None,
        }
    }
}

impl fmt::Display for Tier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// What one feature decided about one input.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "outcome")]
pub enum Outcome {
    /// The input was processed and produced this much.
    Accepted {
        /// What the input produced.
        yielded: u32,
    },
    /// The input was left for a later pass.
    Deferred {
        /// Why it was left.
        reason: String,
    },
    /// The input was refused.
    Rejected {
        /// Why it was refused.
        reason: String,
    },
}

impl Outcome {
    /// What this outcome produced, which is nothing unless it was accepted.
    #[must_use]
    pub const fn yielded(&self) -> u32 {
        match self {
            Self::Accepted { yielded } => *yielded,
            Self::Deferred { .. } | Self::Rejected { .. } => 0,
        }
    }
}

/// The settings this mod reads from its configuration document.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Settings {
    /// Whether the features run at all.
    pub enabled: bool,
    /// How many inputs a pass takes.
    pub batch: u32,
    /// The weight below which an input is refused.
    pub floor: u32,
    /// What this mod calls itself in reports.
    pub label: String,
    /// The tiers this mod is willing to promote into.
    pub tiers: Vec<Tier>,
    /// Per-tag batch sizes that override `batch`.
    pub overrides: BTreeMap<String, u32>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            batch: 8,
            floor: 3,
            label: MOD_ID.to_owned(),
            tiers: Tier::ALL.to_vec(),
            overrides: BTreeMap::new(),
        }
    }
}

/// A rule a feature applies to the inputs it is given.
pub trait Rule {
    /// What the rule reads.
    type Input;

    /// The rule's reported name.
    fn name(&self) -> &'static str;

    /// Decides one input against the settings.
    fn evaluate(&self, input: &Self::Input, settings: &Settings) -> Outcome;
}

/// Applies one rule across a batch, which is the shape every feature shares.
pub fn apply<R: Rule>(rule: &R, inputs: &[R::Input], settings: &Settings) -> Vec<Outcome> {
    inputs
        .iter()
        .map(|input| rule.evaluate(input, settings))
        .collect()
}

/// Totals what a batch produced, keyed by the rule that produced it.
pub fn tally<R: Rule>(rule: &R, outcomes: &[Outcome]) -> BTreeMap<&'static str, u32> {
    let mut totals = BTreeMap::new();
    for outcome in outcomes {
        let running = totals.entry(rule.name()).or_insert(0u32);
        *running = running.saturating_add(outcome.yielded());
    }
    totals
}
"##;

const FEATURE_BLOCK: &str = r#"
/// Input the IDX feature works on.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct StockIDX {
    /// How far this stock has been processed.
    pub tier: Tier,
    /// How much of it there is.
    pub count: u32,
    /// The identifier overrides are keyed on.
    pub tag: String,
}

impl StockIDX {
    /// Builds one stock entry under this mod's own tag namespace.
    #[must_use]
    pub fn new(tier: Tier, count: u32) -> Self {
        Self {
            tier,
            count,
            tag: format!("MOD_NAME:stock-IDX/{tier}"),
        }
    }

    /// What this stock is worth before the rule sees it.
    #[must_use]
    pub fn weight(&self) -> u32 {
        self.count.saturating_mul(self.tier.multiplier())
    }
}

/// The IDX feature's rule.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FeatureIDX {
    /// What this feature adds before comparing against the floor.
    pub bias: u32,
}

impl Rule for FeatureIDX {
    type Input = StockIDX;

    fn name(&self) -> &'static str {
        "MOD_NAME:feature-IDX"
    }

    fn evaluate(&self, input: &Self::Input, settings: &Settings) -> Outcome {
        if !settings.enabled {
            return Outcome::Deferred {
                reason: format!("{} is disabled", self.name()),
            };
        }
        let weight = input
            .weight()
            .saturating_add(self.bias)
            .saturating_add(ORD);
        if weight < settings.floor {
            return Outcome::Rejected {
                reason: format!("{} weighs {weight}, under the floor", input.tag),
            };
        }
        let scaled = settings
            .overrides
            .get(input.tag.as_str())
            .copied()
            .unwrap_or(settings.batch)
            .saturating_mul(weight);
        match input.tier.next() {
            Some(next) if settings.tiers.contains(&next) => Outcome::Accepted {
                yielded: scaled.saturating_add(next.multiplier()),
            },
            Some(_) | None => Outcome::Accepted { yielded: scaled },
        }
    }
}

/// What the IDX feature claims in the registry.
#[must_use]
pub fn claims_IDX(id: &ModId) -> Vec<RegistryKey> {
    [
        (ContentKind::Block, "stock-IDX"),
        (ContentKind::Item, "stock-IDX"),
        (ContentKind::Recipe, "smelting/stock-IDX"),
    ]
    .into_iter()
    .map(|(kind, path)| {
        RegistryKey::new(
            kind,
            ContentId::new(id.clone(), path).expect("a legal content path"),
        )
    })
    .collect()
}

/// Runs the IDX feature over a batch built from the settings.
#[must_use]
pub fn run_IDX(settings: &Settings) -> u32 {
    let feature = FeatureIDX { bias: ORD };
    let inputs: Vec<StockIDX> = Tier::ALL
        .into_iter()
        .map(|tier| StockIDX::new(tier, settings.batch.saturating_add(ORD)))
        .collect();
    let outcomes = apply(&feature, &inputs, settings);
    tally(&feature, &outcomes)
        .values()
        .copied()
        .fold(0u32, u32::saturating_add)
}
"#;

const MOD_TAIL: &str = r#"
/// The generated mod the host links and drives.
pub struct PackMod {
    id: ModId,
    settings: Settings,
    registered: Vec<RegistryKey>,
    yielded: u32,
    entered: usize,
}

impl PackMod {
    /// Builds the mod instance the loader drives, reading its own configuration.
    ///
    /// # Panics
    ///
    /// Panics if `MOD_ID` stops being a legal mod id or the built-in
    /// configuration stops parsing, either of which would mean this crate can
    /// no longer be packaged.
    #[must_use]
    pub fn new() -> Self {
        let mut value = Self {
            id: MOD_ID.parse().expect("`MOD_ID` is a legal mod id"),
            settings: Settings::default(),
            registered: Vec::new(),
            yielded: 0,
            entered: 0,
        };
        value
            .configure(DEFAULT_CONFIGURATION)
            .expect("the built-in configuration parses");
        value
    }

    /// Replaces this mod's settings with a configuration document.
    ///
    /// # Errors
    ///
    /// Returns the parse failure, in serde's words.
    pub fn configure(&mut self, document: &str) -> Result<(), String> {
        self.settings = serde_json::from_str(document).map_err(|error| error.to_string())?;
        Ok(())
    }

    /// What this mod's features produced while running.
    #[must_use]
    pub const fn yielded(&self) -> u32 {
        self.yielded
    }

    /// How many lifecycle steps this mod took.
    #[must_use]
    pub const fn entered(&self) -> usize {
        self.entered
    }

    /// The keys the loader accepted, in the order it accepted them.
    #[must_use]
    pub fn registered(&self) -> &[RegistryKey] {
        &self.registered
    }

    /// Every key this mod claims, one feature at a time.
    fn claims(&self) -> Vec<RegistryKey> {
        let mut keys = Vec::new();
CLAIM_CALLS
        keys
    }

    /// Runs every feature and totals what they produced.
    fn work(&self) -> u32 {
        let mut total = 0u32;
RUN_CALLS
        total
    }
}

impl Default for PackMod {
    fn default() -> Self {
        Self::new()
    }
}

impl HostMod for PackMod {
    fn id(&self) -> &ModId {
        &self.id
    }

    /// Accepts every step, and does this mod's work once it is running.
    ///
    /// # Errors
    ///
    /// Never returns an error.
    fn enter(&mut self, state: LifecycleState) -> Result<(), HostError> {
        self.entered = self.entered.saturating_add(1);
        if state == LifecycleState::Running {
            self.yielded = self.work();
        }
        Ok(())
    }

    /// Claims one block, one item, and one recipe per feature.
    ///
    /// # Errors
    ///
    /// Returns `HostError::Refused` naming the identifier the loader would not
    /// give it.
    fn register(&mut self, registrar: &mut dyn Registrar) -> Result<(), HostError> {
        for key in self.claims() {
            registrar
                .claim(key.clone())
                .map_err(|refused| HostError::Refused {
                    mod_id: self.id.clone(),
                    state: LifecycleState::Constructed,
                    detail: refused.to_string(),
                })?;
            self.registered.push(key);
        }
        Ok(())
    }
}
"#;

const HOST_SOURCE: &str = r#"use ravel_core::{LifecycleState, RegistryKey};
use ravel_host_api::{HostMod, Registrar, RegistrationRefused};
MOD_USES

/// A registrar that accepts everything, because this host measures a build.
#[derive(Default)]
struct Accepting {
    claimed: Vec<RegistryKey>,
}

impl Registrar for Accepting {
    fn claim(&mut self, key: RegistryKey) -> Result<(), RegistrationRefused> {
        self.claimed.push(key);
        Ok(())
    }

    fn claimed(&self) -> &[RegistryKey] {
        &self.claimed
    }
}

fn main() {
    let mut mods: Vec<Box<dyn HostMod>> = Vec::new();
MOD_PUSHES
    let mut registrar = Accepting::default();
    let mut entered = 0usize;
    for value in &mut mods {
        value
            .register(&mut registrar)
            .expect("a generated mod registers");
        for state in [
            LifecycleState::Loaded,
            LifecycleState::Constructed,
            LifecycleState::Registered,
            LifecycleState::Running,
            LifecycleState::Stopping,
        ] {
            value
                .enter(state)
                .expect("a generated mod accepts every step");
            entered += 1;
        }
    }
    println!("{entered} {}", registrar.claimed().len());
}
"#;
