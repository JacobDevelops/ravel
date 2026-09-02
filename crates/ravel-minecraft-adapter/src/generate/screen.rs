//! Rendering the Ravel-owned screen sources in the name space one version links.
//!
//! ADG-6 draws Ravel's own screens in Java, and a screen extends the game's
//! `Screen` and overrides its methods. A compile-time reference is a constant
//! pool entry, so those names cannot be resolved at runtime the way the agent
//! jar's adapters resolve theirs through `GameNames`: what javac writes is what
//! the JVM links. Under an unobfuscated version the reviewed name is that name;
//! under an obfuscated one it is not, and a hand-written source is then a jar
//! that compiles and links nothing.
//!
//! So the sources are generated. The tracked half is a template that names roles
//! inside `{{…}}` — never a game class, because a template is shared by every
//! version and a class name is one version's spelling asserted over the rest.
//! Each version's `stubs.toml` binds a role to the class it means there and a
//! member role to the member, exactly as `reflection.toml` binds the roles the
//! agent jar resolves, and this module rewrites each placeholder into the name
//! that version links. That indirection is what carries a template across a
//! rename: 1.21.1 calls `Identifier` `ResourceLocation` and spells
//! `splitIgnoringLanguage` `split`, and neither costs the template a word.
//! Everything outside a placeholder is copied through byte for byte, which is
//! what keeps a template reviewable as the Java it is and what makes an
//! unobfuscated version's output the same source it was before it was
//! generated.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::inputs::{Adapter, Seeds};
use super::mapping::Mapping;
use super::stubs::Stub;
use super::{GenerateError, SCREEN_PACKAGE};

/// The suffix a template carries, over the name of the source it renders.
const TEMPLATE_SUFFIX: &str = ".in";

/// Everything one version's screen sources come to.
pub(super) struct Screens {
    /// Each source's path below the generated tree, with its Java.
    pub(super) java: Vec<(String, String)>,
    /// The templates a build has to watch.
    pub(super) templates: Vec<PathBuf>,
    /// Each screen class's constant name and the binary name it was emitted as.
    pub(super) names: Vec<(String, String)>,
}

/// Renders every tracked template into the name space the version links.
///
/// # Errors
///
/// Returns an error when a template is unreadable, when it names a class the
/// version's stub seeds do not carry, when it names a member that class does not
/// declare, or when the names it asks for cannot be written down together.
pub(super) fn render(
    directory: &Path,
    adapter: &Adapter,
    seeds: &Seeds,
    stubs: &[Stub],
    mapping: &Mapping,
) -> Result<Screens, GenerateError> {
    let templates = templates(directory)?;
    let mut read = Vec::with_capacity(templates.len());
    for path in &templates {
        read.push(template(path)?);
    }

    let mut referenced = BTreeSet::new();
    for template in &read {
        for placeholder in &template.placeholders {
            if let Placeholder::Class(role) | Placeholder::Import(role) = placeholder {
                referenced.insert(role.clone());
            }
        }
    }

    every_seeded_member_resolves(seeds, stubs, mapping)?;
    let linked = linked_classes(&referenced, seeds, stubs, mapping)?;
    // A class the mapping puts in the unnamed package cannot be named from a
    // package at all — the language has no syntax for it — so the screens go
    // where their supertypes are. Under a version that keeps the game in
    // packages they stay where ADG-6 put them, which is why an unobfuscated
    // version generates the sources it already had.
    let unnamed = linked.values().any(|name| !name.contains('/'));
    let package = (!unnamed).then(|| SCREEN_PACKAGE.to_owned());

    let mut java = Vec::with_capacity(read.len());
    let mut names = Vec::with_capacity(read.len());
    for template in &read {
        let source = expand(
            template,
            adapter,
            seeds,
            stubs,
            mapping,
            &linked,
            package.as_deref(),
        )?;
        let (relative, binary) = match &package {
            Some(package) => (
                format!("{}/{}.java", package.replace('.', "/"), template.name),
                format!("{package}.{}", template.name),
            ),
            None => (format!("{}.java", template.name), template.name.clone()),
        };
        java.push((relative, source));
        names.push((constant(&template.name), binary));
    }
    Ok(Screens {
        java,
        templates,
        names,
    })
}

/// Every template under the tracked directory, in a stable order.
fn templates(directory: &Path) -> Result<Vec<PathBuf>, GenerateError> {
    let mut found = Vec::new();
    let entries = fs::read_dir(directory).map_err(|error| {
        GenerateError::Input(format!(
            "cannot read the screen templates at {}: {error}",
            directory.display()
        ))
    })?;
    for entry in entries {
        let path = entry
            .map_err(|error| {
                GenerateError::Input(format!("cannot read a screen template: {error}"))
            })?
            .path();
        if path
            .to_str()
            .is_some_and(|name| name.ends_with(TEMPLATE_SUFFIX))
        {
            found.push(path);
        }
    }
    if found.is_empty() {
        return Err(GenerateError::Input(format!(
            "{} holds no screen template, so the screen jar would be built from nothing",
            directory.display()
        )));
    }
    found.sort();
    Ok(found)
}

/// What one `{{…}}` asks for.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Placeholder {
    /// The generated file's own package declaration.
    Package,
    /// An import of one reviewed game class.
    Import(String),
    /// A reference to one reviewed game class.
    Class(String),
    /// A reference to one member of one reviewed game class.
    Member(String, String),
}

impl Placeholder {
    /// Reads what one placeholder's body asks for.
    ///
    /// Every body names a role rather than a game class, so nothing here is
    /// abbreviated and nothing has to be expanded: a role is one word under
    /// every version, and the version's seed list is what says which class and
    /// which member that word means.
    fn read(body: &str) -> Result<Self, String> {
        if body == "package" {
            return Ok(Self::Package);
        }
        if let Some(role) = body.strip_prefix("import ") {
            return Ok(Self::Import(role.trim().to_owned()));
        }
        if body.is_empty() || body.contains(char::is_whitespace) {
            return Err(format!(
                "{{{{{body}}}}} names nothing this generator renders"
            ));
        }
        Ok(match body.split_once('#') {
            Some((role, member)) => Self::Member(role.to_owned(), member.to_owned()),
            None => Self::Class(body.to_owned()),
        })
    }
}

/// One template, read once.
struct Template {
    /// The Java class it renders.
    name: String,
    /// Its text, placeholders and all.
    text: String,
    /// What those placeholders ask for.
    placeholders: Vec<Placeholder>,
}

/// Reads one template and everything its placeholders ask for.
fn template(path: &Path) -> Result<Template, GenerateError> {
    let text = fs::read_to_string(path).map_err(|error| {
        GenerateError::Input(format!("cannot read {}: {error}", path.display()))
    })?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".java.in"))
        .ok_or_else(|| {
            GenerateError::Input(format!(
                "{} is not named after the Java source it renders",
                path.display()
            ))
        })?
        .to_owned();

    let mut placeholders = Vec::new();
    let mut rest = text.as_str();
    while let Some(index) = rest.find("{{") {
        let after = &rest[index + 2..];
        let end = after.find("}}").ok_or_else(|| {
            GenerateError::Input(format!(
                "{} opens a placeholder it never closes",
                path.display()
            ))
        })?;
        placeholders.push(
            Placeholder::read(&after[..end])
                .map_err(|detail| GenerateError::Input(format!("{}: {detail}", path.display())))?,
        );
        rest = &after[end + 2..];
    }
    Ok(Template {
        name,
        text,
        placeholders,
    })
}

/// The linked internal name of every class the templates reference.
///
/// A role is only rendered where the version's seed list binds it, and only
/// where the stub tree carries the class it binds to, because that tree is what
/// the screen jar compiles against: a name outside it would compile against
/// nothing, and the seed list is the file that has to say so.
fn linked_classes(
    referenced: &BTreeSet<String>,
    seeds: &Seeds,
    stubs: &[Stub],
    mapping: &Mapping,
) -> Result<BTreeMap<String, String>, GenerateError> {
    let mut linked = BTreeMap::new();
    let mut simple: BTreeMap<String, String> = BTreeMap::new();
    for role in referenced {
        let class = seeded_class(role, seeds)?;
        if !stubs.iter().any(|stub| stub.class == class) {
            return Err(GenerateError::Unresolved(format!(
                "a screen template names {role}, which the version's stub seeds bind to {}, and \
                 that class is not stubbed, so the screen jar would compile against nothing",
                class.replace('/', ".")
            )));
        }
        let name = mapping.runtime_class(&class).to_owned();
        let last = name.rsplit('/').next().unwrap_or(&name).to_owned();
        if let Some(other) = simple.insert(last.clone(), role.clone())
            && &other != role
        {
            return Err(GenerateError::Unsupported(format!(
                "the roles {other} and {role} both link as {last}, and a source that names one \
                 names the other"
            )));
        }
        linked.insert(role.clone(), name);
    }
    Ok(linked)
}

/// Every seeded member has to name exactly one linked member.
///
/// Checked over the whole seed list rather than over the members templates
/// happen to name, because the alternative hides the defect until someone writes
/// the placeholder. An obfuscated version can link two overloads under different
/// names, and a seed that does not say which one it means is ambiguous the day
/// it is written — not the day it is first referenced. `SavedData#setDirty` was
/// exactly that: two linked names, invisible because no template reached it.
///
/// # Errors
///
/// Returns an error when a seeded member resolves to no linked member or to
/// more than one.
fn every_seeded_member_resolves(
    seeds: &Seeds,
    stubs: &[Stub],
    mapping: &Mapping,
) -> Result<(), GenerateError> {
    for seed in &seeds.stubs {
        for member in &seed.members {
            member_name(&seed.role, member.role(), seeds, stubs, mapping)?;
        }
    }
    Ok(())
}

/// The internal class name one version's seed list binds a role to.
fn seeded_class(role: &str, seeds: &Seeds) -> Result<String, GenerateError> {
    seeds
        .stubs
        .iter()
        .find(|seed| seed.role == role)
        .map(|seed| seed.class.replace('.', "/"))
        .ok_or_else(|| {
            GenerateError::Unresolved(format!(
                "a screen template names the role {role} and the version's stub seeds bind no \
                 class to it, so nothing says which class this version means"
            ))
        })
}

/// Rewrites one template into the source the version compiles.
///
/// Everything outside a placeholder is copied through, so a template's own line
/// structure is the generated source's.
fn expand(
    template: &Template,
    adapter: &Adapter,
    seeds: &Seeds,
    stubs: &[Stub],
    mapping: &Mapping,
    linked: &BTreeMap<String, String>,
    package: Option<&str>,
) -> Result<String, GenerateError> {
    let mut source = String::with_capacity(template.text.len());
    let mut rest = template.text.as_str();
    let mut placeholders = template.placeholders.iter();
    while let Some(index) = rest.find("{{") {
        source.push_str(&rest[..index]);
        let after = &rest[index + 2..];
        let end = after.find("}}").unwrap_or(after.len());
        let placeholder = placeholders.next().ok_or_else(|| {
            GenerateError::Input(format!(
                "{} carries more placeholders than were read from it",
                template.name
            ))
        })?;
        source.push_str(&resolve(
            placeholder,
            adapter,
            seeds,
            stubs,
            mapping,
            linked,
            package,
        )?);
        rest = &after[end + 2..];
    }
    source.push_str(rest);
    Ok(source)
}

/// What one placeholder renders as.
fn resolve(
    placeholder: &Placeholder,
    adapter: &Adapter,
    seeds: &Seeds,
    stubs: &[Stub],
    mapping: &Mapping,
    linked: &BTreeMap<String, String>,
    package: Option<&str>,
) -> Result<String, GenerateError> {
    let name = |class: &String| {
        linked.get(class).cloned().ok_or_else(|| {
            GenerateError::Unresolved(format!(
                "no linked name was resolved for {}",
                class.replace('/', ".")
            ))
        })
    };
    match placeholder {
        // The provenance rides on the package line rather than above it, because
        // a line added at the top of a generated source moves every line below
        // it and the class files would stop being the ones this version built.
        Placeholder::Package => Ok(match package {
            Some(package) => format!(
                "package {package}; // Generated from the pinned Minecraft {} inputs. Do not edit.",
                adapter.minecraft
            ),
            None => format!(
                "// Generated from the pinned Minecraft {} inputs. Do not edit. The game is in the \
                 unnamed package here, so this is too.",
                adapter.minecraft
            ),
        }),
        // A class the version links in the unnamed package is imported by
        // nothing: the language refuses the import and the simple name is the
        // whole name.
        Placeholder::Import(class) => Ok(match name(class)?.rsplit_once('/') {
            Some(_) => format!("import {};", name(class)?.replace('/', ".")),
            None => String::new(),
        }),
        Placeholder::Class(class) => {
            let linked = name(class)?;
            Ok(linked.rsplit('/').next().unwrap_or(&linked).to_owned())
        }
        Placeholder::Member(role, member) => member_name(role, member, seeds, stubs, mapping),
    }
}

/// The linked name of the member one version binds a member role to.
///
/// Two bindings cross here. The seed list says which member of this version's
/// class the template's word means, which is what carries a template across a
/// rename, and the mapping says what that member links as. Every overload the
/// seed list stubbed is asked, and they have to agree. A template names a member
/// and never a descriptor, for the reason `stubs.toml` does not: writing one
/// down is the transcription the generated stubs exist to avoid. Two overloads
/// that link under different names cannot be written as one reference, so that
/// is a refusal rather than a choice made here.
fn member_name(
    role: &str,
    member_role: &str,
    seeds: &Seeds,
    stubs: &[Stub],
    mapping: &Mapping,
) -> Result<String, GenerateError> {
    let seed = seeds
        .stubs
        .iter()
        .find(|seed| seed.role == role)
        .ok_or_else(|| {
            GenerateError::Unresolved(format!(
                "a screen template names {role}#{member_role} and the version's stub seeds bind \
                 no class to that role"
            ))
        })?;
    let class = seed.class.replace('.', "/");
    let member = seed
        .members
        .iter()
        .find(|seeded| seeded.role() == member_role)
        .ok_or_else(|| {
            GenerateError::Unresolved(format!(
                "a screen template names {role}#{member_role} and the version's stub seeds bind \
                 no member of {} to that role",
                seed.class
            ))
        })?
        .member();
    let stub = stubs
        .iter()
        .find(|stub| stub.class == class)
        .ok_or_else(|| {
            GenerateError::Unresolved(format!(
                "a screen template names {role}#{member_role} and {} is not stubbed",
                seed.class
            ))
        })?;
    let mut names = stub
        .members
        .iter()
        .filter(|declared| declared.name == member)
        .map(|declared| {
            mapping
                .linked_member(&class, &declared.name, &declared.descriptor)
                .0
        })
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    match names.as_slice() {
        [] => Err(GenerateError::Unresolved(format!(
            "a screen template names {role}#{member_role}, which this version binds to {}#{member}, \
             and the stub tree does not carry that member",
            seed.class
        ))),
        [one] => Ok(one.clone()),
        several => Err(GenerateError::Unsupported(format!(
            "{}#{member} links under {} names ({}), so no single reference names them all",
            seed.class,
            several.len(),
            several.join(", ")
        ))),
    }
}

/// The constant one screen class is reached by.
///
/// The agent jar loads every screen reflectively and by name, so the name it
/// passes is version data exactly as a game class name is: the screens move to
/// the unnamed package under a version that puts the game there. The constant is
/// the class's own name, which is what survives that move.
fn constant(name: &str) -> String {
    let mut text = String::with_capacity(name.len() + 4);
    for (index, character) in name.char_indices() {
        if character.is_ascii_uppercase() && index != 0 {
            text.push('_');
        }
        text.push(character.to_ascii_uppercase());
    }
    text
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use super::super::inputs::{self, MappingSource, Seed, SeedMember};
    use super::super::render;
    use super::super::stubs::{Member, Stub};
    use super::*;

    /// A mapping that renames a class into the unnamed package and its members
    /// with it, which is the shape an obfuscated version publishes.
    ///
    /// The classes are invented rather than the pinned version's, because a game
    /// name written into a crate is one version's spelling asserted over every
    /// other and `ravel-verify-scaffold` refuses it. What is under test is the
    /// translation, which does not know whose names it is translating.
    const MAPPING: &str = "\
com.example.gui.Panel -> a:
    int width -> j
    void draw() -> b
    void draw(int) -> b
    void resize() -> c
    void resize(int) -> d
com.example.text.Label -> a$b:
    com.example.text.Label of(java.lang.String) -> e
com.example.util.Held -> com.example.util.Held:
    void tick() -> tick
";

    fn proguard() -> Mapping {
        Mapping::read(MappingSource::OfficialProguardMappings, MAPPING)
            .expect("the invented mapping should parse")
    }

    fn method(name: &str, descriptor: &str) -> Member {
        Member {
            name: name.to_owned(),
            descriptor: descriptor.to_owned(),
            flags: 1,
            method: true,
        }
    }

    /// The seed list binding a role to each invented class.
    ///
    /// The roles are the words a template would write, and they are deliberately
    /// not the class names: what is under test is that the binding is what
    /// carries a template, so a test whose role happened to equal the simple
    /// name would pass under a generator that ignored the seed list entirely.
    fn seeds() -> inputs::Seeds {
        inputs::Seeds {
            schema_version: 1,
            minecraft: "0.0.0-invented".to_owned(),
            stubs: vec![
                Seed {
                    role: "Surface".to_owned(),
                    class: "com.example.gui.Panel".to_owned(),
                    members: vec![
                        SeedMember::Same("draw".to_owned()),
                        SeedMember::Bound(inputs::BoundMember {
                            role: "size".to_owned(),
                            member: "width".to_owned(),
                        }),
                    ],
                },
                Seed {
                    role: "Text".to_owned(),
                    class: "com.example.text.Label".to_owned(),
                    members: vec![SeedMember::Same("of".to_owned())],
                },
                Seed {
                    role: "Clock".to_owned(),
                    class: "com.example.util.Held".to_owned(),
                    members: vec![SeedMember::Same("tick".to_owned())],
                },
            ],
        }
    }

    fn stubs() -> Vec<Stub> {
        vec![
            Stub {
                class: "com/example/gui/Panel".to_owned(),
                flags: 1,
                extends: None,
                implements: Vec::new(),
                members: vec![
                    Member {
                        name: "width".to_owned(),
                        descriptor: "I".to_owned(),
                        flags: 1,
                        method: false,
                    },
                    method("draw", "()V"),
                    method("draw", "(I)V"),
                    method("resize", "()V"),
                    method("resize", "(I)V"),
                ],
            },
            Stub {
                class: "com/example/text/Label".to_owned(),
                flags: 1,
                extends: None,
                implements: Vec::new(),
                members: vec![method("of", "(Ljava/lang/String;)Lcom/example/text/Label;")],
            },
            Stub {
                class: "com/example/util/Held".to_owned(),
                flags: 1,
                extends: None,
                implements: Vec::new(),
                members: vec![method("tick", "()V")],
            },
        ]
    }

    #[test]
    fn an_overload_set_that_links_under_one_name_renders_that_name() {
        let name = member_name("Surface", "draw", &seeds(), &stubs(), &proguard())
            .expect("both overloads link under one name");

        assert_eq!(name, "b");
    }

    #[test]
    fn a_member_the_seeds_do_not_carry_is_refused_rather_than_passed_through() {
        let error = member_name("Surface", "paint", &seeds(), &stubs(), &proguard())
            .expect_err("a member no seed asked for is stubbed by nothing");

        assert!(matches!(error, GenerateError::Unresolved(_)), "{error}");
    }

    #[test]
    fn a_class_the_mapping_leaves_in_a_package_is_still_imported() {
        let referenced = ["Clock".to_owned()].into_iter().collect();
        let linked =
            linked_classes(&referenced, &seeds(), &stubs(), &proguard()).expect("resolvable");

        assert_eq!(linked["Clock"], "com/example/util/Held");
    }

    #[test]
    fn a_class_the_stub_seeds_do_not_carry_is_refused() {
        let referenced = ["Unseeded".to_owned()].into_iter().collect();

        let error = linked_classes(&referenced, &seeds(), &stubs(), &proguard())
            .expect_err("a template may only name a role the seeds bind");

        assert!(matches!(error, GenerateError::Unresolved(_)), "{error}");
    }

    fn repository_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("the crate lives under crates/<name>")
            .to_owned()
    }

    /// A published mapping that renames the pinned version's whole stub set.
    ///
    /// It is derived from that set rather than written, for two reasons. A game
    /// name written into a crate is refused by `ravel-verify-scaffold`, and a
    /// mapping written by hand would name the classes its author remembered
    /// rather than the ones the templates use. Every class goes to the unnamed
    /// package, which is where an obfuscated version puts most of the game, and
    /// every member is renamed with it. Overloads share one linked name, as a
    /// published mapping gives them.
    fn obfuscating(stubs: &[Stub]) -> Mapping {
        let mut text = String::new();
        for (index, stub) in stubs.iter().enumerate() {
            let _ = writeln!(text, "{} -> o{index}:", stub.class.replace('/', "."));
            let mut renamed: BTreeMap<&str, String> = BTreeMap::new();
            for member in &stub.members {
                if member.name == "<init>" {
                    continue;
                }
                let next = format!("m{index}_{}", renamed.len());
                let linked = renamed.entry(member.name.as_str()).or_insert(next).clone();
                if member.method {
                    let (parameters, returns) =
                        render::signature(&member.descriptor).expect("a stubbed descriptor reads");
                    let _ = writeln!(
                        text,
                        "    {returns} {}({}) -> {linked}",
                        member.name,
                        parameters.join(",")
                    );
                } else {
                    let (kind, _) =
                        render::java_type(&member.descriptor).expect("a stubbed descriptor reads");
                    let _ = writeln!(text, "    {kind} {} -> {linked}", member.name);
                }
            }
        }
        Mapping::read(MappingSource::OfficialProguardMappings, &text)
            .expect("a mapping derived from the stub set should parse")
    }

    /// The tracked templates, rendered against a mapping that renames everything.
    ///
    /// This is the half every earlier test of this generator left unproven: the
    /// identity mapping renders a template into the source it already was, so a
    /// placeholder nobody wrote and a name nobody translated look the same. Here
    /// the linked names share nothing with the reviewed ones, so a reviewed class
    /// name surviving into the generated source is a reference the templates
    /// spell rather than name.
    #[test]
    fn the_tracked_templates_render_into_a_name_space_that_shares_nothing_with_the_reviewed_one() {
        let root = repository_root();
        let pinned = inputs::pinned(&root, None).expect("the tracked version data should parse");
        let tracked = std::fs::read_to_string(pinned.directory.join("stubs.lock"))
            .expect("the tracked stub lock should be readable");
        let stubs = super::super::stubs::from_lock(&tracked, &pinned.adapter, &pinned.seeds)
            .expect("the tracked lock should resolve");
        let mapping = obfuscating(&stubs);

        let screens = render(
            &root.join(super::super::SCREEN_TEMPLATES),
            &pinned.adapter,
            &pinned.seeds,
            &stubs,
            &mapping,
        )
        .expect("the tracked templates should render against an obfuscated mapping");

        let reviewed = stubs
            .iter()
            .map(|stub| {
                stub.class
                    .rsplit('/')
                    .next()
                    .unwrap_or(&stub.class)
                    .to_owned()
            })
            .collect::<BTreeSet<_>>();

        for (relative, source) in &screens.java {
            assert!(
                !relative.contains('/'),
                "a mapping that puts the game in the unnamed package puts the screens there \
                 too, and {relative} is in a package"
            );
            assert!(
                !source.contains("{{"),
                "every placeholder is rendered: {relative}"
            );
            for (number, line) in code_lines(source) {
                for name in &reviewed {
                    assert!(
                        !names(&line).contains(name),
                        "{relative}:{number} names the reviewed class {name}, which this version \
                         links nothing under: {line}"
                    );
                }
            }
        }

        for (_, binary) in &screens.names {
            assert!(
                !binary.contains('.'),
                "the agent jar is told where the screens went, and they went to the unnamed \
                 package: {binary}"
            );
        }
    }

    /// One rendered source's lines, with its comments and strings taken out.
    ///
    /// A class name in a comment is the prose it is, and one in a string literal
    /// is a name the source resolves at runtime rather than links.
    fn code_lines(source: &str) -> Vec<(usize, String)> {
        let mut lines = Vec::new();
        let mut block = false;
        for (index, line) in source.lines().enumerate() {
            let mut code = String::new();
            let mut rest = line;
            loop {
                if block {
                    match rest.split_once("*/") {
                        Some((_, after)) => {
                            block = false;
                            rest = after;
                        }
                        None => break,
                    }
                } else if let Some((before, after)) = rest.split_once("/*") {
                    code.push_str(before);
                    block = true;
                    rest = after;
                } else if let Some((before, _)) = rest.split_once("//") {
                    code.push_str(before);
                    break;
                } else {
                    code.push_str(rest);
                    break;
                }
            }
            let stripped = code.split('"').step_by(2).collect::<Vec<_>>().join(" ");
            lines.push((index + 1, stripped));
        }
        lines
    }

    /// Every identifier one line carries.
    fn names(line: &str) -> BTreeSet<String> {
        line.split(|character: char| !(character.is_alphanumeric() || character == '_'))
            .filter(|word| !word.is_empty())
            .map(str::to_owned)
            .collect()
    }

    /// A placeholder body is a role and is read as the one word it is. Nothing
    /// binds it here, because the version's seed list is what binds it, and a
    /// template that spelled a package would be naming a class rather than a
    /// role.
    #[test]
    fn a_placeholder_body_reads_as_the_role_it_names() {
        assert_eq!(
            Placeholder::read("Surface#draw").expect("a placeholder body"),
            Placeholder::Member("Surface".to_owned(), "draw".to_owned())
        );
        assert_eq!(
            Placeholder::read("import Surface").expect("a placeholder body"),
            Placeholder::Import("Surface".to_owned())
        );
    }

    /// The property the whole indirection exists for: what a template renders is
    /// decided by the binding and not by the word. Both halves are checked,
    /// because a generator that resolved the role and then ignored the member
    /// binding would pass a class-only test.
    #[test]
    fn a_role_renders_the_class_and_member_the_seed_list_binds_it_to() {
        let referenced = ["Surface".to_owned()].into_iter().collect();
        let linked =
            linked_classes(&referenced, &seeds(), &stubs(), &proguard()).expect("resolvable");
        assert_eq!(linked["Surface"], "a");

        let name = member_name("Surface", "size", &seeds(), &stubs(), &proguard())
            .expect("the seed list binds the member role size to width");

        assert_eq!(name, "j");
    }

    /// A member role the seed list does not bind is refused rather than tried as
    /// a member name, which is what keeps a template from quietly working on the
    /// version it was authored against and failing on every other.
    /// A seed list binding one member role of the panel to a named overload.
    ///
    /// `resize` is the pair that matters: the invented mapping links `resize()`
    /// as `c` and `resize(int)` as `d`, which is the shape an obfuscated version
    /// produces and the whole reason a count exists.
    fn seeds_binding(member_role: &str, parameters: Option<usize>) -> inputs::Seeds {
        let mut seeds = seeds();
        seeds.stubs[0].members = vec![SeedMember::Bound(inputs::BoundMember {
            role: member_role.to_owned(),
            member: "resize".to_owned(),
            parameters,
        })];
        seeds
    }

    /// The count names which overload the template meant, and the two overloads
    /// link under different names, so getting this wrong is silently calling the
    /// other method rather than failing.
    #[test]
    fn a_parameter_count_picks_the_overload_it_names() {
        let none = seeds_binding("scale", Some(0));
        assert_eq!(
            member_name("Surface", "scale", &none, &stubs(), &proguard())
                .expect("resize() is the zero-argument overload"),
            "c"
        );

        let one = seeds_binding("scale", Some(1));
        assert_eq!(
            member_name("Surface", "scale", &one, &stubs(), &proguard())
                .expect("resize(int) is the one-argument overload"),
            "d"
        );
    }

    /// A count that separates nothing is a binding that names no member, and is
    /// refused rather than falling back to the whole overload set.
    #[test]
    fn a_parameter_count_matching_no_overload_is_refused() {
        let seeds = seeds_binding("scale", Some(2));

        let error = member_name("Surface", "scale", &seeds, &stubs(), &proguard())
            .expect_err("the panel declares no two-argument resize");

        assert!(matches!(error, GenerateError::Unresolved(_)), "{error}");
    }

    /// Without a count the two overloads are two answers, and the generator says
    /// so instead of picking. This is the refusal the count exists to let a seed
    /// list resolve deliberately.
    #[test]
    fn an_overload_set_linking_under_two_names_without_a_count_is_refused() {
        let seeds = seeds_binding("scale", None);

        let error = member_name("Surface", "scale", &seeds, &stubs(), &proguard())
            .expect_err("resize links under two names here");

        assert!(matches!(error, GenerateError::Unsupported(_)), "{error}");
    }

    /// The sweep that would have caught `SavedData#setDirty` before a template
    /// reached it: every seeded member is resolved, not only the referenced ones.
    #[test]
    fn a_seeded_member_no_template_names_is_still_required_to_resolve() {
        let ambiguous = seeds_binding("scale", None);

        let error = every_seeded_member_resolves(&ambiguous, &stubs(), &proguard())
            .expect_err("an ambiguous seed is a defect whether or not a template names it");

        assert!(matches!(error, GenerateError::Unsupported(_)), "{error}");
        every_seeded_member_resolves(&seeds_binding("scale", Some(0)), &stubs(), &proguard())
            .expect("a bound overload resolves");
    }

    #[test]
    fn a_member_role_the_seed_list_does_not_bind_is_refused() {
        let error = member_name("Surface", "width", &seeds(), &stubs(), &proguard())
            .expect_err("width is a member name here, and the role bound to it is size");

        assert!(matches!(error, GenerateError::Unresolved(_)), "{error}");
    }

    #[test]
    fn a_constant_is_the_screen_class_name() {
        assert_eq!(constant("RavelSettingsScreen"), "RAVEL_SETTINGS_SCREEN");
        assert_eq!(constant("Wire"), "WIRE");
    }
}
