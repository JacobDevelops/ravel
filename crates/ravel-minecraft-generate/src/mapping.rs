//! Maps reviewed names to linked jar names through identity or validated name maps.

use std::collections::BTreeMap;
use std::fmt;

use proguard::ProguardRecord;

use super::inputs::{Lookup, MappingSource, Reflected, ReflectedMember, Reflection};
use super::{GenerateError, classfile, jar};

/// How a Mojang name becomes the name the JVM links.
#[derive(Clone, Debug)]
pub(super) enum Mapping {
    /// The jar declares the reviewed names, so nothing is translated.
    Identity,
    /// A published mapping normalized into a common, bidirectional name map.
    NameMap(NameMap),
}

impl Mapping {
    /// Builds the mapping one source calls for.
    ///
    /// # Errors
    ///
    /// Returns an error when translating records do not parse or collide.
    /// Feather text must already be normalized by the mapping-io helper.
    pub(super) fn read(source: MappingSource, text: &str) -> Result<Self, GenerateError> {
        match source {
            MappingSource::UnobfuscatedClassFiles => Ok(Self::Identity),
            MappingSource::OfficialProguardMappings => NameMap::parse(text).map(Self::NameMap),
            MappingSource::FeatherTinyMappings => Self::tiny(text),
        }
    }

    /// Reads normalized mapping-io output, not the Tiny file grammar itself.
    pub(super) fn tiny(normalized: &str) -> Result<Self, GenerateError> {
        let mut names = NameMap::normalized(normalized)?;
        names.inherited_hooks = true;
        Ok(Self::NameMap(names))
    }

    /// The internal name the JVM links for a reviewed internal name.
    pub(super) fn runtime_class<'a>(&'a self, mojang: &'a str) -> &'a str {
        match self {
            Self::Identity => mojang,
            Self::NameMap(proguard) => proguard.runtime_class(mojang),
        }
    }

    /// The internal name the JVM links for a reviewed *game* class.
    ///
    /// [`Self::runtime_class`] answers for any name at all, because a stub seed
    /// may name a library class the published mapping never describes and
    /// passing that name through is the right answer. Every name in the
    /// reflective surface is a game class instead, so a mapping that does not
    /// name one has not agreed with it — it has said nothing about it, and
    /// resolving it to itself would accept a Mojang name the target version
    /// renamed away.
    ///
    /// # Errors
    ///
    /// Returns an error when a translating mapping does not name the class.
    pub(super) fn linked_class<'a>(&'a self, mojang: &'a str) -> Result<&'a str, GenerateError> {
        match self {
            Self::Identity => Ok(mojang),
            Self::NameMap(proguard) => proguard.linked_class(mojang),
        }
    }

    /// The reviewed internal name a linked internal name came from.
    pub(super) fn mojang_class<'a>(&'a self, runtime: &'a str) -> &'a str {
        match self {
            Self::Identity => runtime,
            Self::NameMap(proguard) => proguard.mojang_class(runtime),
        }
    }

    /// The archive entry holding one reviewed class's bytes.
    pub(super) fn entry(&self, mojang: &str) -> String {
        format!("{}.class", self.runtime_class(mojang))
    }

    /// The reviewed descriptor a linked descriptor came from.
    ///
    /// A class file names its own types the way the JVM links them, so a
    /// descriptor read out of one has to come back to the reviewed name space
    /// before it is compared with anything the inventory or the seed list says.
    pub(super) fn mojang_descriptor(&self, descriptor: &str) -> String {
        match self {
            Self::Identity => descriptor.to_owned(),
            Self::NameMap(proguard) => map_descriptor(&proguard.mojang, descriptor),
        }
    }

    /// The reviewed name a linked member of one reviewed class came from.
    ///
    /// This is the direction a stub seed needs, and [`Self::runtime_member`]
    /// cannot serve it. A seed names a member and deliberately never a
    /// descriptor, because a name the class overloads is meant to stub every
    /// overload — and the forward direction is keyed on the descriptor precisely
    /// because one reviewed name can link as several. So the translation runs
    /// the other way: a class file declares both halves of the linked pair, and
    /// that pair is what names one member.
    ///
    /// A pair the mapping does not carry reads as itself, for the reason
    /// [`Self::mojang_class`] does: the pinned library jar beside the game is
    /// described by no published mapping, and a mapped class still declares
    /// members — bridges, and anything the mapping had no reason to name — that
    /// the file lists none of.
    pub(super) fn mojang_member<'a>(
        &'a self,
        mojang_class: &str,
        member: &'a str,
        descriptor: &str,
    ) -> &'a str {
        match self {
            Self::Identity => member,
            Self::NameMap(proguard) => proguard.mojang_member(mojang_class, member, descriptor),
        }
    }

    /// The linked pair one reviewed member is emitted as.
    ///
    /// This is the inverse of [`Self::mojang_member`] and shares its
    /// pass-through: a member the mapping never named is emitted under the name
    /// it already carries, which is what a constructor and a library class's
    /// members need. [`Self::runtime_member`] refuses that case instead, because
    /// a hook target the mapping does not name is a hook nobody reviewed; a stub
    /// declares whatever the class file declares, and `<init>` is in every one of
    /// them.
    pub(super) fn linked_member(
        &self,
        mojang_class: &str,
        member: &str,
        descriptor: &str,
    ) -> (String, String) {
        match self {
            Self::Identity => (member.to_owned(), descriptor.to_owned()),
            Self::NameMap(proguard) => proguard.linked_member(mojang_class, member, descriptor),
        }
    }

    /// The member name and descriptor the JVM links for a reviewed member.
    ///
    /// A class the mapping does not name keeps its members, which is what a
    /// library jar beside the game needs: nothing in the published mapping
    /// describes it, and inventing a translation for it would be the
    /// transcription this generator exists to avoid. A class the mapping does
    /// name and a member it does not is refused, because that mapping is the
    /// only thing that could have said what the jar calls it.
    ///
    /// # Errors
    ///
    /// Returns an error when the mapping names the class but not the member.
    pub(super) fn runtime_member(
        &self,
        mojang_class: &str,
        member: &str,
        descriptor: &str,
    ) -> Result<(String, String), GenerateError> {
        match self {
            Self::Identity => Ok((member.to_owned(), descriptor.to_owned())),
            Self::NameMap(proguard) => proguard.runtime_member(mojang_class, member, descriptor),
        }
    }

    /// Resolves an explicitly declared reflective member before exact class-file verification.
    ///
    /// JVM initializer names cannot be remapped and Tiny may omit them. Only their descriptors
    /// need translation; ordinary members retain strict published-mapping resolution.
    pub(super) fn reflected_member(
        &self,
        class: &str,
        member: &str,
        descriptor: &str,
    ) -> Result<(String, String), GenerateError> {
        if matches!(member, "<init>" | "<clinit>") {
            let (_, linked) = self.linked_member(class, member, descriptor);
            return Ok((member.to_owned(), linked));
        }
        self.runtime_member(class, member, descriptor)
    }

    /// Resolves an override whose reviewed name is recorded only on an ancestor.
    /// All inherited candidates must agree; the caller verifies the concrete member.
    pub(super) fn runtime_hook_member(
        &self,
        archive: &jar::Archive,
        class: &str,
        member: &str,
        descriptor: &str,
        kind: super::inputs::Kind,
    ) -> Result<(String, String), GenerateError> {
        if matches!(member, "<init>" | "<clinit>") {
            return self.reflected_member(class, member, descriptor);
        }
        let Self::NameMap(names) = self else {
            return self.runtime_member(class, member, descriptor);
        };
        if !names.inherited_hooks || kind != super::inputs::Kind::InstanceMethod {
            return self.runtime_member(class, member, descriptor);
        }
        if !names.classes.contains_key(class)
            || names.members.get(class).is_some_and(|members| {
                members.contains_key(&(member.to_owned(), descriptor.to_owned()))
            })
        {
            return self.runtime_member(class, member, descriptor);
        }
        let direct = Err(GenerateError::Unresolved(format!(
            "the client mapping names {class} but not its {member}{descriptor}"
        )));
        let candidates =
            self.inherited_hook_candidates(archive, names, class, member, descriptor)?;
        if candidates.len() == 1 {
            let pair = candidates
                .into_iter()
                .next()
                .expect("one inherited candidate");
            if names
                .linked
                .get(class)
                .and_then(|members| members.get(&pair))
                .is_some_and(|reviewed| reviewed != member)
            {
                return Err(GenerateError::Unresolved(format!(
                    "{class}.{member}{descriptor} inherits a pair explicitly mapped to another name"
                )));
            }
            return Ok(pair);
        }
        if candidates.is_empty() {
            return direct;
        }
        Err(GenerateError::Unresolved(format!(
            "{class}.{member}{descriptor} has conflicting inherited mappings"
        )))
    }

    fn inherited_hook_candidates(
        &self,
        archive: &jar::Archive,
        names: &NameMap,
        class: &str,
        member: &str,
        descriptor: &str,
    ) -> Result<std::collections::BTreeSet<(String, String)>, GenerateError> {
        let mut pending = vec![self.runtime_class(class).to_owned()];
        let mut visited = std::collections::BTreeSet::new();
        let mut candidates = std::collections::BTreeSet::new();
        while let Some(runtime) = pending.pop() {
            if !visited.insert(runtime.clone()) {
                continue;
            }
            let entry = format!("{runtime}.class");
            if !archive.holds(&entry) {
                continue;
            }
            let bytes = archive.entry(&entry)?;
            if runtime != self.runtime_class(class)
                && let Some(pair) = names
                    .members
                    .get(self.mojang_class(&runtime))
                    .and_then(|members| members.get(&(member.to_owned(), descriptor.to_owned())))
            {
                let declared = classfile::members(&bytes)
                    .map_err(|error| GenerateError::Extraction(format!("{runtime}: {error}")))?;
                if declared.iter().any(|target| {
                    target.method
                        && target.name == pair.0
                        && target.descriptor == pair.1
                        && !target.is_static()
                        && target.flags & classfile::ACC_PRIVATE == 0
                }) {
                    candidates.insert(pair.clone());
                }
            }
            let hierarchy = classfile::hierarchy(&bytes)
                .map_err(|error| GenerateError::Extraction(format!("{runtime}: {error}")))?;
            pending.extend(hierarchy.supertypes().map(str::to_owned));
        }
        Ok(candidates)
    }

    /// The pair the JVM links for a reviewed member named by its width.
    ///
    /// The forward direction is keyed on the descriptor, which a lookup by
    /// argument count does not have. What it does have is enough for a
    /// translating mapping to supply one: the mapping declares every member of
    /// the class, so the reviewed name and the width select exactly one line,
    /// and a name and width that select several is a declaration that says
    /// nothing about which member is meant. Under the identity there is no line
    /// to read and the width is what the class file is asked for directly.
    ///
    /// # Errors
    ///
    /// Returns an error when a translating mapping declares no member of that
    /// name and width, or more than one.
    fn linked_by_arity(
        &self,
        mojang_class: &str,
        member: &str,
        parameters: usize,
    ) -> Result<Linked, GenerateError> {
        match self {
            Self::Identity => Ok(Linked::Counted {
                name: member.to_owned(),
                parameters,
            }),
            Self::NameMap(proguard) => proguard.linked_by_arity(mojang_class, member, parameters),
        }
    }
}

/// Verifies declared runtime class names against the mapping.
///
/// # Errors
///
/// Returns an error for missing translated classes or mismatched runtime names.
pub(super) fn verify_reflection(
    surface: &Reflection,
    mapping: &Mapping,
) -> Result<(), GenerateError> {
    for reflected in &surface.classes {
        let resolved = binary(mapping.linked_class(&internal(&reflected.class))?);
        if resolved != reflected.runtime {
            return Err(GenerateError::Drift(format!(
                "the reflective surface says {} links as {} but the mapping resolves it to \
                 {resolved}",
                reflected.class, reflected.runtime
            )));
        }
    }
    Ok(())
}

/// Verifies reflected members against their linked class files.
///
/// Descriptor lookups require an exact pair; argument-count lookups resolve by width.
///
/// # Errors
///
/// Returns an error for invalid lookups, unmapped members, or missing declarations.
pub(super) fn verify_reflected_members(
    surface: &Reflection,
    mapping: &Mapping,
    archive: &jar::Archive,
) -> Result<(), GenerateError> {
    for reflected in &surface.classes {
        if reflected.members.is_empty() {
            continue;
        }
        let mojang = internal(&reflected.class);
        let bytes = archive.entry(&mapping.entry(&mojang))?;
        let declared = classfile::members(&bytes)
            .map_err(|error| GenerateError::Extraction(format!("{}: {error}", reflected.class)))?;
        for member in &reflected.members {
            verify_member(reflected, member, mapping, &mojang, &declared)?;
        }
    }
    Ok(())
}

/// The pair a class file has to declare for one reviewed member.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Linked {
    /// The exact pair the mapping resolves the declaration to.
    Exact { name: String, descriptor: String },
    /// The linked name, with the argument count that picks the overload.
    ///
    /// Only an identity mapping answers this way. A translating one has the
    /// declaring line in front of it and so knows the descriptor too.
    Counted { name: String, parameters: usize },
}

impl Linked {
    /// The name the class file has to declare.
    fn name(&self) -> &str {
        match self {
            Self::Exact { name, .. } | Self::Counted { name, .. } => name,
        }
    }

    /// Whether one declared member is the member this wanted.
    fn matches(&self, held: &classfile::Member) -> bool {
        held.name == self.name()
            && held.method == self.is_method()
            && match self {
                Self::Exact { descriptor, .. } => held.descriptor == *descriptor,
                Self::Counted { parameters, .. } => takes(&held.descriptor, *parameters),
            }
    }

    /// Whether this names a method rather than a field.
    ///
    /// An argument count is one only a method has, so a declaration that carries
    /// one has said which kind it means as surely as a descriptor does.
    fn is_method(&self) -> bool {
        match self {
            Self::Exact { descriptor, .. } => descriptor.starts_with('('),
            Self::Counted { .. } => true,
        }
    }
}

impl fmt::Display for Linked {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exact { name, descriptor } => write!(formatter, "{name}{descriptor}"),
            Self::Counted { name, parameters } => write!(formatter, "{name} taking {parameters}"),
        }
    }
}

/// Proves one declared member against the class file that has to declare it.
fn verify_member(
    reflected: &Reflected,
    member: &ReflectedMember,
    mapping: &Mapping,
    mojang_class: &str,
    declared: &[classfile::Member],
) -> Result<(), GenerateError> {
    let linked = match &member.lookup {
        Lookup::Descriptor(descriptor) => {
            let (name, descriptor) =
                mapping.reflected_member(mojang_class, &member.name, descriptor)?;
            Linked::Exact { name, descriptor }
        }
        Lookup::Parameters(parameters) => {
            mapping.linked_by_arity(mojang_class, &member.name, *parameters)?
        }
    };
    if declared.iter().any(|held| linked.matches(held)) {
        return Ok(());
    }
    Err(GenerateError::Drift(format!(
        "the reflective surface says {} declares {}, which links as {linked}, and {} declares {}",
        reflected.class,
        member.name,
        reflected.runtime,
        describe_declared(declared, linked.name())
    )))
}

/// What one class file declares under a linked name, for a refusal to name.
fn describe_declared(declared: &[classfile::Member], name: &str) -> String {
    let mut held = declared
        .iter()
        .filter(|member| member.name == name)
        .map(|member| format!("{name}{}", member.descriptor))
        .collect::<Vec<_>>();
    held.sort();
    held.dedup();
    match held.len() {
        0 => format!("no {name} at all"),
        // A linked name an obfuscated class reuses can carry dozens of members,
        // and a refusal nobody can read is a refusal that gets skimmed.
        1..=6 => held.join(", "),
        many => format!("{} and {} more", held[..6].join(", "), many - 6),
    }
}

/// Whether a descriptor is a method descriptor of the given width.
fn takes(descriptor: &str, parameters: usize) -> bool {
    classfile::signature(descriptor).is_ok_and(|(taken, _)| taken.len() == parameters)
}

/// A member's name paired with its descriptor.
type Member = (String, String);

/// Every member of one class, from the reviewed name to the linked one.
type Members = BTreeMap<Member, Member>;

/// Every member of one class, from the linked name back to the reviewed one.
type Reviewed = BTreeMap<Member, String>;

/// Validated names and descriptors, independent of the published mapping format.
#[derive(Clone, Debug)]
pub(super) struct NameMap {
    /// Only Tiny mappings inherit reviewed method names across jar hierarchies.
    inherited_hooks: bool,
    /// Reviewed internal name to linked internal name.
    classes: BTreeMap<String, String>,
    /// Linked internal name back to the reviewed one.
    mojang: BTreeMap<String, String>,
    /// Reviewed internal class name, then reviewed member and descriptor.
    members: BTreeMap<String, Members>,
    /// Reviewed internal class name, then linked member and descriptor.
    linked: BTreeMap<String, Reviewed>,
}

/// Admits the same class bijection for every supported mapping syntax.
fn admit_class(
    classes: &mut BTreeMap<String, String>,
    mojang: &mut BTreeMap<String, String>,
    from: &str,
    to: &str,
    number: usize,
) -> Result<(), GenerateError> {
    if classes.insert(from.to_owned(), to.to_owned()).is_some() {
        return Err(unusable(number, &format!("{from} is mapped twice")));
    }
    if let Some(other) = mojang.insert(to.to_owned(), from.to_owned()) {
        return Err(unusable(
            number,
            &format!("{other} and {from} both map to {to}"),
        ));
    }
    Ok(())
}

impl NameMap {
    /// Admits normalized records through the common bidirectional checks.
    fn normalized(text: &str) -> Result<Self, GenerateError> {
        let mut classes = BTreeMap::new();
        let mut mojang = BTreeMap::new();
        let mut declared: BTreeMap<String, Vec<Declared>> = BTreeMap::new();
        for (index, line) in text.lines().enumerate() {
            let number = index + 1;
            let mut columns = line.split('\t');
            let record = (
                columns.next(),
                columns.next(),
                columns.next(),
                columns.next(),
                columns.next(),
                columns.next(),
            );
            match record {
                (Some("C"), Some(from), Some(to), None, None, None) => {
                    verify_internal_class_name(from).map_err(|detail| unusable(number, &detail))?;
                    verify_internal_class_name(to).map_err(|detail| unusable(number, &detail))?;
                    admit_class(&mut classes, &mut mojang, from, to, number)?;
                    declared.entry(from.to_owned()).or_default();
                }
                (
                    Some(kind @ ("F" | "M")),
                    Some(owner),
                    Some(name),
                    Some(descriptor),
                    Some(runtime),
                    None,
                ) => {
                    verify_internal_class_name(owner)
                        .map_err(|detail| unusable(number, &detail))?;
                    verify_member_name(name).map_err(|detail| unusable(number, &detail))?;
                    verify_member_name(runtime).map_err(|detail| unusable(number, &detail))?;
                    verify_descriptor(descriptor, kind == "M")
                        .map_err(|detail| unusable(number, &detail))?;
                    declared
                        .entry(owner.to_owned())
                        .or_default()
                        .push(Declared {
                            member: name.to_owned(),
                            descriptor: descriptor.to_owned(),
                            runtime: runtime.to_owned(),
                        });
                }
                _ => return Err(unusable(number, "invalid normalized mapping record")),
            }
        }
        for owner in declared.keys() {
            if !classes.contains_key(owner) {
                return Err(GenerateError::Input(format!(
                    "the normalized mapping declares members of unmapped class {owner}"
                )));
            }
        }
        Self::parse_members(classes, mojang, declared)
    }

    /// Parses a published mapping file.
    ///
    /// Every line is either a comment, a class line, or a member of the class
    /// above it. A line that is none of those is refused rather than skipped: a
    /// mapping the parser silently drops half of resolves the other half against
    /// a jar and writes a binding, which is the failure this whole layer exists
    /// to make impossible.
    fn parse(text: &str) -> Result<Self, GenerateError> {
        let mut classes = BTreeMap::new();
        let mut mojang: BTreeMap<String, String> = BTreeMap::new();
        let mut declared: BTreeMap<String, Vec<Declared>> = BTreeMap::new();
        let mut current: Option<String> = None;

        for (number, line) in text.lines().enumerate() {
            let number = number + 1;
            // A comment is a comment at any indentation; testing the untrimmed
            // line would send an indented one to the member branch.
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let record = ProguardRecord::try_parse(line.trim_end().as_bytes())
                .map_err(|error| unusable(number, &error.to_string()))?;
            match record {
                ProguardRecord::Class {
                    original,
                    obfuscated,
                } => {
                    let from = internal(original);
                    let to = internal(obfuscated);
                    admit_class(&mut classes, &mut mojang, &from, &to, number)?;
                    current = Some(from);
                }
                ProguardRecord::Field { .. } | ProguardRecord::Method { .. } => {
                    let class = current
                        .as_ref()
                        .ok_or_else(|| unusable(number, "a member precedes every class"))?;
                    let member =
                        Declared::parse(&record).map_err(|detail| unusable(number, &detail))?;
                    // Qualified methods describe inline stack frames, not
                    // declarations on this class. Validate them above, but
                    // leave the linked pair to the actual outer declaration.
                    if matches!(
                        record,
                        ProguardRecord::Method {
                            original_class: Some(_),
                            ..
                        }
                    ) {
                        continue;
                    }
                    declared.entry(class.clone()).or_default().push(member);
                }
                ProguardRecord::Header { .. } | ProguardRecord::R8Header(_) => {}
            }
        }

        Self::parse_members(classes, mojang, declared)
    }

    fn parse_members(
        classes: BTreeMap<String, String>,
        mojang: BTreeMap<String, String>,
        declared: BTreeMap<String, Vec<Declared>>,
    ) -> Result<Self, GenerateError> {
        // Descriptors are rewritten once every class is known, because a
        // parameter can name a class the file has not reached yet.
        let mut members = BTreeMap::new();
        let mut linked = BTreeMap::new();
        for (class, entries) in declared {
            let mut forward = BTreeMap::new();
            let mut backward = BTreeMap::new();
            for entry in entries {
                let runtime = map_descriptor(&classes, &entry.descriptor);
                // Refusing a second member on one linked pair is what makes the
                // backward direction a function: it is the same refusal the
                // forward direction needs, and it now owns the map it guards.
                if backward
                    .insert(
                        (entry.runtime.clone(), runtime.clone()),
                        entry.member.clone(),
                    )
                    .is_some()
                {
                    return Err(GenerateError::Input(format!(
                        "the client mapping maps two members of {class} onto {} {}",
                        entry.runtime, runtime
                    )));
                }
                let reviewed = (entry.member, entry.descriptor);
                if forward
                    .insert(reviewed.clone(), (entry.runtime, runtime))
                    .is_some()
                {
                    return Err(GenerateError::Input(format!(
                        "the client mapping declares {}{} of {class} twice",
                        reviewed.0, reviewed.1
                    )));
                }
            }
            members.insert(class.clone(), forward);
            linked.insert(class, backward);
        }

        // A mapping that names nothing translates nothing, and every lookup
        // below would fall through to the reviewed name. That is the identity
        // wearing a source that promised to translate, so it is refused here
        // rather than resolved against a jar that declares none of those names.
        if classes.is_empty() {
            return Err(GenerateError::Input(
                "the client mapping names no class".to_owned(),
            ));
        }

        Ok(Self {
            inherited_hooks: false,
            classes,
            mojang,
            members,
            linked,
        })
    }

    fn runtime_class<'a>(&'a self, mojang: &'a str) -> &'a str {
        self.classes.get(mojang).map_or(mojang, String::as_str)
    }

    fn mojang_class<'a>(&'a self, runtime: &'a str) -> &'a str {
        self.mojang.get(runtime).map_or(runtime, String::as_str)
    }

    fn linked_class(&self, mojang: &str) -> Result<&str, GenerateError> {
        self.classes.get(mojang).map(String::as_str).ok_or_else(|| {
            GenerateError::Unresolved(format!(
                "the client mapping names no class {}",
                mojang.replace('/', ".")
            ))
        })
    }

    fn mojang_member<'a>(
        &'a self,
        mojang_class: &str,
        member: &'a str,
        descriptor: &str,
    ) -> &'a str {
        self.linked
            .get(mojang_class)
            .and_then(|declared| declared.get(&(member.to_owned(), descriptor.to_owned())))
            .map_or(member, String::as_str)
    }

    fn linked_member(
        &self,
        mojang_class: &str,
        member: &str,
        descriptor: &str,
    ) -> (String, String) {
        self.members
            .get(mojang_class)
            .and_then(|declared| declared.get(&(member.to_owned(), descriptor.to_owned())))
            .cloned()
            .unwrap_or_else(|| (member.to_owned(), map_descriptor(&self.classes, descriptor)))
    }

    fn runtime_member(
        &self,
        mojang_class: &str,
        member: &str,
        descriptor: &str,
    ) -> Result<(String, String), GenerateError> {
        let Some(declared) = self.members.get(mojang_class) else {
            return Ok((member.to_owned(), descriptor.to_owned()));
        };
        declared
            .get(&(member.to_owned(), descriptor.to_owned()))
            .cloned()
            .ok_or_else(|| {
                GenerateError::Unresolved(format!(
                    "the client mapping names {} but not its {member}{descriptor}",
                    mojang_class.replace('/', ".")
                ))
            })
    }

    fn linked_by_arity(
        &self,
        mojang_class: &str,
        member: &str,
        parameters: usize,
    ) -> Result<Linked, GenerateError> {
        let mut found = self
            .members
            .get(mojang_class)
            .into_iter()
            .flatten()
            .filter(|((name, descriptor), _)| name == member && takes(descriptor, parameters))
            .map(|(_, linked)| linked.clone())
            .collect::<Vec<_>>();
        found.sort();
        found.dedup();
        let readable = mojang_class.replace('/', ".");
        match found.as_slice() {
            [] => Err(GenerateError::Unresolved(format!(
                "the client mapping names {readable} but no {member} of it taking {parameters}"
            ))),
            [(name, descriptor)] => Ok(Linked::Exact {
                name: name.clone(),
                descriptor: descriptor.clone(),
            }),
            several => Err(GenerateError::Unresolved(format!(
                "the client mapping declares {} members of {readable} called {member} taking \
                 {parameters}, and a count that separates nothing names none of them",
                several.len()
            ))),
        }
    }
}

/// One member line, before descriptors are rewritten.
struct Declared {
    member: String,
    descriptor: String,
    runtime: String,
}

impl Declared {
    /// Converts a parsed member into the reviewed descriptor namespace.
    ///
    /// Line metadata is deliberately ignored, not parsed as part of the name.
    fn parse(record: &ProguardRecord<'_>) -> Result<Self, String> {
        let (member, descriptor, runtime) = match record {
            ProguardRecord::Field {
                ty,
                original,
                obfuscated,
            } => ((*original).to_owned(), descriptor_of(ty)?, obfuscated),
            ProguardRecord::Method {
                ty,
                original,
                obfuscated,
                arguments,
                ..
            } => {
                let mut descriptor = String::from("(");
                if !arguments.is_empty() {
                    for parameter in arguments.split(',').map(str::trim) {
                        descriptor.push_str(&descriptor_of(parameter)?);
                    }
                }
                descriptor.push(')');
                descriptor.push_str(&descriptor_of(ty)?);
                ((*original).to_owned(), descriptor, obfuscated)
            }
            ProguardRecord::Class { .. }
            | ProguardRecord::Header { .. }
            | ProguardRecord::R8Header(_) => return Err("record declares no member".to_owned()),
        };
        Ok(Self {
            member,
            descriptor,
            runtime: (*runtime).to_owned(),
        })
    }
}

/// A refusal naming the line the mapping stopped being readable on.
fn unusable(number: usize, detail: &str) -> GenerateError {
    GenerateError::Input(format!(
        "the client mapping is unusable at line {number}: {detail}"
    ))
}

/// Validates an internal class name without treating a descriptor as a name.
fn verify_internal_class_name(name: &str) -> Result<(), String> {
    if name.split('/').any(str::is_empty)
        || name.chars().any(|character| {
            character.is_whitespace()
                || character.is_control()
                || matches!(character, '.' | ';' | '[' | '(' | ')')
        })
    {
        return Err(format!("{name:?} is not an internal class name"));
    }
    Ok(())
}

/// Validates one member name, retaining JVM constructor spellings.
fn verify_member_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.chars().any(|character| {
            character.is_whitespace()
                || character.is_control()
                || matches!(character, '.' | ';' | '[' | '/' | '(' | ')')
        })
    {
        return Err(format!("{name:?} is not a member name"));
    }
    Ok(())
}

/// Uses the shared JVM tokenizer rather than accepting malformed descriptors.
fn verify_descriptor(descriptor: &str, method: bool) -> Result<(), String> {
    if method {
        classfile::method_types(descriptor)?;
    } else {
        let (kind, after) = classfile::java_type(descriptor)?;
        if kind == "void" || !after.is_empty() {
            return Err(format!("{descriptor} is not a field descriptor"));
        }
    }
    let mut rest = descriptor;
    while let Some(start) = rest.find('L') {
        let after = &rest[start + 1..];
        let end = after
            .find(';')
            .ok_or_else(|| format!("{descriptor} has an unterminated class name"))?;
        verify_internal_class_name(&after[..end])?;
        rest = &after[end + 1..];
    }
    Ok(())
}

/// The internal form of a name the mapping writes with dots.
fn internal(name: &str) -> String {
    name.replace('.', "/")
}

/// The binary form of an internal name, which is what `Class.forName` takes.
fn binary(name: &str) -> String {
    name.replace('/', ".")
}

/// The JVM descriptor of one source-form type.
fn descriptor_of(source: &str) -> Result<String, String> {
    let mut dimensions = 0_usize;
    let mut base = source;
    while let Some(rest) = base.strip_suffix("[]") {
        dimensions += 1;
        base = rest;
    }
    let core = match base {
        "void" => "V".to_owned(),
        "boolean" => "Z".to_owned(),
        "byte" => "B".to_owned(),
        "char" => "C".to_owned(),
        "short" => "S".to_owned(),
        "int" => "I".to_owned(),
        "long" => "J".to_owned(),
        "float" => "F".to_owned(),
        "double" => "D".to_owned(),
        "" => return Err(format!("{source} names no type")),
        other => format!("L{};", internal(other)),
    };
    Ok("[".repeat(dimensions) + &core)
}

/// Rewrites every class a descriptor names into the name the JVM links.
fn map_descriptor(classes: &BTreeMap<String, String>, descriptor: &str) -> String {
    let mut mapped = String::with_capacity(descriptor.len());
    let mut rest = descriptor;
    while let Some(index) = rest.find('L') {
        mapped.push_str(&rest[..index]);
        let after = &rest[index + 1..];
        let Some(end) = after.find(';') else {
            mapped.push_str(&rest[index..]);
            return mapped;
        };
        let class = &after[..end];
        mapped.push('L');
        mapped.push_str(classes.get(class).map_or(class, String::as_str));
        mapped.push(';');
        rest = &after[end + 1..];
    }
    mapped.push_str(rest);
    mapped
}

#[cfg(test)]
mod tests {
    use super::super::classfile;
    use super::super::inputs::{Reflected, ReflectedMember, Reflection};
    use super::super::jar::{self, fixture};
    use super::{Mapping, MappingSource, NameMap, verify_reflected_members, verify_reflection};

    /// An invented mapping, in the shape the published files take.
    const MAPPING: &str = "\
# a comment the parser ignores
com.example.Alpha -> a:
# {\"fileName\":\"Alpha.java\",\"id\":\"sourceFile\"}
    java.util.regex.Pattern PATTERN -> f
    int[][] grid -> g
    10:10:void <init>() -> <init>
    17:26:boolean allowed(char) -> a
    30:30:com.example.Beta widen(com.example.Beta,int[]) -> b
com.example.Beta -> b$c:
    5:5:void tick() -> a
com.example.Plain -> com.example.Plain:
    5:5:void tick() -> tick
";

    fn parsed() -> NameMap {
        NameMap::parse(MAPPING).expect("the invented mapping should parse")
    }

    #[test]
    fn a_class_reads_both_ways_and_an_unnamed_one_reads_as_itself() {
        let mapping = parsed();

        assert_eq!(mapping.runtime_class("com/example/Alpha"), "a");
        assert_eq!(mapping.mojang_class("a"), "com/example/Alpha");
        assert_eq!(mapping.runtime_class("com/example/Beta"), "b$c");
        assert_eq!(mapping.mojang_class("b$c"), "com/example/Beta");
        assert_eq!(
            mapping.runtime_class("com/example/Plain"),
            "com/example/Plain"
        );
        assert_eq!(
            mapping.runtime_class("java/lang/String"),
            "java/lang/String"
        );
        assert_eq!(mapping.mojang_class("java/lang/String"), "java/lang/String");
    }

    #[test]
    fn a_member_carries_the_descriptor_the_inventory_reviews() {
        let mapping = parsed();

        assert_eq!(
            mapping
                .runtime_member("com/example/Alpha", "allowed", "(C)Z")
                .expect("the mapping names it"),
            ("a".to_owned(), "(C)Z".to_owned())
        );
        assert_eq!(
            mapping
                .runtime_member("com/example/Alpha", "PATTERN", "Ljava/util/regex/Pattern;")
                .expect("a field is a member too"),
            ("f".to_owned(), "Ljava/util/regex/Pattern;".to_owned())
        );
        assert_eq!(
            mapping
                .runtime_member("com/example/Alpha", "grid", "[[I")
                .expect("an array field is a member too"),
            ("g".to_owned(), "[[I".to_owned())
        );
    }

    /// The direction a stub seed reads, on the pair a class file declares.
    ///
    /// `widen` is the case the forward direction cannot answer: a stub seed
    /// carries no descriptor, so nothing could hand `runtime_member` the one it
    /// is keyed on, and the linked descriptor is in the obfuscated name space
    /// besides.
    #[test]
    fn a_linked_member_reads_back_as_the_name_the_seed_list_writes() {
        let mapping = Mapping::NameMap(parsed());

        assert_eq!(
            mapping.mojang_member("com/example/Alpha", "a", "(C)Z"),
            "allowed"
        );
        assert_eq!(
            mapping.mojang_member("com/example/Alpha", "b", "(Lb$c;[I)Lb$c;"),
            "widen"
        );
        assert_eq!(
            mapping.mojang_member("com/example/Alpha", "f", "Ljava/util/regex/Pattern;"),
            "PATTERN"
        );
        assert_eq!(
            mapping.mojang_member("com/example/Alpha", "g", "[[I"),
            "grid"
        );
    }

    /// The descriptor is what tells one linked name from another, both ways.
    #[test]
    fn a_linked_name_the_class_reuses_is_told_apart_by_its_descriptor() {
        let mapping = Mapping::NameMap(
            NameMap::parse(
                "com.example.Alpha -> a:\n    \
                 5:5:java.lang.String getValue() -> a\n    \
                 6:6:void setValue(java.lang.String) -> a\n",
            )
            .expect("one linked name on two descriptors is one mapping, not a collision"),
        );

        assert_eq!(
            mapping.mojang_member("com/example/Alpha", "a", "()Ljava/lang/String;"),
            "getValue"
        );
        assert_eq!(
            mapping.mojang_member("com/example/Alpha", "a", "(Ljava/lang/String;)V"),
            "setValue"
        );
    }

    /// The leniency [`Mapping::mojang_class`] has, for the same reason: a mapped
    /// class still declares members the published file names none of.
    #[test]
    fn a_linked_pair_no_mapping_carries_reads_as_itself() {
        let mapping = Mapping::NameMap(parsed());

        assert_eq!(mapping.mojang_member("com/example/Alpha", "a", "(I)Z"), "a");
        assert_eq!(
            mapping.mojang_member("com/example/Absent", "a", "(C)Z"),
            "a"
        );
        assert_eq!(
            Mapping::Identity.mojang_member("com/example/Alpha", "a", "(C)Z"),
            "a",
            "an identity mapping translates nothing, which is what hid this"
        );
    }

    #[test]
    fn every_class_a_descriptor_names_is_rewritten_with_it() {
        let mapping = parsed();

        assert_eq!(
            mapping
                .runtime_member(
                    "com/example/Alpha",
                    "widen",
                    "(Lcom/example/Beta;[I)Lcom/example/Beta;"
                )
                .expect("the mapping names it"),
            ("b".to_owned(), "(Lb$c;[I)Lb$c;".to_owned())
        );
    }

    #[test]
    fn a_named_class_whose_member_is_unnamed_is_refused_rather_than_passed_through() {
        let error = parsed()
            .runtime_member("com/example/Alpha", "allowed", "(I)Z")
            .expect_err("the mapping declares (C)Z and nothing else");

        assert!(error.to_string().contains("com.example.Alpha"), "{error}");
        assert!(error.to_string().contains("allowed(I)Z"), "{error}");
    }

    #[test]
    fn a_class_the_mapping_never_names_keeps_its_members() {
        assert_eq!(
            parsed()
                .runtime_member("com/example/Absent", "read", "()V")
                .expect("nothing describes it, so nothing translates it"),
            ("read".to_owned(), "()V".to_owned())
        );
    }

    #[test]
    fn two_names_that_map_onto_one_are_refused() {
        for (mapping, expected) in [
            (
                "com.example.A -> x:\ncom.example.B -> x:\n",
                "both map to x",
            ),
            (
                "com.example.A -> x:\ncom.example.A -> y:\n",
                "is mapped twice",
            ),
            (
                "com.example.A -> x:\n    void one(int) -> a\n    void two(int) -> a\n",
                "onto a (I)V",
            ),
        ] {
            let error = NameMap::parse(mapping).expect_err("a collision must be refused");

            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[test]
    fn normalized_classes_share_the_proguard_bijection_refusals() {
        for (mapping, expected) in [
            (
                "C\tcom/example/A\tx\nC\tcom/example/B\tx\n",
                "both map to x",
            ),
            (
                "C\tcom/example/A\tx\nC\tcom/example/A\ty\n",
                "is mapped twice",
            ),
        ] {
            let error = NameMap::normalized(mapping).expect_err("class collision");
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[test]
    fn a_line_the_parser_cannot_read_is_refused_rather_than_skipped() {
        for (mapping, expected) in [
            ("    void tick() -> a\n", "a member precedes every class"),
            ("com.example.A:\n", "not a valid proguard record"),
            (
                "com.example.A -> x:\n    nothing -> a\n",
                "not a valid proguard record",
            ),
            (
                "com.example.A -> x:\n    void tick( -> a\n",
                "not a valid proguard record",
            ),
            (
                "com.example.A -> x:\n    void tick([]) -> a\n",
                "names no type",
            ),
        ] {
            let error = NameMap::parse(mapping).expect_err("an unreadable line must be refused");

            assert!(error.to_string().contains(expected), "{error}");
            assert!(error.to_string().contains("at line"), "{error}");
        }
    }

    #[test]
    fn original_line_ranges_do_not_change_member_descriptors() {
        let mapping = NameMap::parse(
            "com.example.A -> x:\n    1:2:int[] read(java.lang.String,int[]):40:41 -> a\n\
             \tvoid tick():50 -> b\n",
        )
        .expect("both ProGuard original-line suffix forms are records");

        assert_eq!(
            mapping
                .runtime_member("com/example/A", "read", "(Ljava/lang/String;[I)[I")
                .expect("line metadata is not part of a descriptor"),
            ("a".to_owned(), "(Ljava/lang/String;[I)[I".to_owned())
        );
        assert_eq!(
            mapping
                .runtime_member("com/example/A", "tick", "()V")
                .expect("a suffix does not require a minified line prefix"),
            ("b".to_owned(), "()V".to_owned())
        );
    }

    #[test]
    fn malformed_records_are_not_accepted_as_partial_declarations() {
        for line in [
            "com.example.A -> x",
            "com.example.A -> x: trailing",
            "com.example.A -> x::",
            "com.example.A -> x:\n    1:void tick() -> a",
            "com.example.A -> x:\n    1:x:void tick() -> a",
            "com.example.A -> x:\n    void tick():x -> a",
            "com.example.A -> x:\n    void tick(int,,long) -> a",
        ] {
            let error = NameMap::parse(line).expect_err("every record must be complete");

            assert!(error.to_string().contains("at line"), "{error}");
        }
    }

    #[test]
    fn two_lines_declaring_one_member_are_refused_rather_than_last_wins() {
        let error =
            NameMap::parse("com.example.A -> x:\n    void tick() -> a\n    void tick() -> b\n")
                .expect_err("one member may name one linked member");

        assert!(error.to_string().contains("tick()V"), "{error}");
        assert!(error.to_string().contains("twice"), "{error}");
    }

    #[test]
    fn inline_frames_do_not_collide_with_the_outer_declaration() {
        let mapping = NameMap::parse(
            "com.example.A -> x:\n    1:1:void com.example.B.tick():5:5 -> a\n\
             \t1:1:void run():10:10 -> a\n",
        )
        .expect("an inline frame and its outer declaration share a linked pair");

        assert_eq!(
            mapping
                .runtime_member("com/example/A", "run", "()V")
                .expect("only the outer method declares the linked pair"),
            ("a".to_owned(), "()V".to_owned())
        );
        assert_eq!(mapping.mojang_member("com/example/A", "a", "()V"), "run");
        for name in ["tick", "com.example.B.tick"] {
            assert!(
                mapping
                    .runtime_member("com/example/A", name, "()V")
                    .is_err(),
                "an inline frame must not invent a declaration for {name}"
            );
        }
    }

    #[test]
    fn a_comment_is_a_comment_wherever_it_is_indented() {
        let mapping = NameMap::parse("com.example.A -> x:\n    # a member never follows\n")
            .expect("an indented comment is not a member");

        assert_eq!(mapping.runtime_class("com/example/A"), "x");
    }

    #[test]
    fn a_mapping_that_names_no_class_is_refused_rather_than_read_as_the_identity() {
        for text in ["", "\n", "# nothing but a comment\n"] {
            let error = NameMap::parse(text).expect_err("a source that translates must translate");

            assert!(error.to_string().contains("names no class"), "{error}");
        }
    }

    #[test]
    fn the_unobfuscated_source_translates_nothing_and_reads_no_text() {
        let mapping = Mapping::read(
            MappingSource::UnobfuscatedClassFiles,
            "not a mapping at all",
        )
        .expect("the identity needs no text");

        assert_eq!(
            mapping.runtime_class("com/example/Alpha"),
            "com/example/Alpha"
        );
        assert_eq!(
            mapping.mojang_class("com/example/Alpha"),
            "com/example/Alpha"
        );
        assert_eq!(
            mapping.entry("com/example/Alpha"),
            "com/example/Alpha.class"
        );
        assert_eq!(
            mapping
                .runtime_member("com/example/Alpha", "allowed", "(C)Z")
                .expect("nothing to translate"),
            ("allowed".to_owned(), "(C)Z".to_owned())
        );
    }

    #[test]
    fn the_proguard_source_asks_an_archive_for_the_linked_entry() {
        let mapping =
            Mapping::read(MappingSource::OfficialProguardMappings, MAPPING).expect("parses");

        assert_eq!(mapping.entry("com/example/Alpha"), "a.class");
        assert_eq!(mapping.entry("com/example/Beta"), "b$c.class");
    }

    /// One reviewed role, in the shape a version's reflective surface declares.
    fn surface(class: &str, runtime: &str) -> Reflection {
        Reflection {
            schema_version: 1,
            // Empty because nothing here reads it, and because no source in this
            // crate may spell a Minecraft version.
            minecraft: String::new(),
            classes: vec![Reflected {
                name: "ALPHA".to_owned(),
                class: class.to_owned(),
                runtime: runtime.to_owned(),
                members: Vec::new(),
            }],
        }
    }

    /// The obfuscated case the whole record exists for: the reviewed name is one
    /// thing, the name the JVM links is another, and only the mapping knows.
    #[test]
    fn a_proguard_surface_is_re_derived_from_the_mapping_rather_than_trusted() {
        let mapping =
            Mapping::read(MappingSource::OfficialProguardMappings, MAPPING).expect("parses");

        verify_reflection(&surface("com.example.Alpha", "a"), &mapping)
            .expect("the mapping resolves it to exactly that");

        let error = verify_reflection(&surface("com.example.Alpha", "b"), &mapping)
            .expect_err("a runtime name the mapping disagrees with must be refused");

        assert!(error.to_string().contains("links as b"), "{error}");
    }

    /// A surface copied from another version keeps a Mojang name the target
    /// renamed away. The mapping has no entry for it, so a pass-through would
    /// compare that name with itself and agree.
    #[test]
    fn a_class_a_proguard_mapping_never_names_is_refused_rather_than_matching_itself() {
        let mapping =
            Mapping::read(MappingSource::OfficialProguardMappings, MAPPING).expect("parses");

        let error = verify_reflection(
            &surface("com.example.Absent", "com.example.Absent"),
            &mapping,
        )
        .expect_err("a class the mapping does not name is not a class it agreed with");

        assert!(
            error
                .to_string()
                .contains("names no class com.example.Absent"),
            "{error}"
        );
    }

    /// Under the identity there is nothing to look up, so the reviewed name is
    /// the linked one and a declared runtime name that is not it is the defect.
    #[test]
    fn an_identity_surface_is_the_reviewed_name_compared_with_itself() {
        verify_reflection(
            &surface("com.example.Absent", "com.example.Absent"),
            &Mapping::Identity,
        )
        .expect("nothing translates it, so it links as itself");

        let error = verify_reflection(&surface("com.example.Absent", "a"), &Mapping::Identity)
            .expect_err("an unobfuscated version links the reviewed name");

        assert!(error.to_string().contains("links as a"), "{error}");
    }

    #[test]
    fn the_proguard_source_refuses_text_that_is_not_a_mapping() {
        let error = Mapping::read(
            MappingSource::OfficialProguardMappings,
            "not a mapping at all\n",
        )
        .expect_err("the source requires a real mapping");

        assert!(
            error.to_string().contains("not a valid proguard record"),
            "{error}"
        );
    }

    /// One class file declaring exactly the members it is given.
    ///
    /// Written out rather than reached for, because the point of every test
    /// below is a class file that declares something other than what the
    /// surface says, and no real jar holds one of those.
    fn class_file(
        class: &str,
        parent: &str,
        interfaces: &[&str],
        members: &[(&str, &str, bool, u16)],
    ) -> Vec<u8> {
        let mut pool = vec![class.to_owned(), parent.to_owned()];
        pool.extend(interfaces.iter().map(|name| (*name).to_owned()));
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        for (name, descriptor, method, flags) in members {
            pool.push((*name).to_owned());
            pool.push((*descriptor).to_owned());
            let index = u16::try_from(pool.len()).expect("a small pool");
            let entry = (index - 1, index, *flags);
            if *method {
                methods.push(entry);
            } else {
                fields.push(entry);
            }
        }

        let mut bytes = Vec::new();
        bytes.extend(0xCAFE_BABE_u32.to_be_bytes());
        bytes.extend(0_u16.to_be_bytes()); // minor
        bytes.extend(69_u16.to_be_bytes()); // major
        bytes.extend(
            u16::try_from(pool.len() + interfaces.len() + 3)
                .expect("a small pool")
                .to_be_bytes(),
        );
        for text in &pool {
            bytes.push(1); // modified UTF-8
            bytes.extend(u16::try_from(text.len()).expect("short").to_be_bytes());
            bytes.extend(text.as_bytes());
        }
        let this = u16::try_from(pool.len() + 1).expect("a small pool");
        for name in 1..=interfaces.len() + 2 {
            bytes.push(7); // Class
            bytes.extend(u16::try_from(name).expect("a small pool").to_be_bytes());
        }
        let flags = if members
            .iter()
            .any(|(_, _, _, flags)| flags & classfile::ACC_ABSTRACT != 0)
        {
            0x0601
        } else {
            0x0021
        };
        for value in [flags, this, this + 1] {
            bytes.extend(value.to_be_bytes()); // access, this, super
        }
        bytes.extend(u16::try_from(interfaces.len()).expect("few").to_be_bytes());
        for index in 0..interfaces.len() {
            bytes.extend((this + 2 + u16::try_from(index).expect("few")).to_be_bytes());
        }
        for table in [&fields, &methods] {
            bytes.extend(u16::try_from(table.len()).expect("few").to_be_bytes());
            for (name, descriptor, flags) in table {
                for value in [*flags, *name, *descriptor, 0] {
                    bytes.extend(value.to_be_bytes()); // flags, name, type, attributes
                }
            }
        }
        bytes.extend(0_u16.to_be_bytes()); // class attributes
        bytes
    }

    /// A surface of one class carrying one declared member.
    fn declaring(
        class: &str,
        name: &str,
        descriptor: Option<&str>,
        parameters: Option<usize>,
    ) -> Reflection {
        let mut surface = surface(class, class);
        surface.classes[0].members = vec![ReflectedMember {
            name: name.to_owned(),
            lookup: match (descriptor, parameters) {
                (Some(descriptor), None) => {
                    super::super::inputs::Lookup::Descriptor(descriptor.to_owned())
                }
                (None, Some(count)) => super::super::inputs::Lookup::Parameters(count),
                _ => panic!("the fixture must declare exactly one lookup"),
            },
        }];
        surface
    }

    /// An archive holding one class file under the entry the mapping asks for.
    fn holding(entry: &str, members: &[(&str, &str, bool)]) -> jar::Archive {
        let class = entry.strip_suffix(".class").expect("a class entry");
        let members = members
            .iter()
            .map(|(name, descriptor, method)| (*name, *descriptor, *method, 0))
            .collect::<Vec<_>>();
        fixture::parse(fixture::stored(
            entry,
            &class_file(class, "java/lang/Object", &[], &members),
        ))
        .expect("a synthetic archive")
    }

    fn hierarchy_archive(entries: &[(&str, Vec<u8>)]) -> jar::Archive {
        let entries = entries
            .iter()
            .map(|(name, bytes)| (*name, bytes.as_slice()))
            .collect::<Vec<_>>();
        fixture::parse(fixture::entries(&entries, 0)).expect("hierarchy archive")
    }

    fn inherited_fixture() -> (super::super::inputs::Inventory, Mapping, jar::Archive) {
        let inventory: super::super::inputs::Inventory = toml::from_str(
            "schemaVersion = 1\nminecraft = \"test\"\n\
             [[hook]]\nid = \"tick\"\nclass = \"example.Child\"\n\
             member = \"tick\"\ndescriptor = \"()V\"\n\
             kind = \"instance-method\"\ncapability = \"test\"\n",
        )
        .expect("concrete hook");
        let mapping = Mapping::tiny(INHERITED_RECORDS).expect("interface mapping");
        let archive = hierarchy_archive(&[
            (
                "a.class",
                class_file("a", "p", &[], &[("b", "()V", true, 1)]),
            ),
            ("p.class", class_file("p", "java/lang/Object", &["i"], &[])),
            (
                "i.class",
                class_file("i", "java/lang/Object", &[], &[("b", "()V", true, 0x0401)]),
            ),
        ]);
        (inventory, mapping, archive)
    }

    const INHERITED_RECORDS: &str = "C\texample/Child\ta\nC\texample/Parent\tp\n\
        C\texample/Contract\ti\nM\texample/Contract\ttick\t()V\tb\n";

    #[test]
    fn tiny_instance_overrides_follow_multilevel_interface_declarations() {
        let (inventory, mapping, archive) = inherited_fixture();
        let resolved = super::super::resolve::from_jar(&archive, &inventory, &mapping)
            .expect("multilevel interface override");
        assert_eq!(resolved[0].runtime_member, "b");
    }

    #[test]
    fn tiny_superclass_names_still_require_a_concrete_override() {
        let (inventory, _, _) = inherited_fixture();
        let parent_mapping = Mapping::tiny(
            "C\texample/Child\ta\nC\texample/Parent\tp\nM\texample/Parent\ttick\t()V\tb\n",
        )
        .expect("parent mapping");
        let parent_archive = hierarchy_archive(&[
            (
                "a.class",
                class_file("a", "p", &[], &[("b", "()V", true, 1)]),
            ),
            (
                "p.class",
                class_file("p", "java/lang/Object", &[], &[("b", "()V", true, 1)]),
            ),
        ]);
        super::super::resolve::from_jar(&parent_archive, &inventory, &parent_mapping)
            .expect("a superclass can name a concrete override");
        let wrong_concrete = hierarchy_archive(&[
            (
                "a.class",
                class_file("a", "p", &[], &[("c", "()V", true, 1)]),
            ),
            (
                "p.class",
                class_file("p", "java/lang/Object", &[], &[("b", "()V", true, 1)]),
            ),
        ]);
        super::super::resolve::from_jar(&wrong_concrete, &inventory, &parent_mapping)
            .expect_err("an inherited name still requires the concrete declaration");
    }

    #[test]
    fn inherited_names_are_tiny_instance_only_and_respect_explicit_child_names() {
        let (inventory, mapping, archive) = inherited_fixture();
        let records = INHERITED_RECORDS;
        let resolve = |mapping: &Mapping, inventory: &super::super::inputs::Inventory| {
            super::super::resolve::from_jar(&archive, inventory, mapping)
        };
        for kind in [
            super::super::inputs::Kind::StaticMethod,
            super::super::inputs::Kind::InstanceField,
            super::super::inputs::Kind::StaticField,
        ] {
            let mut wrong = inventory.clone();
            wrong.hooks[0].kind = kind;
            resolve(&mapping, &wrong).expect_err("inheritance is instance-method only");
        }
        let explicit_other = Mapping::tiny(&format!("{records}M\texample/Child\tother\t()V\tb\n"))
            .expect("explicit child name");
        resolve(&explicit_other, &inventory)
            .expect_err("explicit child names cannot be overridden");
        let mut official = mapping;
        let Mapping::NameMap(names) = &mut official else {
            panic!("name map")
        };
        names.inherited_hooks = false;
        resolve(&official, &inventory).expect_err("official mappings do not inherit names");
    }

    #[test]
    fn inherited_names_require_accessible_unambiguous_ancestor_declarations() {
        let (inventory, mapping, _) = inherited_fixture();
        let records = INHERITED_RECORDS;
        for flags in [
            classfile::ACC_PRIVATE,
            classfile::ACC_STATIC | classfile::ACC_PUBLIC,
        ] {
            let inaccessible = hierarchy_archive(&[
                (
                    "a.class",
                    class_file("a", "java/lang/Object", &["i"], &[("b", "()V", true, 1)]),
                ),
                (
                    "i.class",
                    class_file("i", "java/lang/Object", &[], &[("b", "()V", true, flags)]),
                ),
            ]);
            super::super::resolve::from_jar(&inaccessible, &inventory, &mapping)
                .expect_err("private and static ancestor methods are not overrides");
        }
        let missing = hierarchy_archive(&[(
            "a.class",
            class_file("a", "java/lang/Object", &["i"], &[("b", "()V", true, 1)]),
        )]);
        super::super::resolve::from_jar(&missing, &inventory, &mapping)
            .expect_err("a mapping without actual ancestor declaration proves nothing");
        let conflict = Mapping::tiny(&format!(
            "{records}C\texample/Other\tj\nM\texample/Other\ttick\t()V\tc\n"
        ))
        .expect("two inherited names");
        let conflicting = hierarchy_archive(&[
            (
                "a.class",
                class_file(
                    "a",
                    "java/lang/Object",
                    &["i", "j"],
                    &[("b", "()V", true, 1), ("c", "()V", true, 1)],
                ),
            ),
            (
                "i.class",
                class_file("i", "java/lang/Object", &[], &[("b", "()V", true, 0x0401)]),
            ),
            (
                "j.class",
                class_file("j", "java/lang/Object", &[], &[("c", "()V", true, 0x0401)]),
            ),
        ]);
        super::super::resolve::from_jar(&conflicting, &inventory, &conflict)
            .expect_err("conflicting inherited pairs cannot choose a concrete declaration");
    }

    /// The identity is where this check has teeth: nothing translates, so
    /// nothing else in the generator ever asks whether the member is there.
    #[test]
    fn an_identity_source_proves_every_declared_member_against_the_class_file() {
        let archive = holding(
            "com/example/Alpha.class",
            &[
                ("allowed", "(C)Z", true),
                ("PATTERN", "Ljava/util/regex/Pattern;", false),
            ],
        );

        for (member, descriptor) in [
            ("allowed", "(C)Z"),
            ("PATTERN", "Ljava/util/regex/Pattern;"),
        ] {
            verify_reflected_members(
                &declaring("com.example.Alpha", member, Some(descriptor), None),
                &Mapping::Identity,
                &archive,
            )
            .expect("the class file declares exactly that");
        }

        let error = verify_reflected_members(
            &declaring("com.example.Alpha", "allowed", Some("(I)Z"), None),
            &Mapping::Identity,
            &archive,
        )
        .expect_err("a descriptor the class file does not declare must be refused");

        assert!(error.to_string().contains("com.example.Alpha"), "{error}");
        assert!(error.to_string().contains("allowed(C)Z"), "{error}");
    }

    #[test]
    fn tiny_omitted_initializers_still_require_exact_linked_classfile_declarations() {
        let mapping = Mapping::tiny("C\tcom/example/Alpha\ta\nC\tcom/example/Beta\tb\n")
            .expect("Feather-style class-only mappings omit constructors");
        let archive = holding(
            "a.class",
            &[("<init>", "(Lb;)V", true), ("<clinit>", "()V", true)],
        );
        for (name, descriptor) in [("<init>", "(Lcom/example/Beta;)V"), ("<clinit>", "()V")] {
            verify_reflected_members(
                &declaring("com.example.Alpha", name, Some(descriptor), None),
                &mapping,
                &archive,
            )
            .expect("initializer names remain invariant and descriptors translate");
        }
        verify_reflected_members(
            &declaring("com.example.Alpha", "<init>", Some("()V"), None),
            &mapping,
            &archive,
        )
        .expect_err("a constructor omitted from the class file is not invented");
        assert!(
            mapping
                .runtime_member("com/example/Alpha", "unknown", "()V")
                .is_err()
        );
    }

    #[test]
    fn tiny_omitted_constructor_hooks_require_the_exact_declared_body() {
        let mut inventory: super::super::inputs::Inventory = toml::from_str(
            "schemaVersion = 1\nminecraft = \"test\"\n\
             [[hook]]\nid = \"construct\"\nclass = \"example.Alpha\"\n\
             member = \"<init>\"\ndescriptor = \"(Lexample/Beta;)V\"\n\
             kind = \"instance-method\"\ncapability = \"test\"\n",
        )
        .expect("reviewed constructor hook");
        let mapping = Mapping::tiny("C\texample/Alpha\ta\nC\texample/Beta\tb\n")
            .expect("constructors are absent from Feather mappings");
        let archive = holding("a.class", &[("<init>", "(Lb;)V", true)]);
        let resolved = super::super::resolve::from_jar(&archive, &inventory, &mapping)
            .expect("the class file declares the translated constructor");
        assert_eq!(resolved[0].runtime_member, "<init>");
        assert_eq!(resolved[0].runtime_descriptor, "(Lb;)V");
        inventory.hooks[0].descriptor = "()V".to_owned();
        super::super::resolve::from_jar(&archive, &inventory, &mapping)
            .expect_err("a different constructor body is not a match");
    }

    #[test]
    fn a_member_the_class_file_names_nowhere_is_refused_by_name() {
        let error = verify_reflected_members(
            &declaring("com.example.Alpha", "absent", Some("()V"), None),
            &Mapping::Identity,
            &holding("com/example/Alpha.class", &[("allowed", "(C)Z", true)]),
        )
        .expect_err("a member nothing declares must be refused");

        assert!(error.to_string().contains("no absent at all"), "{error}");
    }

    /// The case a published mapping cannot answer on its own: it says what the
    /// name becomes, and the class file is the only thing that says the class
    /// still declares it.
    #[test]
    fn a_member_a_translating_mapping_names_and_the_jar_dropped_is_refused() {
        let mapping = Mapping::NameMap(parsed());

        verify_reflected_members(
            &declaring("com.example.Alpha", "allowed", Some("(C)Z"), None),
            &mapping,
            &holding("a.class", &[("a", "(C)Z", true)]),
        )
        .expect("the linked class file declares the linked pair");

        let error = verify_reflected_members(
            &declaring("com.example.Alpha", "allowed", Some("(C)Z"), None),
            &mapping,
            &holding("a.class", &[("b", "(C)Z", true)]),
        )
        .expect_err("a linked pair the class file does not declare must be refused");

        assert!(error.to_string().contains("links as a(C)Z"), "{error}");
        assert!(error.to_string().contains("no a at all"), "{error}");
    }

    #[test]
    fn a_declared_width_the_class_file_does_not_declare_is_refused() {
        let archive = holding(
            "com/example/Alpha.class",
            &[("tick", "(II)V", true), ("tick", "()V", true)],
        );

        for width in [0, 2] {
            verify_reflected_members(
                &declaring("com.example.Alpha", "tick", None, Some(width)),
                &Mapping::Identity,
                &archive,
            )
            .expect("one of the two overloads is that wide");
        }

        let error = verify_reflected_members(
            &declaring("com.example.Alpha", "tick", None, Some(1)),
            &Mapping::Identity,
            &archive,
        )
        .expect_err("a width neither overload has must be refused");

        assert!(error.to_string().contains("tick taking 1"), "{error}");
        assert!(error.to_string().contains("tick()V"), "{error}");
    }

    /// A counted lookup explicitly requests a method, not a field.
    #[test]
    fn a_field_is_not_the_member_a_declared_width_asked_for() {
        let wanted = super::Linked::Counted {
            name: "tick".to_owned(),
            parameters: 0,
        };
        assert!(wanted.is_method(), "argument counts always request methods");
        let members = classfile::members(&class_file(
            "com/example/Alpha",
            "java/lang/Object",
            &[],
            &[("tick", "I", false, 0), ("tick", "()V", true, 0)],
        ))
        .expect("valid field and method tables decode independently");
        assert!(!members[0].method);
        assert!(members[1].method);
        assert!(!wanted.matches(&members[0]));
        assert!(wanted.matches(&members[1]));

        let error = verify_reflected_members(
            &declaring("com.example.Alpha", "tick", None, Some(0)),
            &Mapping::Identity,
            &holding("com/example/Alpha.class", &[("tick", "I", false)]),
        )
        .expect_err("a field does not declare the requested zero-argument method");

        assert!(error.to_string().contains("tick taking 0"), "{error}");
    }

    #[test]
    fn a_width_a_translating_mapping_resolves_is_required_as_the_pair_it_resolves_to() {
        let mapping = Mapping::NameMap(parsed());

        verify_reflected_members(
            &declaring("com.example.Alpha", "allowed", None, Some(1)),
            &mapping,
            &holding("a.class", &[("a", "(C)Z", true)]),
        )
        .expect("the mapping knows the one allowed of that width");

        let error = verify_reflected_members(
            &declaring("com.example.Alpha", "allowed", None, Some(1)),
            &mapping,
            &holding("a.class", &[("a", "(I)Z", true)]),
        )
        .expect_err("the width is not the whole answer where a mapping knows the descriptor");

        assert!(error.to_string().contains("links as a(C)Z"), "{error}");
    }

    #[test]
    fn a_width_a_translating_mapping_cannot_separate_names_no_member() {
        let mapping = Mapping::NameMap(
            NameMap::parse(
                "com.example.Alpha -> a:\n    \
                 5:5:void tick(int) -> a\n    \
                 6:6:void tick(char) -> b\n",
            )
            .expect("two overloads of one name is one mapping"),
        );

        let error = verify_reflected_members(
            &declaring("com.example.Alpha", "tick", None, Some(1)),
            &mapping,
            &holding("a.class", &[("a", "(I)V", true), ("b", "(C)V", true)]),
        )
        .expect_err("a width that separates nothing names neither of them");

        assert!(error.to_string().contains("declares 2 members"), "{error}");
        assert!(error.to_string().contains("tick taking 1"), "{error}");
    }

    #[test]
    fn a_member_the_translating_mapping_never_names_is_refused_rather_than_passed_through() {
        for (descriptor, parameters, expected) in [
            (Some("()V"), None, "but not its absent()V"),
            (None, Some(0), "no absent of it taking 0"),
        ] {
            let error = verify_reflected_members(
                &declaring("com.example.Alpha", "absent", descriptor, parameters),
                &Mapping::NameMap(parsed()),
                &holding("a.class", &[("absent", "()V", true)]),
            )
            .expect_err("a member the mapping says nothing about is not one it agreed with");

            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[test]
    fn a_member_declared_in_no_way_or_in_two_is_refused_before_the_jar_is_asked() {
        for declaration in [
            "name = \"tick\"\ndescriptor = \"()V\"\nparameters = 0",
            "name = \"tick\"",
        ] {
            let error = toml::from_str::<ReflectedMember>(declaration)
                .expect_err("a member that says which member it is twice or never must be refused");

            assert!(error.to_string().contains("tick"), "{error}");
            assert!(
                error
                    .to_string()
                    .contains("exactly one of descriptor or parameters"),
                "{error}"
            );
        }
    }

    /// A class reached only as a parameter type declares nothing, and an archive
    /// that does not even hold it is what proves nothing was asked of it.
    #[test]
    fn a_class_declaring_no_member_asks_the_archive_for_nothing() {
        verify_reflected_members(
            &surface("com.example.Alpha", "com.example.Alpha"),
            &Mapping::Identity,
            &holding("some/Other.class", &[("tick", "()V", true)]),
        )
        .expect("nothing was declared, so nothing had to be found");
    }
}

#[cfg(test)]
mod tiny_tests {
    use super::{Linked, Mapping};

    const NORMALIZED: &str = "\
C\treviewed/Alpha\ta
C\treviewed/Beta\tb
F\treviewed/Alpha\tvalue\t[Lreviewed/Beta;\tf
M\treviewed/Alpha\tread\t(Lreviewed/Beta;)Lreviewed/Beta;\tm
M\treviewed/Alpha\tread\t(I)Lreviewed/Beta;\tn
";

    #[test]
    fn tiny_descriptors_and_overloads_resolve_in_both_directions() {
        let mapping = Mapping::tiny(NORMALIZED).expect("normalized mappings");
        assert_eq!(mapping.runtime_class("reviewed/Alpha"), "a");
        assert_eq!(mapping.mojang_class("b"), "reviewed/Beta");
        assert_eq!(
            mapping
                .runtime_member("reviewed/Alpha", "read", "(I)Lreviewed/Beta;")
                .expect("integer overload"),
            ("n".to_owned(), "(I)Lb;".to_owned())
        );
        assert_eq!(
            mapping.mojang_member("reviewed/Alpha", "m", "(Lb;)Lb;"),
            "read"
        );
        assert_eq!(
            mapping.linked_member("reviewed/Alpha", "value", "[Lreviewed/Beta;"),
            ("f".to_owned(), "[Lb;".to_owned())
        );
        assert_eq!(
            mapping.mojang_descriptor("(Lb;)Lb;"),
            "(Lreviewed/Beta;)Lreviewed/Beta;"
        );
        assert!(
            mapping
                .linked_by_arity("reviewed/Alpha", "read", 1)
                .is_err()
        );
        let single = Mapping::tiny("C\tA\ta\nM\tA\ttick\t()V\tm\n").expect("one method");
        assert_eq!(
            single
                .linked_by_arity("A", "tick", 0)
                .expect("unique width"),
            Linked::Exact {
                name: "m".to_owned(),
                descriptor: "()V".to_owned()
            }
        );
    }

    #[test]
    fn tiny_refuses_duplicate_or_ambiguous_mapping_keys() {
        for text in [
            "C\tA\ta\nC\tA\tb\n",
            "C\tA\ta\nC\tB\ta\n",
            "C\tA\ta\nM\tA\tread\t()V\tm\nM\tA\tread\t()V\tn\n",
            "C\tA\ta\nM\tA\tread\t()V\tm\nM\tA\twrite\t()V\tm\n",
            "C\tA\ta\nF\tA\tfirst\tI\tf\nF\tA\tsecond\tI\tf\n",
        ] {
            assert!(Mapping::tiny(text).is_err(), "{text}");
        }
    }

    #[test]
    fn tiny_refuses_bad_protocol_names_descriptors_and_owners() {
        for text in [
            "",
            "C\tA\n",
            "C\tA\ta\textra\n",
            "C\t\tb\n",
            "C\tA.b\ta\n",
            "C\tA\ta\nM\tB\ttick\t()V\tm\n",
            "C\tA\ta\nF\tA\tvalue\tV\tf\n",
            "C\tA\ta\nF\tA\tvalue\t[V\tf\n",
            "C\tA\ta\nF\tA\tvalue\tII\tf\n",
            "C\tA\ta\nF\tA\tvalue\tL;\tf\n",
            "C\tA\ta\nM\tA\ttick\t(I)\tm\n",
            "C\tA\ta\nM\tA\ttick\t([V)V\tm\n",
            "C\tA\ta\nM\tA\ttick\t()VZ\tm\n",
            "C\tA\ta\nM\tA\ttick\t()LA.B;\tm\n",
            "C\tA\ta\nF\tA\tvalue\t()V\tf\n",
            "C\tA\ta\nM\tA\ttick\tI\tm\n",
            "tiny\t2\t0\tofficial\tnamed\n",
        ] {
            assert!(Mapping::tiny(text).is_err(), "{text}");
        }
    }

    #[test]
    fn tiny_does_not_treat_an_empty_mapped_class_as_an_unmapped_library() {
        let mapping = Mapping::tiny("C\tA\ta\n").expect("a class without members");
        assert!(mapping.runtime_member("A", "tick", "()V").is_err());
        assert!(mapping.linked_class("absent").is_err());
        assert_eq!(
            mapping
                .runtime_member("library/Type", "tick", "()V")
                .expect("unmapped library member"),
            ("tick".to_owned(), "()V".to_owned())
        );
    }
}
