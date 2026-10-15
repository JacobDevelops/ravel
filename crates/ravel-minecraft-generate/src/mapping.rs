//! Maps reviewed names to linked jar names, using identity or official Proguard mappings.

use std::collections::BTreeMap;
use std::fmt;

use super::inputs::{Lookup, MappingSource, Reflected, ReflectedMember, Reflection};
use super::{GenerateError, classfile, jar};

/// How a Mojang name becomes the name the JVM links.
#[derive(Clone, Debug)]
pub(super) enum Mapping {
    /// The jar declares the reviewed names, so nothing is translated.
    Identity,
    /// The published proguard mapping decides every name.
    Proguard(Proguard),
}

impl Mapping {
    /// Builds the mapping one source calls for.
    ///
    /// # Errors
    ///
    /// Returns an error when a proguard mapping is required and its text does
    /// not parse, or when it maps two names onto one.
    pub(super) fn read(source: MappingSource, text: &str) -> Result<Self, GenerateError> {
        match source {
            MappingSource::UnobfuscatedClassFiles => Ok(Self::Identity),
            MappingSource::OfficialProguardMappings => Proguard::parse(text).map(Self::Proguard),
        }
    }

    /// The internal name the JVM links for a reviewed internal name.
    pub(super) fn runtime_class<'a>(&'a self, mojang: &'a str) -> &'a str {
        match self {
            Self::Identity => mojang,
            Self::Proguard(proguard) => proguard.runtime_class(mojang),
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
            Self::Proguard(proguard) => proguard.linked_class(mojang),
        }
    }

    /// The reviewed internal name a linked internal name came from.
    pub(super) fn mojang_class<'a>(&'a self, runtime: &'a str) -> &'a str {
        match self {
            Self::Identity => runtime,
            Self::Proguard(proguard) => proguard.mojang_class(runtime),
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
            Self::Proguard(proguard) => map_descriptor(&proguard.mojang, descriptor),
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
            Self::Proguard(proguard) => proguard.mojang_member(mojang_class, member, descriptor),
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
            Self::Proguard(proguard) => proguard.linked_member(mojang_class, member, descriptor),
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
            Self::Proguard(proguard) => proguard.runtime_member(mojang_class, member, descriptor),
        }
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
            Self::Proguard(proguard) => proguard.linked_by_arity(mojang_class, member, parameters),
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
                mapping.runtime_member(mojang_class, &member.name, descriptor)?;
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
        holds(declared, linked.name())
    )))
}

/// What one class file declares under a linked name, for a refusal to name.
fn holds(declared: &[classfile::Member], name: &str) -> String {
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

/// One parsed proguard mapping file.
#[derive(Clone, Debug)]
pub(super) struct Proguard {
    /// Reviewed internal name to linked internal name.
    classes: BTreeMap<String, String>,
    /// Linked internal name back to the reviewed one.
    mojang: BTreeMap<String, String>,
    /// Reviewed internal class name, then reviewed member and descriptor.
    members: BTreeMap<String, Members>,
    /// Reviewed internal class name, then linked member and descriptor.
    linked: BTreeMap<String, Reviewed>,
}

impl Proguard {
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
            if line.starts_with(char::is_whitespace) {
                let class = current
                    .as_ref()
                    .ok_or_else(|| unusable(number, "a member precedes every class"))?;
                let member =
                    Declared::parse(trimmed).map_err(|detail| unusable(number, &detail))?;
                declared.entry(class.clone()).or_default().push(member);
                continue;
            }

            let (from, to) =
                split(trimmed.trim_end_matches(':')).map_err(|detail| unusable(number, &detail))?;
            let from = internal(from);
            let to = internal(to);
            if classes.insert(from.clone(), to.clone()).is_some() {
                return Err(unusable(number, &format!("{from} is mapped twice")));
            }
            if let Some(other) = mojang.insert(to.clone(), from.clone()) {
                return Err(unusable(
                    number,
                    &format!("{other} and {from} both map to {to}"),
                ));
            }
            current = Some(from);
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
    /// Reads one member line, without its leading indentation.
    ///
    /// A method carries the line range it occupies, which says nothing about the
    /// name and is dropped. What is kept is the declared type, the name, and the
    /// parameter list, because those are the descriptor the inventory reviews.
    fn parse(line: &str) -> Result<Self, String> {
        let (declaration, runtime) = split(line)?;
        let declaration = declaration.trim_start_matches(|c: char| c.is_ascii_digit() || c == ':');
        let (returns, rest) = declaration
            .split_once(' ')
            .ok_or_else(|| format!("{declaration} declares no type"))?;
        let returns = descriptor_of(returns)?;

        let Some((member, parameters)) = rest.split_once('(') else {
            return Ok(Self {
                member: rest.to_owned(),
                descriptor: returns,
                runtime: runtime.to_owned(),
            });
        };
        let parameters = parameters
            .strip_suffix(')')
            .ok_or_else(|| format!("{rest} never closes its parameter list"))?;
        let mut descriptor = String::from("(");
        for parameter in parameters
            .split(',')
            .map(str::trim)
            .filter(|text| !text.is_empty())
        {
            descriptor.push_str(&descriptor_of(parameter)?);
        }
        descriptor.push(')');
        descriptor.push_str(&returns);
        Ok(Self {
            member: member.to_owned(),
            descriptor,
            runtime: runtime.to_owned(),
        })
    }
}

/// A refusal naming the line the mapping stopped being readable on.
fn unusable(number: usize, detail: &str) -> GenerateError {
    GenerateError::Input(format!(
        "the client mapping is unusable at line {number}: {detail}"
    ))
}

/// Splits one `from -> to` line.
fn split(line: &str) -> Result<(&str, &str), String> {
    line.split_once(" -> ")
        .ok_or_else(|| format!("{line} maps nothing onto anything"))
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
    use super::super::inputs::{Reflected, ReflectedMember, Reflection};
    use super::super::jar::{self, fixture};
    use super::{Mapping, MappingSource, Proguard, verify_reflected_members, verify_reflection};

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

    fn parsed() -> Proguard {
        Proguard::parse(MAPPING).expect("the invented mapping should parse")
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
        let mapping = Mapping::Proguard(parsed());

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
        let mapping = Mapping::Proguard(
            Proguard::parse(
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
        let mapping = Mapping::Proguard(parsed());

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
            let error = Proguard::parse(mapping).expect_err("a collision must be refused");

            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[test]
    fn a_line_the_parser_cannot_read_is_refused_rather_than_skipped() {
        for (mapping, expected) in [
            ("    void tick() -> a\n", "a member precedes every class"),
            ("com.example.A:\n", "maps nothing onto anything"),
            (
                "com.example.A -> x:\n    nothing -> a\n",
                "declares no type",
            ),
            (
                "com.example.A -> x:\n    void tick( -> a\n",
                "never closes its parameter list",
            ),
            (
                "com.example.A -> x:\n    void tick([]) -> a\n",
                "names no type",
            ),
        ] {
            let error = Proguard::parse(mapping).expect_err("an unreadable line must be refused");

            assert!(error.to_string().contains(expected), "{error}");
            assert!(error.to_string().contains("at line"), "{error}");
        }
    }

    #[test]
    fn two_lines_declaring_one_member_are_refused_rather_than_last_wins() {
        let error =
            Proguard::parse("com.example.A -> x:\n    void tick() -> a\n    void tick() -> b\n")
                .expect_err("one member may name one linked member");

        assert!(error.to_string().contains("tick()V"), "{error}");
        assert!(error.to_string().contains("twice"), "{error}");
    }

    #[test]
    fn a_comment_is_a_comment_wherever_it_is_indented() {
        let mapping = Proguard::parse("com.example.A -> x:\n    # a member never follows\n")
            .expect("an indented comment is not a member");

        assert_eq!(mapping.runtime_class("com/example/A"), "x");
    }

    #[test]
    fn a_mapping_that_names_no_class_is_refused_rather_than_read_as_the_identity() {
        for text in ["", "\n", "# nothing but a comment\n"] {
            let error = Proguard::parse(text).expect_err("a source that translates must translate");

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
            error.to_string().contains("maps nothing onto anything"),
            "{error}"
        );
    }

    /// One class file declaring exactly the members it is given.
    ///
    /// Written out rather than reached for, because the point of every test
    /// below is a class file that declares something other than what the
    /// surface says, and no real jar holds one of those.
    fn class_file(members: &[(&str, &str, bool)]) -> Vec<u8> {
        let mut pool = Vec::new();
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        for (name, descriptor, method) in members {
            pool.push((*name).to_owned());
            pool.push((*descriptor).to_owned());
            let index = u16::try_from(pool.len()).expect("a small pool");
            let entry = (index - 1, index);
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
            u16::try_from(pool.len() + 1)
                .expect("a small pool")
                .to_be_bytes(),
        );
        for text in &pool {
            bytes.push(1); // modified UTF-8
            bytes.extend(u16::try_from(text.len()).expect("short").to_be_bytes());
            bytes.extend(text.as_bytes());
        }
        for value in [0_u16, 0, 0, 0] {
            bytes.extend(value.to_be_bytes()); // access, this, super, interfaces
        }
        for table in [&fields, &methods] {
            bytes.extend(u16::try_from(table.len()).expect("few").to_be_bytes());
            for (name, descriptor) in table {
                for value in [0_u16, *name, *descriptor, 0] {
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
        fixture::parse(fixture::stored(entry, &class_file(members))).expect("a synthetic archive")
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
        let mapping = Mapping::Proguard(parsed());

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

    /// A width is a method's, so a field of that name is not the member meant.
    /// The class file is built to declare one anyway, carrying a descriptor no
    /// field could have, because a fixture that could not hold the defect proves
    /// nothing about the check that refuses it.
    #[test]
    fn a_field_is_not_the_member_a_declared_width_asked_for() {
        let error = verify_reflected_members(
            &declaring("com.example.Alpha", "tick", None, Some(0)),
            &Mapping::Identity,
            &holding("com/example/Alpha.class", &[("tick", "()V", false)]),
        )
        .expect_err("a field is not a method however it is spelled");

        assert!(error.to_string().contains("tick taking 0"), "{error}");
    }

    #[test]
    fn a_width_a_translating_mapping_resolves_is_required_as_the_pair_it_resolves_to() {
        let mapping = Mapping::Proguard(parsed());

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
        let mapping = Mapping::Proguard(
            Proguard::parse(
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
                &Mapping::Proguard(parsed()),
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
