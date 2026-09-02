//! Relating the names a reviewed inventory carries to the names a jar declares.
//!
//! ADG-3's amendment admits two mapping sources. Under `unobfuscated-class-files`
//! the jar declares the names the inventory reviews and this layer is the
//! identity. Under `official-proguard-mappings` the jar declares obfuscated names
//! and the version's own published mapping file says which is which, so every
//! name the generator hands to an archive goes through here first.

use std::collections::{BTreeMap, BTreeSet};

use super::GenerateError;
use super::inputs::MappingSource;

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
}

/// A member's name paired with its descriptor.
type Member = (String, String);

/// Every member of one class, from the reviewed name to the linked one.
type Members = BTreeMap<Member, Member>;

/// One parsed proguard mapping file.
#[derive(Clone, Debug)]
pub(super) struct Proguard {
    /// Reviewed internal name to linked internal name.
    classes: BTreeMap<String, String>,
    /// Linked internal name back to the reviewed one.
    mojang: BTreeMap<String, String>,
    /// Reviewed internal class name, then reviewed member and descriptor.
    members: BTreeMap<String, Members>,
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

        // Descriptors are rewritten once every class is known, because a
        // parameter can name a class the file has not reached yet.
        let mut members = BTreeMap::new();
        for (class, entries) in declared {
            let mut forward = BTreeMap::new();
            let mut linked = BTreeSet::new();
            for entry in entries {
                let runtime = map_descriptor(&classes, &entry.descriptor);
                if !linked.insert((entry.runtime.clone(), runtime.clone())) {
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
            members.insert(class, forward);
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
        })
    }

    fn runtime_class<'a>(&'a self, mojang: &'a str) -> &'a str {
        self.classes.get(mojang).map_or(mojang, String::as_str)
    }

    fn mojang_class<'a>(&'a self, runtime: &'a str) -> &'a str {
        self.mojang.get(runtime).map_or(runtime, String::as_str)
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
    use super::{Mapping, MappingSource, Proguard};

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
}
