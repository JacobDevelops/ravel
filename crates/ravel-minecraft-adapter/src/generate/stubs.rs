//! Resolving the compile-time stubs the settings screen is built against.
//!
//! `jvm/build/build-screen.sh` cannot put the client jar on its class path: the
//! jar is a restricted local fixture and most machines do not hold one, which is
//! the property `build-agent.sh` already protects for the agent. So the screen
//! compiles against stub declarations of the few game types it names.
//!
//! Those stubs are generated rather than written, and the reason is erasure. A
//! hand-written stub transcribes a signature a person read, generics and all;
//! the JVM links on the erased descriptor, which is a different string. A
//! generated stub emits the descriptor the class file actually declares, so what
//! the screen compiles against and what the runtime resolves are the same thing
//! by construction. The seed list names classes and member names; every
//! descriptor, every access flag, and every supertype edge comes from the jar.
//!
//! A stub reproduces the hierarchy only where it has to. The set of stubbed
//! classes induces a subgraph of the game's real one: a supertype is declared
//! when it is itself stubbed, and dropped when it is not. That is what keeps
//! `Screen` free of the six types above it while still letting the `Component`
//! that `MutableComponent` implements be assignable to it.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

use super::classfile::{ACC_ABSTRACT, ACC_FINAL, ACC_INTERFACE, Hierarchy};
use super::inputs::{Adapter, Seeds};
use super::mapping::Mapping;
use super::{GenerateError, classfile, jar};

/// Packages already on every class path, which are never stubbed.
const PROVIDED: [&str; 2] = ["java/", "javax/"];

/// One member a stub declares.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct Member {
    pub(super) name: String,
    pub(super) descriptor: String,
    pub(super) flags: u16,
    /// Whether the member is a method rather than a field.
    pub(super) method: bool,
    /// What this member's `java.util` return holds, in reviewed names.
    ///
    /// The erased descriptor above is what the JVM links and says nothing about
    /// a collection's contents, so a stub generated from it alone declares a raw
    /// `List` and every element of it is an `Object`. A screen that read one
    /// element back out then compiled against a type the game never returns, and
    /// the mismatch reached a player as a cast that always throws.
    ///
    /// Carried only where the erased return is a `java.util` type, which is
    /// where a caller reads an element back. Rendering every generic a class
    /// file records would mean rendering the whole signature grammar, including
    /// declarations no stub here has a use for, and this is the shape the defect
    /// arrived in.
    #[serde(default)]
    pub(super) returns: Option<String>,
}

/// One generated stub class.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct Stub {
    /// Internal class name, as the JVM writes it.
    pub(super) class: String,
    /// The class's own access flags, as the jar declares them.
    pub(super) flags: u16,
    /// The stubbed superclass, when one of this class's ancestors is stubbed.
    #[serde(default)]
    pub(super) extends: Option<String>,
    /// Stubbed interfaces this class is assignable to.
    #[serde(default)]
    pub(super) implements: Vec<String>,
    /// Members the seed list asked for, empty for a class pulled in by a
    /// descriptor rather than named as a seed.
    #[serde(default, rename = "member")]
    pub(super) members: Vec<Member>,
    /// The superclass constructor this class's own constructors have to chain to.
    ///
    /// A stub body is unreachable, but a constructor still runs its superclass's
    /// first, and javac inserts a no-argument `super()` where a source writes
    /// none. That is fine while a stubbed class's stubbed superclass declares no
    /// constructor, which was every case until an item that places a block
    /// needed one: `BlockItem` extends `Item`, `Item`'s stub declares only the
    /// constructor the jar declares, and the implicit call named a constructor
    /// that is not there.
    ///
    /// So the descriptor is recorded here, where the whole stub set is known,
    /// and the renderer writes an explicit call. It is `None` where the
    /// superclass is unstubbed or declares no constructor of its own, which is
    /// the ordinary case.
    #[serde(default, rename = "superConstructor")]
    pub(super) super_constructor: Option<String>,
}

impl Stub {
    /// Whether the class file declares an interface rather than a class.
    pub(super) const fn is_interface(&self) -> bool {
        self.flags & ACC_INTERFACE != 0
    }

    /// Whether the class file declares the class abstract.
    pub(super) const fn is_abstract(&self) -> bool {
        self.flags & ACC_ABSTRACT != 0
    }

    /// Whether the class file declares the class final.
    pub(super) const fn is_final(&self) -> bool {
        self.flags & ACC_FINAL != 0
    }
}

/// The tracked stub lock.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Lock {
    schema_version: u8,
    minecraft: String,
    extractor: String,
    client_jar_sha256: String,
    data_fixer_jar_sha256: String,
    #[serde(rename = "stub")]
    stubs: Vec<Stub>,
}

/// Resolves every seeded class, and everything its members name, from the jars.
///
/// The archives are searched in the order the adapter inputs pin them, and the
/// first one that carries a class is the one it is read from. The game's own jar
/// comes first, so a library class the game shadows is still read from the game.
pub(super) fn from_jar(
    archives: &[&jar::Archive],
    seeds: &Seeds,
    mapping: &Mapping,
) -> Result<Vec<Stub>, GenerateError> {
    let mut members: BTreeMap<String, Vec<Member>> = BTreeMap::new();
    let mut referenced = BTreeSet::new();

    for seed in &seeds.stubs {
        let class = seed.class.replace('.', "/");
        // A class file names both halves of every member the way the JVM links
        // them, and the seed list is in the reviewed name space. Both halves
        // cross here, once, so nothing below can compare one name space against
        // the other or emit a stub that is half of each.
        let read = read_members(archives, mapping, &class)?;
        let declared = read
            .iter()
            .filter(|member| member.is_visible())
            .map(|member| {
                (
                    member,
                    Member {
                        name: mapping
                            .mojang_member(&class, &member.name, &member.descriptor)
                            .to_owned(),
                        descriptor: mapping.mojang_descriptor(&member.descriptor),
                        flags: member.flags,
                        method: member.method,
                        returns: None,
                    },
                )
            })
            .collect::<Vec<_>>();
        let mut selected = Vec::new();
        for seeded in &seed.members {
            let name = seeded.member();
            let matched = declared
                .iter()
                .filter(|(_, member)| member.name == name)
                .collect::<Vec<_>>();
            if matched.is_empty() {
                return Err(GenerateError::Unresolved(format!(
                    "{} declares no reachable {name}",
                    seed.class
                )));
            }
            // Every overload is stubbed. Picking one would mean writing its
            // descriptor into the seed list, which is the transcription this
            // generator exists to avoid.
            for (read, member) in matched {
                let mut member = member.clone();
                member.returns = element_of(&seed.class, read, &member.descriptor, mapping)?;
                referenced.extend(names_in(&member.descriptor));

                if let Some((_, element)) = member.returns.as_deref().and_then(argument) {
                    referenced.extend(names_in(&format!("L{element};")));
                }
                selected.push(member);
            }
        }
        selected.sort_by(|left, right| {
            (&left.name, &left.descriptor).cmp(&(&right.name, &right.descriptor))
        });
        if members.insert(class, selected).is_some() {
            return Err(GenerateError::Unresolved(format!(
                "{} is seeded twice",
                seed.class
            )));
        }
    }

    // A class named by a selected member's descriptor is stubbed too, with no
    // members of its own: the screen has to be able to write the type down, not
    // to call anything on it. Such a stub names nothing further, so the closure
    // stops here rather than walking the jar.
    let mut set = members.keys().cloned().collect::<BTreeSet<_>>();
    set.extend(referenced);

    let mut hierarchies = BTreeMap::new();
    for class in &set {
        hierarchies.insert(class.clone(), read_hierarchy(archives, mapping, class)?);
    }

    let mut stubs = Vec::with_capacity(set.len());
    for class in &set {
        let (extends, implements) = above(archives, mapping, class, &set, &hierarchies)?;
        stubs.push(Stub {
            class: class.clone(),
            flags: hierarchies[class].flags,
            extends,
            implements,
            members: members.get(class).cloned().unwrap_or_default(),
            super_constructor: None,
        });
    }
    chain_constructors(&mut stubs);
    Ok(stubs)
}

/// What one member's `java.util` return holds, in reviewed names.
///
/// A descriptor is erased, so it is the class file's `Signature` attribute that
/// says a split answers lines of text rather than lines of anything. Only the
/// one shape a stub can render is read: a `java.util` type carrying a single
/// class as its type argument. Anything else with a `java.util` return is
/// refused rather than emitted raw, because a raw collection is the silence
/// this exists to end and a generator that fell back to one would restore it.
///
/// # Errors
///
/// Returns an error when a member's `java.util` return records no type argument
/// or carries ones this cannot render.
fn element_of(
    class: &str,
    member: &classfile::Member,
    descriptor: &str,
    mapping: &Mapping,
) -> Result<Option<String>, GenerateError> {
    let returns = if member.method {
        descriptor
            .rsplit_once(')')
            .map_or(descriptor, |(_, after)| after)
    } else {
        descriptor
    };
    let Some(erased) = returns
        .strip_prefix('L')
        .and_then(|name| name.strip_suffix(';'))
        .filter(|name| name.starts_with("java/util/"))
    else {
        return Ok(None);
    };
    // A declaration recording no type argument for a `java.util` return is raw,
    // and a jar stripped of its `Signature` attributes makes every one of them
    // so. Emitting that raw is the silence this exists to end.
    let recorded = member
        .signature
        .as_deref()
        .map(|signature| {
            signature
                .rsplit_once(')')
                .map_or(signature, |(_, after)| after)
        })
        .filter(|tail| *tail != returns);
    let (bound, element) = recorded
        .and_then(|tail| {
            tail.strip_prefix(&format!("L{erased}<"))
                .and_then(|rest| rest.strip_suffix(">;"))
        })
        .and_then(argument)
        .ok_or_else(|| {
            GenerateError::Unsupported(format!(
                "{class} declares {} as {}, which is not a java.util type over one class",
                member.name,
                recorded.unwrap_or(returns)
            ))
        })?;
    Ok(Some(format!(
        "{}L{};",
        bound.recorded(),
        mapping.mojang_class(element)
    )))
}

/// How a recorded type argument bounds the class it names.
///
/// Closed so that a bound cannot be added and then dropped: every rendering of
/// one matches on this and stops compiling when a variant appears.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Bound {
    /// The class itself.
    Exact,
    /// Anything assignable to the class.
    Extends,
    /// Anything the class is assignable to.
    Super,
}

impl Bound {
    /// How a class file's signature writes this bound.
    pub(super) const fn recorded(self) -> &'static str {
        match self {
            Self::Exact => "",
            Self::Extends => "+",
            Self::Super => "-",
        }
    }

    /// How Java source writes this bound, ahead of the class it applies to.
    pub(super) const fn java(self) -> &'static str {
        match self {
            Self::Exact => "",
            Self::Extends => "? extends ",
            Self::Super => "? super ",
        }
    }
}

/// The wildcard bound and the class one recorded type argument names.
///
/// A signature writes an unbounded argument as the class alone, `? extends` as a
/// leading `+`, and `? super` as a leading `-`. Anything else — a type variable,
/// a nested argument, an inner class — answers `None`, which is what makes a
/// shape a stub cannot render a refusal rather than a raw declaration.
pub(super) fn argument(recorded: &str) -> Option<(Bound, &str)> {
    let (bound, rest) = recorded.strip_prefix('+').map_or_else(
        || {
            recorded
                .strip_prefix('-')
                .map_or((Bound::Exact, recorded), |rest| (Bound::Super, rest))
        },
        |rest| (Bound::Extends, rest),
    );
    let class = rest.strip_prefix('L')?.strip_suffix(';')?;
    (!class.contains(['<', '>', ';', '.'])).then_some((bound, class))
}

/// Records, for each stub, the superclass constructor its own has to call.
///
/// Done over the finished set because a stub is rendered on its own and cannot
/// see its superclass's members. Only a class that declares a constructor needs
/// one, and only where the stubbed superclass declares one too: everywhere else
/// javac's implicit `super()` reaches the implicit no-argument constructor that
/// a class declaring none has.
fn chain_constructors(stubs: &mut [Stub]) {
    let declared = stubs
        .iter()
        .map(|stub| {
            let first = stub
                .members
                .iter()
                .find(|member| member.method && member.name == "<init>")
                .map(|member| member.descriptor.clone());
            (stub.class.clone(), first)
        })
        .collect::<BTreeMap<_, _>>();
    for stub in stubs.iter_mut() {
        if !stub
            .members
            .iter()
            .any(|member| member.method && member.name == "<init>")
        {
            continue;
        }
        stub.super_constructor = stub
            .extends
            .as_ref()
            .and_then(|parent| declared.get(parent))
            .and_then(Clone::clone);
    }
}

/// Resolves every stub from the tracked lock.
///
/// This is what a machine without the restricted client jar generates from, for
/// the same reason `mappings.lock` exists: the lock already carries every
/// descriptor the jar resolved to, and it is tracked, so a change shows up in a
/// diff rather than in a build that quietly disagreed with it.
pub(super) fn from_lock(
    tracked: &str,
    adapter: &Adapter,
    seeds: &Seeds,
) -> Result<Vec<Stub>, GenerateError> {
    let lock: Lock = toml::from_str(tracked)
        .map_err(|error| GenerateError::Input(format!("cannot parse stubs.lock: {error}")))?;

    for (what, recorded, pinned) in [
        (
            "the version",
            lock.minecraft.as_str(),
            adapter.minecraft.as_str(),
        ),
        (
            "the client jar hash",
            lock.client_jar_sha256.as_str(),
            adapter.inputs.client_jar.sha256.as_deref().unwrap_or(""),
        ),
        (
            "the data fixer jar hash",
            lock.data_fixer_jar_sha256.as_str(),
            adapter
                .inputs
                .data_fixer_jar
                .sha256
                .as_deref()
                .unwrap_or(""),
        ),
    ] {
        if recorded != pinned {
            return Err(GenerateError::Drift(format!(
                "stubs.lock records {what} as {recorded} but the adapter inputs pin {pinned}"
            )));
        }
    }
    if lock.schema_version != adapter.schema_version {
        return Err(GenerateError::Drift(format!(
            "stubs.lock is schema {} but the adapter inputs are schema {}",
            lock.schema_version, adapter.schema_version
        )));
    }
    if lock.extractor != super::EXTRACTOR {
        return Err(GenerateError::Drift(format!(
            "stubs.lock was written by the {} extractor",
            lock.extractor
        )));
    }

    // The closure cannot be recomputed without the jar, so what is checked here
    // is the half that can be: every seeded class is present, and every member
    // name the seed list asks for resolves to at least one declaration.
    for seed in &seeds.stubs {
        let class = seed.class.replace('.', "/");
        let stub = lock
            .stubs
            .iter()
            .find(|stub| stub.class == class)
            .ok_or_else(|| {
                GenerateError::Drift(format!("stubs.lock carries no stub for {}", seed.class))
            })?;
        for seeded in &seed.members {
            let name = seeded.member();
            if !stub.members.iter().any(|member| member.name == name) {
                return Err(GenerateError::Drift(format!(
                    "stubs.lock pins no {name} on {}",
                    seed.class
                )));
            }
        }
    }
    Ok(lock.stubs)
}

/// Whether any pinned jar carries the class file one reviewed name resolves to.
fn held(archives: &[&jar::Archive], mapping: &Mapping, class: &str) -> bool {
    let entry = mapping.entry(class);
    archives.iter().any(|archive| archive.holds(&entry))
}

/// One class's bytes, from the first pinned jar that carries it.
///
/// A class no jar carries is unresolved rather than extracted: the seed list
/// named something that is in none of the sources the adapter inputs pin, and
/// guessing a declaration for it is the transcription the stubs exist to avoid.
fn read_class(
    archives: &[&jar::Archive],
    mapping: &Mapping,
    class: &str,
) -> Result<Vec<u8>, GenerateError> {
    let entry = mapping.entry(class);
    let archive = archives
        .iter()
        .find(|archive| archive.holds(&entry))
        .ok_or_else(|| {
            GenerateError::Unresolved(format!("no pinned jar carries {}", class.replace('/', ".")))
        })?;
    archive.entry(&entry)
}

/// Reads one class's declared members out of the jars.
fn read_members(
    archives: &[&jar::Archive],
    mapping: &Mapping,
    class: &str,
) -> Result<Vec<classfile::Member>, GenerateError> {
    let bytes = read_class(archives, mapping, class)?;
    classfile::members(&bytes)
        .map_err(|error| GenerateError::Extraction(format!("{class}: {error}")))
}

/// Reads one class's own flags and supertypes out of the jars.
fn read_hierarchy(
    archives: &[&jar::Archive],
    mapping: &Mapping,
    class: &str,
) -> Result<Hierarchy, GenerateError> {
    let bytes = read_class(archives, mapping, class)?;
    let read = classfile::hierarchy(&bytes)
        .map_err(|error| GenerateError::Extraction(format!("{class}: {error}")))?;
    // A class file names its supertypes the way the JVM links them, and every
    // set this walk compares against is in the reviewed name space.
    Ok(Hierarchy {
        flags: read.flags,
        super_class: read
            .super_class
            .map(|name| mapping.mojang_class(&name).to_owned()),
        interfaces: read
            .interfaces
            .iter()
            .map(|name| mapping.mojang_class(name).to_owned())
            .collect(),
    })
}

/// The stubbed types one class is assignable to.
///
/// The superclass slot takes the nearest stubbed ancestor on the extends chain,
/// so a stub declares at most the one Java allows. Every other stubbed type
/// above the class is an interface and is declared as one. A walk that leaves
/// the pinned jars stops: they carry the game's classes and the serialization
/// library, and not everything either of those compiles against.
fn above(
    archives: &[&jar::Archive],
    mapping: &Mapping,
    class: &str,
    set: &BTreeSet<String>,
    hierarchies: &BTreeMap<String, Hierarchy>,
) -> Result<(Option<String>, Vec<String>), GenerateError> {
    let mut extends = None;
    let mut walk = Some(class.to_owned());
    while let Some(current) = walk {
        let hierarchy = hierarchy_of(archives, mapping, &current, hierarchies)?;
        let Some(parent) = hierarchy.super_class else {
            break;
        };
        if set.contains(&parent) {
            extends = Some(parent);
            break;
        }
        walk = held(archives, mapping, &parent).then_some(parent);
    }

    let mut implements = BTreeSet::new();
    let mut seen = BTreeSet::new();
    let mut pending = vec![class.to_owned()];
    while let Some(current) = pending.pop() {
        if !seen.insert(current.clone()) {
            continue;
        }
        let hierarchy = hierarchy_of(archives, mapping, &current, hierarchies)?;
        for supertype in hierarchy.supertypes() {
            if set.contains(supertype) && Some(supertype) != extends.as_deref() {
                implements.insert(supertype.to_owned());
            }
            if held(archives, mapping, supertype) {
                pending.push(supertype.to_owned());
            }
        }
    }
    Ok((extends, implements.into_iter().collect()))
}

/// The hierarchy of one class, reading a jar only for a class not already read.
fn hierarchy_of(
    archives: &[&jar::Archive],
    mapping: &Mapping,
    class: &str,
    hierarchies: &BTreeMap<String, Hierarchy>,
) -> Result<Hierarchy, GenerateError> {
    match hierarchies.get(class) {
        Some(known) => Ok(known.clone()),
        None => read_hierarchy(archives, mapping, class),
    }
}

/// Every internal class name a descriptor mentions that is not already provided.
///
/// A nested class arrives here under its binary name — `Button$Builder` — and is
/// kept under it. The pinned version builds every interactive widget through
/// one: `Button` is
/// abstract and reached through `Button.builder`, which returns `Button$Builder`
/// and takes a `Button$OnPress`, so a stub set that refused them would compile a
/// screen with no button on it.
///
/// The stub for one is an ordinary top-level class that happens to have a `$` in
/// its name, which Java permits in an identifier. That is the same reasoning the
/// stubs rest on everywhere else: what the JVM links on is the binary name, the
/// class file javac writes for `class Button$Builder` carries exactly the binary
/// name the game's nested class has, and the source shape a human would write is
/// not what the linkage is against. Emitting it nested inside its enclosing stub
/// would produce the same class file from more machinery.
pub(super) fn names_in(descriptor: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = descriptor;
    while let Some(start) = rest.find('L') {
        let after = &rest[start + 1..];
        let Some(end) = after.find(';') else {
            break;
        };
        let name = &after[..end];
        // `L` also opens a class name inside an array descriptor, and both write
        // the name the same way, so one scan finds every one of them.
        if !name.is_empty() && !PROVIDED.iter().any(|package| name.starts_with(package)) {
            names.push(name.to_owned());
        }
        rest = &after[end + 1..];
    }
    names
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::super::classfile::{self, ACC_PUBLIC};
    use super::super::inputs::{self, MappingSource, PinnedVersion, SeedMember, Seeds};
    use super::super::jar::fixture;
    use super::super::mapping::Mapping;
    use super::{
        Bound, GenerateError, Lock, Stub, argument, element_of, from_jar, from_lock, names_in,
    };

    fn declared(descriptor: &str, signature: Option<&str>) -> classfile::Member {
        classfile::Member {
            name: "splitLines".to_owned(),
            descriptor: descriptor.to_owned(),
            flags: ACC_PUBLIC,
            method: true,
            signature: signature.map(str::to_owned),
        }
    }

    /// The two answers, on the one member shape the defect arrived in: a return
    /// that is not a collection says nothing, and a collection the class file
    /// records an argument for carries it.
    #[test]
    fn an_element_type_is_read_only_where_the_class_file_records_one() {
        let plain = declared("(Ljava/lang/String;)Ljava/lang/String;", None);
        assert_eq!(
            element_of(
                "com.example.Splitter",
                &plain,
                &plain.descriptor,
                &Mapping::Identity
            )
            .expect("a return that is not a collection is not refused"),
            None
        );

        let generic = declared(
            "(Ljava/lang/String;)Ljava/util/List;",
            Some("(Ljava/lang/String;)Ljava/util/List<Lcom/example/Line;>;"),
        );
        assert_eq!(
            element_of(
                "com.example.Splitter",
                &generic,
                &generic.descriptor,
                &Mapping::Identity
            )
            .expect("the recorded argument is read"),
            Some("Lcom/example/Line;".to_owned())
        );
    }

    /// A jar whose `Signature` attributes were stripped declares every returned
    /// collection raw, which is the shape the crash came in, so it is refused
    /// rather than stubbed as a list of anything.
    #[test]
    fn a_collection_return_recording_no_element_type_is_refused() {
        for signature in [None, Some("(Ljava/lang/String;)Ljava/util/List;")] {
            let raw = declared("(Ljava/lang/String;)Ljava/util/List;", signature);

            let error = element_of(
                "com.example.Splitter",
                &raw,
                &raw.descriptor,
                &Mapping::Identity,
            )
            .expect_err("a raw collection must not be emitted");

            assert!(matches!(error, GenerateError::Unsupported(_)), "{error}");
            assert!(error.to_string().contains("splitLines"), "{error}");
        }
    }

    /// The element class is read out of the jar and so arrives linked, which is
    /// the name space a stub never writes.
    #[test]
    fn an_element_class_is_translated_out_of_the_linked_name_space() {
        let mapping = Mapping::read(
            MappingSource::OfficialProguardMappings,
            "com.example.Line -> c:\n",
        )
        .expect("the invented mapping should parse");
        let linked = declared("()Ljava/util/List;", Some("()Ljava/util/List<Lc;>;"));

        assert_eq!(
            element_of(
                "com.example.Splitter",
                &linked,
                &linked.descriptor,
                &mapping
            )
            .expect("the recorded argument is read"),
            Some("Lcom/example/Line;".to_owned()),
            "a stub declaring a list of `c` names a class no reviewed source can"
        );
    }

    /// The two shapes a signature writes that a stub cannot render: a type
    /// variable names no class at all, and a second argument leaves a name that
    /// is not one.
    #[test]
    fn a_type_variable_or_a_second_argument_names_no_single_class() {
        assert_eq!(argument("TT;"), None);
        assert_eq!(argument("Ljava/lang/String;Lcom/example/Line;"), None);
        assert_eq!(
            argument("Lcom/example/Line;"),
            Some((Bound::Exact, "com/example/Line"))
        );
        assert_eq!(
            argument("+Lcom/example/Line;"),
            Some((Bound::Extends, "com/example/Line"))
        );
        assert_eq!(
            argument("-Lcom/example/Line;"),
            Some((Bound::Super, "com/example/Line"))
        );
    }

    /// A collection over something no stub can write down is refused rather than
    /// declared raw, because a raw declaration is the silence this ends.
    #[test]
    fn a_collection_over_a_shape_no_stub_can_render_is_refused() {
        let nested = declared(
            "()Ljava/util/List;",
            Some("()Ljava/util/List<Ljava/util/List<Lcom/example/Line;>;>;"),
        );

        let error = element_of(
            "com.example.Splitter",
            &nested,
            &nested.descriptor,
            &Mapping::Identity,
        )
        .expect_err("a list of lists is not one class");

        assert!(matches!(error, GenerateError::Unsupported(_)), "{error}");
    }

    fn pinned() -> PinnedVersion {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("the crate lives under crates/<name>");
        // The version whose class files are the mapping: these tests assert on a
        // lock whose linked names are its reviewed ones, which is what an
        // unobfuscated source gives and an obfuscated one does not.
        inputs::tracked_version_where(root, |adapter| !adapter.mapping_source.needs_mappings())
    }

    fn tracked_lock(pinned: &PinnedVersion) -> String {
        std::fs::read_to_string(pinned.directory.join("stubs.lock"))
            .expect("the tracked stub lock should be readable")
    }

    #[test]
    fn the_tracked_lock_carries_every_seeded_class_and_member() {
        let pinned = pinned();

        let stubs = from_lock(&tracked_lock(&pinned), &pinned.adapter, &pinned.seeds)
            .expect("the tracked stub lock should resolve");

        let names = stubs
            .iter()
            .map(|stub| &stub.class)
            .collect::<BTreeSet<_>>();
        for seed in &pinned.seeds.stubs {
            assert!(names.contains(&seed.class.replace('.', "/")), "{seed:?}");
        }
        assert!(
            stubs.iter().any(|stub| !stub.members.is_empty()),
            "a lock in which nothing declares a member stubs nothing"
        );
    }

    #[test]
    fn the_closure_carries_a_class_no_seed_names() {
        let pinned = pinned();

        let stubs =
            from_lock(&tracked_lock(&pinned), &pinned.adapter, &pinned.seeds).expect("resolves");

        let seeded = pinned
            .seeds
            .stubs
            .iter()
            .map(|seed| seed.class.replace('.', "/"))
            .collect::<BTreeSet<_>>();
        assert!(
            stubs.iter().any(|stub| !seeded.contains(&stub.class)),
            "a member's descriptor names types the seed list does not, and those are stubbed too"
        );
    }

    #[test]
    fn a_stubbed_supertype_is_declared_and_an_unstubbed_one_is_dropped() {
        let pinned = pinned();
        let stubs =
            from_lock(&tracked_lock(&pinned), &pinned.adapter, &pinned.seeds).expect("resolves");
        let stub = |class: &str| {
            stubs
                .iter()
                .find(|stub| stub.class == class)
                .unwrap_or_else(|| panic!("{class} should be stubbed"))
                .clone()
        };

        // Several types sit above `Screen` in the jar and two of them are stubbed,
        // so the stub declares those two and drops the rest. Neither edge is
        // decoration: the interface is what makes a screen assignable where the
        // game asks for a listener, and the class is where the focus a scrolled
        // screen has to restore is held.
        let screen = stub("net/minecraft/client/gui/screens/Screen");
        assert_eq!(
            screen.extends.as_deref(),
            Some("net/minecraft/client/gui/components/events/AbstractContainerEventHandler")
        );
        assert_eq!(
            screen.implements,
            ["net/minecraft/client/gui/components/events/GuiEventListener"]
        );
        assert!(screen.is_abstract());

        // The same rule under the widgets: each one reaches the stubbed listener
        // through ancestors that are not stubbed themselves.
        for widget in [
            "net/minecraft/client/gui/components/Button",
            "net/minecraft/client/gui/components/EditBox",
            "net/minecraft/client/gui/components/AbstractSliderButton",
        ] {
            assert_eq!(
                stub(widget).extends.as_deref(),
                Some("net/minecraft/client/gui/components/AbstractWidget"),
                "{widget} has to reach the widget base the screen adds it through"
            );
        }

        // Both edges are stubbed and both are kept. The second one is why the
        // profiling screen can hand what `Component.literal` answers to a split
        // that takes formatted text.
        let mutable = stub("net/minecraft/network/chat/MutableComponent");
        assert_eq!(
            mutable.implements,
            [
                "net/minecraft/network/chat/Component",
                "net/minecraft/network/chat/FormattedText"
            ],
            "the stub has to keep the edge that makes a returned value assignable"
        );
        assert!(stub("net/minecraft/network/chat/Component").is_interface());
    }

    #[test]
    fn a_lock_pinning_another_jar_or_another_extractor_is_refused() {
        let pinned = pinned();
        let tracked = tracked_lock(&pinned);
        let jar = pinned
            .adapter
            .inputs
            .client_jar
            .sha256
            .as_deref()
            .expect("a pinned hash");
        let library = pinned
            .adapter
            .inputs
            .data_fixer_jar
            .sha256
            .as_deref()
            .expect("a pinned hash");

        for (broken, expected) in [
            (tracked.replace(jar, &"0".repeat(64)), "the client jar hash"),
            (
                tracked.replace(library, &"0".repeat(64)),
                "the data fixer jar hash",
            ),
            (
                tracked.replace("extractor = \"classfile\"", "extractor = \"javap\""),
                "javap",
            ),
            (
                tracked.replace("schemaVersion = 1", "schemaVersion = 2"),
                "schema 2",
            ),
        ] {
            let error = from_lock(&broken, &pinned.adapter, &pinned.seeds)
                .expect_err("a drifted lock must be refused");
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[test]
    fn a_lock_that_lost_a_seeded_class_or_member_is_refused() {
        let pinned = pinned();
        let tracked = tracked_lock(&pinned);
        let first = pinned.seeds.stubs[0].class.replace('.', "/");
        let member = pinned.seeds.stubs[0].members[0].member();

        let without_class = tracked.replace(&format!("class = \"{first}\""), "class = \"a/Ghost\"");
        let error = from_lock(&without_class, &pinned.adapter, &pinned.seeds)
            .expect_err("a lock missing a seeded class must be refused");
        assert!(error.to_string().contains("carries no stub for"), "{error}");

        let without_member = tracked.replace(&format!("name = \"{member}\""), "name = \"ghost\"");
        let error = from_lock(&without_member, &pinned.adapter, &pinned.seeds)
            .expect_err("a lock missing a seeded member must be refused");
        assert!(error.to_string().contains("pins no"), "{error}");
    }

    #[test]
    fn a_descriptor_names_the_classes_a_stub_has_to_carry_and_no_others() {
        assert_eq!(
            names_in("(Lnet/minecraft/network/chat/Component;)V"),
            ["net/minecraft/network/chat/Component"]
        );
        assert_eq!(
            names_in("(Ljava/lang/String;[Ljava/lang/Object;)Lnet/minecraft/A;"),
            ["net/minecraft/A"],
            "a provided package is on every class path already, array or not"
        );
        assert_eq!(names_in("(II)V"), Vec::<String>::new());
        assert_eq!(names_in("([[Lnet/minecraft/B;)V"), ["net/minecraft/B"]);
    }

    #[test]
    fn a_nested_class_is_carried_under_the_binary_name_that_names_it() {
        let named = names_in("()Lcom/example/core/Holder$Reference;");

        assert_eq!(named, ["com/example/core/Holder$Reference"]);
    }

    /// The smallest class file declaring some public methods and nothing above.
    ///
    /// A method given a signature carries it as its one attribute, which is how
    /// a class file records what a returned collection holds.
    fn class_of(methods: &[(&str, &str, Option<&str>)]) -> Vec<u8> {
        let mut strings: Vec<&str> = Vec::new();
        for &(name, descriptor, signature) in methods {
            strings.push(name);
            strings.push(descriptor);
            if let Some(recorded) = signature {
                if !strings.contains(&"Signature") {
                    strings.push("Signature");
                }
                strings.push(recorded);
            }
        }
        let index = |text: &str| {
            let at = strings
                .iter()
                .position(|held| *held == text)
                .expect("every string a member names is interned");
            u16::try_from(at + 1).expect("a short pool")
        };

        let mut bytes = Vec::new();
        bytes.extend(0xCAFE_BABE_u32.to_be_bytes());
        bytes.extend(0_u16.to_be_bytes()); // minor
        bytes.extend(69_u16.to_be_bytes()); // major
        let pool = u16::try_from(strings.len() + 1).expect("a short pool");
        bytes.extend(pool.to_be_bytes());
        for text in &strings {
            bytes.push(1); // TAG_UTF8
            bytes.extend(u16::try_from(text.len()).expect("short").to_be_bytes());
            bytes.extend(text.as_bytes());
        }
        for value in [0_u16, 0, 0, 0, 0] {
            // flags, this class, no superclass, no interfaces, no fields
            bytes.extend(value.to_be_bytes());
        }
        bytes.extend(u16::try_from(methods.len()).expect("short").to_be_bytes());
        for &(name, descriptor, signature) in methods {
            bytes.extend(ACC_PUBLIC.to_be_bytes());
            bytes.extend(index(name).to_be_bytes());
            bytes.extend(index(descriptor).to_be_bytes());
            match signature {
                Some(recorded) => {
                    bytes.extend(1_u16.to_be_bytes()); // one method attribute
                    bytes.extend(index("Signature").to_be_bytes());
                    bytes.extend(2_u32.to_be_bytes());
                    bytes.extend(index(recorded).to_be_bytes());
                }
                None => bytes.extend(0_u16.to_be_bytes()),
            }
        }
        bytes.extend(0_u16.to_be_bytes()); // class attributes
        bytes
    }

    /// An invented obfuscated version: a seed list, its mapping, and its jars.
    ///
    /// The seed list and the class files share no member name at all, and the
    /// seeded name is overloaded so the two overloads link as two different
    /// names. A stub that comes out of this can only have gone through the
    /// mapping, in both directions: the filter has to find the seeded name on a
    /// class that declares neither, and the descriptor it keeps names a class the
    /// jar writes as `b`.
    fn obfuscated() -> (Seeds, Mapping, Vec<u8>, Vec<u8>) {
        let seeds = toml::from_str(
            "schemaVersion = 1\n\
             minecraft = \"0.0.0-test\"\n\
             [[stub]]\n\
             role = \"Screen\"\n\
             class = \"com.example.Screen\"\n\
             members = [\"init\"]\n",
        )
        .expect("the invented seed list should parse");
        let mapping = Mapping::read(
            MappingSource::OfficialProguardMappings,
            "com.example.Screen -> a:\n    \
             5:5:void init() -> aT_\n    \
             6:6:void init(com.example.Widget) -> b\n\
             com.example.Widget -> b:\n",
        )
        .expect("the invented mapping should parse");

        (
            seeds,
            mapping,
            fixture::stored(
                "a.class",
                &class_of(&[("aT_", "()V", None), ("b", "(Lb;)V", None)]),
            ),
            fixture::stored("b.class", &class_of(&[])),
        )
    }

    #[test]
    fn an_obfuscated_seed_resolves_through_the_mapping_in_both_directions() {
        let (seeds, mapping, screen, widget) = obfuscated();
        let screen = fixture::parse(screen).expect("a synthetic archive");
        let widget = fixture::parse(widget).expect("a synthetic archive");

        let stubs = from_jar(&[&screen, &widget], &seeds, &mapping)
            .expect("the mapping names every seeded member");

        let seeded = stubs
            .iter()
            .find(|stub| stub.class == "com/example/Screen")
            .expect("the seeded class is stubbed");
        assert_eq!(
            seeded
                .members
                .iter()
                .map(|member| (member.name.as_str(), member.descriptor.as_str()))
                .collect::<Vec<_>>(),
            [("init", "()V"), ("init", "(Lcom/example/Widget;)V")],
            "an overloaded seed stubs every overload, under the name the seed \
             list writes and never the one the jar declares"
        );
        assert!(
            stubs.iter().any(|stub| stub.class == "com/example/Widget"),
            "the kept descriptor is in the reviewed name space, so the closure \
             it names is too"
        );
    }

    /// The element type names a class no seed does, and a stub declaring a list
    /// of a class it cannot write down does not compile.
    #[test]
    fn the_class_an_element_type_names_is_stubbed_although_no_seed_names_it() {
        let seeds: Seeds = toml::from_str(
            "schemaVersion = 1\n\
             minecraft = \"0.0.0-test\"\n\
             [[stub]]\n\
             role = \"Screen\"\n\
             class = \"com.example.Splitter\"\n\
             members = [\"split\"]\n",
        )
        .expect("the invented seed list should parse");
        let splitter = fixture::parse(fixture::stored(
            "com/example/Splitter.class",
            &class_of(&[(
                "split",
                "()Ljava/util/List;",
                Some("()Ljava/util/List<Lcom/example/Line;>;"),
            )]),
        ))
        .expect("a synthetic archive");
        let line = fixture::parse(fixture::stored("com/example/Line.class", &class_of(&[])))
            .expect("a synthetic archive");

        let stubs = from_jar(&[&splitter, &line], &seeds, &Mapping::Identity)
            .expect("both classes are in the jars");

        let seeded = stubs
            .iter()
            .find(|stub| stub.class == "com/example/Splitter")
            .expect("the seeded class is stubbed");
        assert_eq!(
            seeded.members[0].returns.as_deref(),
            Some("Lcom/example/Line;")
        );
        assert!(
            stubs.iter().any(|stub| stub.class == "com/example/Line"),
            "the element class is named by no descriptor, so only the element \
             type pulls it into the stubbed set"
        );
    }

    #[test]
    fn a_seeded_member_the_obfuscated_class_does_not_declare_is_refused() {
        let (mut seeds, mapping, screen, widget) = obfuscated();
        seeds.stubs[0].members = vec![SeedMember::Same("onClose".to_owned())];
        let screen = fixture::parse(screen).expect("a synthetic archive");
        let widget = fixture::parse(widget).expect("a synthetic archive");

        let error = from_jar(&[&screen, &widget], &seeds, &mapping)
            .expect_err("the mapping names no onClose");

        assert!(
            error.to_string().contains("declares no reachable onClose"),
            "{error}"
        );
    }

    #[test]
    fn a_lock_whose_shape_is_wrong_is_refused_rather_than_half_read() {
        let pinned = pinned();

        let error = from_lock("stub = 3", &pinned.adapter, &pinned.seeds)
            .expect_err("this is not a stub lock");

        assert!(
            error.to_string().contains("cannot parse stubs.lock"),
            "{error}"
        );
    }

    #[test]
    fn the_lock_shape_round_trips_through_the_kinds_a_stub_can_be() {
        let lock: Lock = toml::from_str(
            "schemaVersion = 1\nminecraft = \"0.0.0\"\nextractor = \"classfile\"\n\
             clientJarSha256 = \"00\"\ndataFixerJarSha256 = \"01\"\n\n\
             [[stub]]\nclass = \"a/B\"\nflags = 1553\n",
        )
        .expect("a stub with no members and no supertypes is a whole stub");

        let [stub] = lock.stubs.as_slice() else {
            panic!("a lock holding one stub reads back as one stub");
        };
        assert!(stub.is_interface() && stub.is_abstract() && stub.is_final());
        assert_eq!(stub.members, Vec::new());

        let plain = Stub {
            class: "a/B".to_owned(),
            flags: 0,
            extends: None,
            implements: Vec::new(),
            members: Vec::new(),
            super_constructor: None,
        };
        assert!(!plain.is_interface() && !plain.is_abstract() && !plain.is_final());
    }
}
