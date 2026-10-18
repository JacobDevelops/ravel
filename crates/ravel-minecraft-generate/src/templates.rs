//! Renders screen and harness templates using a version's linked class and member roles.
//!
//! Non-placeholder text is copied byte for byte; overrides handle API changes, not renames.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::classfile;
use super::inputs::Adapter;
use super::mapping::Mapping;
use super::roles::Roles;
use super::stubs::Stub;
use super::{GenerateError, SCREEN_PACKAGE};

/// The suffix a template carries, over the name of the source it renders.
const TEMPLATE_SUFFIX: &str = ".in";

/// Controls source naming, stub requirements, and version-only additions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Purpose {
    /// The screen jar's sources: Ravel's own classes, named by their templates,
    /// compiled against the stub tree, and the same set under every version
    /// because the agent jar opens the same constants under all of them.
    Screen,
    /// The harness's stand-in game: one game class per template, named and
    /// placed by the role the template is named after. Nothing stubs them —
    /// they are what the harness compiles against — and a version whose game
    /// carries a class another version's does not renders one template more,
    /// because that difference is the game's rather than Ravel's.
    StandIn,
    /// The harness's own drivers: Ravel's classes in the name space a launch
    /// runs them from, which is the unnamed package under every version.
    Driver,
}

impl Purpose {
    /// Whether a role a template references has to be carried by the stub tree.
    fn stubbed(self) -> bool {
        matches!(self, Self::Screen)
    }

    /// Why this tree may not carry a template no shared one is replaced by,
    /// where it may not.
    ///
    /// Only the stand-in game may gain one, and the two refusals are worth
    /// saying apart: a screen would be one the agent jar cannot open under the
    /// others, while a driver is compiled by the sweep that finds it and
    /// reached through no role, so the check that retires an outlived stand-in
    /// has nothing to ask about a driver.
    fn no_addition_reason(self) -> Option<&'static str> {
        match self {
            Self::StandIn => None,
            Self::Screen => Some(
                "a screen only one version draws is one the agent jar cannot open under the \
                 others",
            ),
            Self::Driver => Some(
                "a driver is reached by no role, so nothing could tell one that outlived what \
                 it shadowed from one still in service",
            ),
        }
    }
}

/// The shared resolution context for one version's placeholders.
struct Version<'a> {
    /// What the tree being rendered is for.
    purpose: Purpose,
    /// The version's own pins, for the provenance each source carries.
    adapter: &'a Adapter,
    /// What each role is bound to.
    roles: &'a Roles,
    /// The stub tree a seeded member is resolved against.
    stubs: &'a [Stub],
    /// What turns a reviewed name into the one this version links.
    mapping: &'a Mapping,
    /// The linked internal name of every role the templates reference.
    linked: &'a BTreeMap<String, String>,
    /// Whether a game class this tree names is spelled by the name the
    /// stand-in beside it was written under rather than by the one it links.
    aliased: bool,
    /// The package Ravel's own screens are drawn in, where this version leaves
    /// one. A driver names them by the whole name rather than importing it,
    /// because the name a driver spells has to be legal from the unnamed
    /// package under both cases.
    screens: Option<&'a str>,
}

/// One template tree, and where this version keeps the sources it cannot share.
pub(super) struct Tree<'a> {
    /// Repository boundary retained only for redacted, unambiguous diagnostics.
    pub(super) root: &'a Path,
    /// The shared templates every version renders unless it replaces one.
    pub(super) shared: &'a Path,
    /// This version's own, which live under its version data rather than beside
    /// the shared tree: a source only one version renders is that version's,
    /// and everything a version owns is kept in one place.
    pub(super) overrides: PathBuf,
    /// What the tree renders.
    pub(super) purpose: Purpose,
    /// Whether this tree is compiled beside the stand-in game rather than
    /// against the stub tree, which is what says whether a game class it names
    /// is spelled the way the stand-ins were written or the way they link.
    pub(super) stand_ins: bool,
}

/// One namespace shared by all trees compiled together.
#[derive(Clone, Copy, Debug)]
pub(super) struct NameSpace {
    /// Whether Ravel's own classes are written in the unnamed package.
    unnamed: bool,
    /// Whether the game spans the unnamed package and named ones at once, which
    /// is what makes a stand-in's written name differ from the name it links.
    split: bool,
}

/// Everything one version's sources from one template tree come to.
#[derive(Default)]
pub(super) struct Sources {
    /// Each source's path below the generated tree, with its Java.
    pub(super) java: Vec<(String, String)>,
    /// The templates a build has to watch.
    pub(super) templates: Vec<PathBuf>,
    /// Each screen class's constant name and the binary name it was emitted as.
    pub(super) names: Vec<(String, String)>,
    /// Each source's name, and whether this version's own tree is what rendered it.
    pub(super) origins: Vec<(String, bool)>,
    /// Every role a placeholder in this tree named.
    ///
    /// A template's own name is not among them: what this answers is what the
    /// sources *reach for*, which is how a version-only template is told from
    /// one nothing renders any more.
    referenced: BTreeSet<String>,
    /// Each template this version added rather than replaced, with its role.
    added: Vec<(String, PathBuf)>,
    /// Each stand-in whose written name is not its linked one, as the name it
    /// is written under and the name it stands in for.
    pub(super) aliases: Vec<(String, String)>,
}

/// Renders every tracked template into the name space the version links.
///
/// # Errors
///
/// Returns an error when a template is unreadable, when it names a role no
/// admission list binds, when it names a member that role's class does not
/// declare, or when the names it asks for cannot be written down together.
pub(super) fn render(
    tree: &Tree<'_>,
    adapter: &Adapter,
    roles: &Roles,
    stubs: &[Stub],
    mapping: &Mapping,
    name_space: NameSpace,
) -> Result<Sources, GenerateError> {
    let purpose = tree.purpose;
    let Templates {
        rendered: templates,
        overridden,
        added: version_only,
    } = templates(tree)?;
    let mut read = Vec::with_capacity(templates.len());
    for path in &templates {
        read.push(template(tree.root, path)?);
    }

    let TemplateRoles {
        referenced,
        declared,
        added,
    } = render_roles(&read, purpose, &version_only, roles)?;

    let all = referenced.union(&declared).cloned().collect();
    let linked = linked_classes(&all, roles, stubs, mapping, purpose)?;
    let NameSpace { unnamed, split } = name_space;
    every_name_is_nameable(&read, purpose, &linked, unnamed)?;
    let screens = (!unnamed).then_some(SCREEN_PACKAGE);
    let owned = match purpose {
        Purpose::Screen => screens,
        Purpose::StandIn | Purpose::Driver => None,
    };

    let version = Version {
        purpose,
        adapter,
        roles,
        stubs,
        mapping,
        linked: &linked,
        aliased: split && tree.stand_ins,
        screens,
    };
    let mut java = Vec::with_capacity(read.len());
    let mut names = Vec::with_capacity(read.len());
    let mut aliases = Vec::new();
    for template in &read {
        let (package, simple, relative) = render_location(template, &version, owned, &mut aliases)?;
        let source = expand(template, &version, package.as_deref())?;
        let binary = match package {
            Some(package) => format!("{package}.{simple}"),
            None => simple,
        };
        java.push((relative, source));
        names.push((constant(&template.name), binary));
    }
    let origins = read
        .iter()
        .map(|template| (template.name.clone(), overridden.contains(&template.path)))
        .collect();
    Ok(Sources {
        java,
        templates,
        names,
        origins,
        referenced,
        added,
        aliases,
    })
}

fn render_location(
    template: &Template<'_>,
    version: &Version<'_>,
    owned: Option<&str>,
    aliases: &mut Vec<(String, String)>,
) -> Result<(Option<String>, String, String), GenerateError> {
    // A stand-in is placed and named by the class it declares; Ravel's own
    // sources are placed and named by the template that renders them.
    if version.purpose == Purpose::StandIn {
        let linked_name = version
            .linked
            .get(&template.name)
            .ok_or_else(|| unresolved_role(template, &template.name))?;
        let declared = classfile::alias(linked_name, version.aliased);
        if &declared != linked_name {
            aliases.push((declared.clone(), linked_name.clone()));
        }
        let (package, simple) = classfile::split(&declared);
        Ok((package, simple.to_owned(), format!("{declared}.java")))
    } else {
        let relative = owned.map_or_else(
            || format!("{}.java", template.name),
            |package| format!("{}/{}.java", package.replace('.', "/"), template.name),
        );
        Ok((owned.map(str::to_owned), template.name.clone(), relative))
    }
}

struct TemplateRoles {
    referenced: BTreeSet<String>,
    declared: BTreeSet<String>,
    added: Vec<(String, PathBuf)>,
}

fn render_roles(
    read: &[Template<'_>],
    purpose: Purpose,
    version_only: &BTreeSet<PathBuf>,
    roles: &Roles,
) -> Result<TemplateRoles, GenerateError> {
    let mut referenced = BTreeSet::new();
    let mut declared = BTreeSet::new();
    let mut added = Vec::with_capacity(version_only.len());
    for template in read {
        let mut names = Vec::with_capacity(template.placeholders.len() + 1);
        if purpose == Purpose::StandIn {
            names.push(template.name.clone());
            declared.insert(template.name.clone());
            if version_only.contains(&template.path) {
                added.push((template.name.clone(), template.path.clone()));
            }
        }
        for placeholder in &template.placeholders {
            if let Placeholder::Class(role) | Placeholder::Import(role) = placeholder {
                // A stand-in's own declaration does not count as reaching itself.
                if role != &template.name || purpose != Purpose::StandIn {
                    referenced.insert(role.clone());
                }
                names.push(role.clone());
            }
        }
        for role in names {
            roles
                .class(&role)
                .map_err(|error| in_template(template, error))?;
        }
    }
    Ok(TemplateRoles {
        referenced,
        declared,
        added,
    })
}

/// Rejects version-only additions that no rendered tree references.
///
/// # Errors
///
/// Returns an error naming an unreachable addition and its role.
pub(super) fn every_added_template_is_reached(
    root: &Path,
    rendered: &[&Sources],
) -> Result<(), GenerateError> {
    let mut reached = BTreeSet::new();
    for sources in rendered {
        reached.extend(sources.referenced.iter().cloned());
    }
    for sources in rendered {
        for (role, path) in &sources.added {
            if !reached.contains(role) {
                return Err(GenerateError::Input(format!(
                    "{} replaces no shared template and no source this version renders names the \
                     role {role}, so it is an override that outlived the template it shadowed",
                    diagnostic_path(root, path)
                )));
            }
        }
    }
    Ok(())
}

/// The refusal a template naming a role nothing resolved earns, with the file.
fn unresolved_role(template: &Template<'_>, role: &str) -> GenerateError {
    in_template(
        template,
        GenerateError::Unresolved(format!("no linked name was resolved for the role {role}")),
    )
}

/// Rejects unnamed-package references from packaged sources.
fn every_name_is_nameable(
    read: &[Template<'_>],
    purpose: Purpose,
    linked: &BTreeMap<String, String>,
    unnamed: bool,
) -> Result<(), GenerateError> {
    if unnamed {
        return Ok(());
    }
    for template in read {
        let declared = (purpose == Purpose::StandIn).then_some(template.name.as_str());
        let referenced = template
            .placeholders
            .iter()
            .filter_map(|placeholder| match placeholder {
                Placeholder::Class(role) | Placeholder::Import(role) => Some(role.as_str()),
                Placeholder::Package | Placeholder::Member(..) | Placeholder::Ravel(_) => None,
            });
        for role in declared.into_iter().chain(referenced) {
            let Some(outside) = linked
                .get(role)
                .filter(|name| classfile::split(name).0.is_none())
            else {
                continue;
            };
            return Err(in_template(
                template,
                GenerateError::Unsupported(format!(
                    "names {outside}, which this version links outside every package, while its \
                     stand-in game is in packages and Ravel's own sources here are too"
                )),
            ));
        }
    }
    Ok(())
}

/// Adds the template path to a version-data resolution error.
fn in_template(template: &Template<'_>, error: GenerateError) -> GenerateError {
    let named = |detail: String| {
        format!(
            "{}: {detail}",
            diagnostic_path(template.root, &template.path)
        )
    };
    match error {
        GenerateError::Input(detail) => GenerateError::Input(named(detail)),
        GenerateError::Drift(detail) => GenerateError::Drift(named(detail)),
        GenerateError::Unresolved(detail) => GenerateError::Unresolved(named(detail)),
        GenerateError::Extraction(detail) => GenerateError::Extraction(named(detail)),
        GenerateError::Unsupported(detail) => GenerateError::Unsupported(named(detail)),
    }
}

/// Reads shared templates in order, applying same-named version overrides.
///
/// Only stand-in trees permit additions; bindings handle class and member renames.
fn templates(tree: &Tree<'_>) -> Result<Templates, GenerateError> {
    let mut found = in_directory(tree.root, tree.shared)?;
    let mut overridden = BTreeSet::new();
    let mut added = BTreeSet::new();
    let overrides = &tree.overrides;
    if overrides.is_dir() {
        // A version's override directory holds templates and nothing else. A
        // directory inside it is refused rather than skipped, because
        // `in_directory` reads one level and whatever sat below would render
        // into nothing at all while saying nothing. The shared directory is
        // where these directories legitimately live, which is why this is here
        // rather than in the read.
        for entry in fs::read_dir(overrides).map_err(|error| {
            GenerateError::Input(format!(
                "cannot read the screen templates at {}: {error}",
                diagnostic_path(tree.root, overrides)
            ))
        })? {
            let path = entry
                .map_err(|error| {
                    GenerateError::Input(format!("cannot read a screen template: {error}"))
                })?
                .path();
            if path.is_dir() {
                return Err(GenerateError::Input(format!(
                    "{} is a directory, and a version's screen templates are read from one \
                     level only, so anything inside it would be silently left out",
                    diagnostic_path(tree.root, &path)
                )));
            }
        }
        for path in in_directory(tree.root, overrides)? {
            let name = path.file_name().unwrap_or_default().to_owned();
            let replaced = found
                .iter()
                .position(|shared| shared.file_name().unwrap_or_default() == name);
            overridden.insert(path.clone());
            match replaced {
                Some(index) => found[index] = path,
                None => match tree.purpose.no_addition_reason() {
                    None => {
                        added.insert(path.clone());
                        found.push(path);
                    }
                    Some(reason) => {
                        return Err(GenerateError::Input(format!(
                            "{} overrides no shared template, and {reason}",
                            diagnostic_path(tree.root, &path)
                        )));
                    }
                },
            }
        }
    }
    if found.is_empty() {
        return Err(GenerateError::Input(format!(
            "{} holds no template this version renders, so the tree it feeds would be built \
             from nothing",
            diagnostic_path(tree.root, tree.shared)
        )));
    }
    found.sort();
    Ok(Templates {
        rendered: found,
        overridden,
        added,
    })
}

/// Every template one version renders, and which tree each was read from.
#[derive(Debug)]
struct Templates {
    /// What this version renders, in a stable order.
    rendered: Vec<PathBuf>,
    /// The ones read from this version's own directory rather than the shared tree.
    overridden: BTreeSet<PathBuf>,
    /// The ones it added rather than replaced.
    added: BTreeSet<PathBuf>,
}

/// Every template directly inside one directory, in a stable order.
fn in_directory(root: &Path, directory: &Path) -> Result<Vec<PathBuf>, GenerateError> {
    let mut found = Vec::new();
    let entries = fs::read_dir(directory).map_err(|error| {
        GenerateError::Input(format!(
            "cannot read the screen templates at {}: {error}",
            diagnostic_path(root, directory)
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
    /// The whole name of one class Ravel itself draws, which moves with the
    /// game: a version that leaves the game in packages leaves Ravel's screens
    /// in theirs, and one that does not draws them in the unnamed package too.
    Ravel(String),
}

impl Placeholder {
    /// Reads what one placeholder's body asks for.
    ///
    /// Every body names a role rather than a game class, so nothing here is
    /// abbreviated and nothing has to be expanded: a role is one word under
    /// every version, the version's three admission lists say which class that
    /// word means, and its seed list says which member.
    fn read(body: &str) -> Result<Self, String> {
        if body == "package" {
            return Ok(Self::Package);
        }
        if let Some(role) = body.strip_prefix("import ") {
            return Ok(Self::Import(role.trim().to_owned()));
        }
        if let Some(name) = body.strip_prefix("ravel ") {
            let name = name.trim();
            if name.is_empty() || name.contains(char::is_whitespace) {
                return Err(format!("{{{{{body}}}}} names no class Ravel draws"));
            }
            return Ok(Self::Ravel(name.to_owned()));
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
struct Template<'a> {
    /// Root borrowed from the tree, never rendered into diagnostics.
    root: &'a Path,
    /// Where it was read from, which is what a refusal names.
    path: PathBuf,
    /// The role or the Java class it renders.
    name: String,
    /// Its text, placeholders and all.
    text: String,
    /// What those placeholders ask for.
    placeholders: Vec<Placeholder>,
}

/// Derives the namespace from game declarations or a standalone screen's references.
///
/// # Errors
///
/// Returns an error for unreadable or misnamed templates, or unbound roles.
pub(super) fn name_space(
    tree: &Tree<'_>,
    roles: &Roles,
    mapping: &Mapping,
) -> Result<NameSpace, GenerateError> {
    let mut linked = Vec::new();
    for path in templates(tree)?.rendered {
        let template = template(tree.root, &path)?;
        let declaration = (tree.purpose == Purpose::StandIn).then_some(template.name.as_str());
        let references = template
            .placeholders
            .iter()
            .filter(|_| tree.purpose != Purpose::StandIn)
            .filter_map(|placeholder| match placeholder {
                Placeholder::Class(role)
                | Placeholder::Import(role)
                | Placeholder::Member(role, _) => Some(role.as_str()),
                Placeholder::Package | Placeholder::Ravel(_) => None,
            });
        for role in declaration.into_iter().chain(references) {
            let class = roles
                .class(role)
                .map_err(|error| in_template(&template, error))?;
            linked.push(mapping.runtime_class(&class).to_owned());
        }
    }
    let names = || linked.iter().map(String::as_str);
    Ok(NameSpace {
        unnamed: names().any(|name| classfile::split(name).0.is_none()),
        split: classfile::name_space_is_split(names()),
    })
}

/// Reads one template and everything its placeholders ask for.
fn template<'a>(root: &'a Path, path: &Path) -> Result<Template<'a>, GenerateError> {
    let text = fs::read_to_string(path).map_err(|error| {
        GenerateError::Input(format!(
            "cannot read {}: {error}",
            diagnostic_path(root, path)
        ))
    })?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".java.in"))
        .ok_or_else(|| {
            GenerateError::Input(format!(
                "{} is not named after the Java source it renders",
                diagnostic_path(root, path)
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
                diagnostic_path(root, path)
            ))
        })?;
        placeholders.push(Placeholder::read(&after[..end]).map_err(|detail| {
            GenerateError::Input(format!("{}: {detail}", diagnostic_path(root, path)))
        })?);
        rest = &after[end + 2..];
    }
    Ok(Template {
        root,
        path: path.to_owned(),
        name,
        text,
        placeholders,
    })
}

/// Keeps tracked shared and override paths distinct without exposing the checkout.
fn diagnostic_path<'a>(root: &Path, path: &'a Path) -> std::borrow::Cow<'a, str> {
    path.strip_prefix(root)
        .unwrap_or_else(|_| {
            path.file_name()
                .map_or_else(|| Path::new("input"), Path::new)
        })
        .to_string_lossy()
}

/// Resolves class roles, requiring screen references to exist in the stub tree.
fn linked_classes(
    referenced: &BTreeSet<String>,
    roles: &Roles,
    stubs: &[Stub],
    mapping: &Mapping,
    purpose: Purpose,
) -> Result<BTreeMap<String, String>, GenerateError> {
    let mut linked = BTreeMap::new();
    let mut simple: BTreeMap<String, (String, String)> = BTreeMap::new();
    for role in referenced {
        let class = roles.class(role)?;
        if purpose.stubbed() && !stubs.iter().any(|stub| stub.class == class) {
            return Err(GenerateError::Unresolved(format!(
                "a screen template names {role}, which this version binds to {}, and that class \
                 is not stubbed, so the screen jar would compile against nothing",
                classfile::source_name(&class)
            )));
        }
        let name = mapping.runtime_class(&class).to_owned();
        let last = classfile::split(&name).1.to_owned();
        // Two roles for one class are two words for one thing, which is what a
        // class the reflective surface and the stub seeds both name is. Two
        // roles for two classes that link as one name are the collision: a
        // source naming either of them names whichever the compiler resolves.
        if let Some((other, held)) = simple.insert(last.clone(), (role.clone(), name.clone()))
            && held != name
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

/// Checks every seeded member, including members no template currently references.
///
/// # Errors
///
/// Returns an error when a seeded member has zero or multiple linked names.
pub(super) fn every_seeded_member_resolves(
    roles: &Roles,
    stubs: &[Stub],
    mapping: &Mapping,
) -> Result<(), GenerateError> {
    for (role, members) in roles.seeded() {
        for member in members {
            member_name(role, member.role(), roles, stubs, mapping)?;
        }
    }
    Ok(())
}

/// Expands placeholders without changing surrounding source text.
fn expand(
    template: &Template<'_>,
    version: &Version<'_>,
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
        source.push_str(
            &resolve(placeholder, version, package)
                .map_err(|error| in_template(template, error))?,
        );
        rest = &after[end + 2..];
    }
    source.push_str(rest);
    Ok(source)
}

/// What one placeholder renders as.
fn resolve(
    placeholder: &Placeholder,
    version: &Version<'_>,
    package: Option<&str>,
) -> Result<String, GenerateError> {
    let name = |role: &String| {
        version.linked.get(role).cloned().ok_or_else(|| {
            GenerateError::Unresolved(format!("no linked name was resolved for the role {role}"))
        })
    };
    match placeholder {
        // The provenance rides on the package line rather than above it, because
        // a line added at the top of a generated source moves every line below
        // it and the class files would stop being the ones this version built.
        Placeholder::Package => Ok(package.map_or_else(
            || format!(
                "// Generated from the pinned Minecraft {} inputs. Do not edit. {}",
                version.adapter.minecraft,
                match version.purpose {
                    Purpose::Driver => "A command line names this class, so it has no package.",
                    Purpose::Screen | Purpose::StandIn => {
                        "The game is in the unnamed package here, so this is too."
                    }
                }
            ),
            |package| format!(
                "package {package}; // Generated from the pinned Minecraft {} inputs. Do not edit.",
                version.adapter.minecraft
            ),
        )),
        // A class the version links in the unnamed package is imported by
        // nothing: the language refuses the import and the simple name is the
        // whole name.
        Placeholder::Import(class) => {
            let linked = classfile::alias(&name(class)?, version.aliased);
            Ok(match classfile::split(&linked).0 {
                Some(_) => format!("import {};", classfile::source_name(&linked)),
                None => String::new(),
            })
        }
        Placeholder::Class(class) => {
            let linked = name(class)?;
            Ok(classfile::split(&linked).1.to_owned())
        }
        Placeholder::Member(role, member) => {
            member_name(role, member, version.roles, version.stubs, version.mapping)
        }
        Placeholder::Ravel(name) => Ok(version
            .screens
            .map_or_else(|| name.clone(), |package| format!("{package}.{name}"))),
    }
}

/// Reconciles seeded and runtime member roles, rejecting ambiguous linked names.
fn member_name(
    role: &str,
    member_role: &str,
    roles: &Roles,
    stubs: &[Stub],
    mapping: &Mapping,
) -> Result<String, GenerateError> {
    let class = roles.class(role)?;
    let reviewed = classfile::source_name(&class);
    let seeded = seeded_member_name(&class, role, member_role, roles, stubs, mapping)?;
    let declared = roles.declared_member(role, member_role)?.map(|declared| {
        mapping
            .linked_member(&class, &declared.member, &declared.descriptor)
            .0
    });
    match (seeded, declared) {
        (Some(seeded), Some(declared)) if seeded != declared => {
            Err(GenerateError::Unsupported(format!(
                "{reviewed}#{member_role} links as {seeded} through the stub seeds and as \
                 {declared} through this version's runtime lists, and one member role names one \
                 member"
            )))
        }
        (Some(name), _) | (None, Some(name)) => Ok(name),
        (None, None) => Err(GenerateError::Unresolved(roles.bound_by(role).map_or_else(
            || format!(
                "a template names {role}#{member_role} and none of this version's stubs.toml, \
                 reflection.toml or hooks.toml binds a class to that role"
            ),
            |list| format!(
                "a template names {role}#{member_role}, and this version's {list} binds a class \
                 to that role and no member of it to that member role"
            ),
        ))),
    }
}

/// Resolves a seeded member role, leaving unseeded roles to the runtime declarations.
fn seeded_member_name(
    class: &str,
    role: &str,
    member_role: &str,
    roles: &Roles,
    stubs: &[Stub],
    mapping: &Mapping,
) -> Result<Option<String>, GenerateError> {
    let Some(bound) = roles.seeded_members(role) else {
        return Ok(None);
    };
    let Some(seeded) = bound.iter().find(|seeded| seeded.role() == member_role) else {
        return Ok(None);
    };
    let reviewed = classfile::source_name(class);
    let member = seeded.member();
    let stub = stubs
        .iter()
        .find(|stub| stub.class == class)
        .ok_or_else(|| {
            GenerateError::Unresolved(format!(
                "a template names {role}#{member_role} and {reviewed} is not stubbed"
            ))
        })?;
    // Every overload is stubbed, because the seed list names a member and never
    // a descriptor. Where a version links them under one name that is the whole
    // answer; where it does not, the binding's parameter count is what separates
    // them, and a set it fails to separate is refused below rather than chosen
    // between.
    let mut candidates = Vec::new();
    for declared in stub.members.iter().filter(|held| held.name == member) {
        if let Some(wanted) = seeded.parameters() {
            let (parameters, _) = classfile::signature(&declared.descriptor).map_err(|detail| {
                GenerateError::Unsupported(format!(
                    "{reviewed}#{member} carries a descriptor that does not read: {detail}"
                ))
            })?;
            if !wanted.matches(&parameters) {
                continue;
            }
        }
        candidates.push(declared);
    }
    let mut names = candidates
        .iter()
        .map(|declared| {
            mapping
                .linked_member(class, &declared.name, &declared.descriptor)
                .0
        })
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    match names.as_slice() {
        [] => Err(GenerateError::Unresolved(format!(
            "a template names {role}#{member_role}, which this version binds to \
             {reviewed}#{member}, and the stub tree does not carry that member"
        ))),
        [one] => Ok(Some(one.clone())),
        several => Err(GenerateError::Unsupported(format!(
            "{reviewed}#{member} links under {} names ({}), so no single reference names them all",
            several.len(),
            several.join(", ")
        ))),
    }
}

/// The stable constant used to load a screen reflectively.
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
    /// A tree over a staged shared directory, with its overrides beside it.
    ///
    /// The tracked trees keep a version's own templates under that version's
    /// data; a staged one keeps them in a subdirectory, because what these tests
    /// ask about is the shared-and-override rule rather than where a repository
    /// files things.
    fn tree<'a>(shared: &'a Path, version: &str, purpose: Purpose) -> Tree<'a> {
        Tree {
            root: shared,
            shared,
            overrides: shared.join(version),
            purpose,
            stand_ins: purpose != Purpose::Screen,
        }
    }

    /// A version that leaves its whole game in packages.
    const PACKAGED: NameSpace = NameSpace {
        unnamed: false,
        split: false,
    };

    /// One that leaves none of it in one, which is the ordinary obfuscated case.
    const UNNAMED: NameSpace = NameSpace {
        unnamed: true,
        split: false,
    };

    /// One that does both at once, which is the case an alias exists for.
    const SPLIT: NameSpace = NameSpace {
        unnamed: true,
        split: true,
    };

    use std::fmt::Write as _;

    use super::super::inputs::{self, MappingSource, Seed, SeedMember, Seeds};
    use super::super::stubs::{Member, Stub};
    use super::*;

    /// A missing staged tree needs no cleanup; every other filesystem failure does.
    fn remove_staged(root: &Path) {
        if let Err(error) = fs::remove_dir_all(root) {
            assert_eq!(
                error.kind(),
                std::io::ErrorKind::NotFound,
                "removing staged tree {}: {error}",
                root.display()
            );
        }
    }

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
    void wrap(int) -> e
    void wrap(long) -> f
com.example.text.Label -> a$b:
    com.example.text.Label of(java.lang.String) -> e
com.example.util.Held -> com.example.util.Held:
    void tick() -> tick
";

    /// The role table the screen tests resolve against, over all three lists.
    fn roles_over(seeds: &Seeds, reflected: &[(&str, &str)], hooks: &[(&str, &str)]) -> Roles {
        Roles::new(
            seeds,
            &crate::test_fixtures::reflection(reflected),
            &inputs::Inventory {
                schema_version: 1,
                minecraft: String::from("0.0.0"),
                hooks: hooks
                    .iter()
                    .map(|(id, class)| inputs::Hook {
                        id: (*id).to_owned(),
                        side: inputs::HookSide::Shared,
                        class: (*class).to_owned(),
                        member: String::from("tick"),
                        descriptor: String::from("()V"),
                        kind: inputs::Kind::InstanceMethod,
                        capability: String::from("test"),
                        refuses: false,
                        title_menu_row: None,
                    })
                    .collect(),
                non_goals: Vec::new(),
            },
        )
    }

    fn roles(seeds: &Seeds) -> Roles {
        roles_over(seeds, &[], &[])
    }

    fn proguard() -> Mapping {
        Mapping::read(MappingSource::OfficialProguardMappings, MAPPING)
            .expect("the invented mapping should parse")
    }

    #[test]
    fn a_standalone_screen_derives_its_namespace_from_the_game_it_references() {
        let root = staged_tree(
            "standalone-namespace",
            "Observation.java.in",
            "{{package}}\npublic class Observation { {{Ticker}} held; }\n",
        );
        let bound = roles_over(&seeds(), &[("Ticker", "Held")], &[]);
        let mapping =
            Mapping::read(MappingSource::UnobfuscatedClassFiles, "").expect("identity mapping");
        let namespace = name_space(&tree(&root, "none", Purpose::Screen), &bound, &mapping)
            .expect("a standalone screen needs no stand-in game");
        assert!(namespace.unnamed);
        assert!(!namespace.split);
        remove_staged(&root);
    }

    fn method(name: &str, descriptor: &str) -> Member {
        Member {
            name: name.to_owned(),
            descriptor: descriptor.to_owned(),
            flags: 1,
            method: true,
            returns: None,
        }
    }

    /// The seed list binding a role to each invented class.
    ///
    /// The roles are the words a template would write, and they are deliberately
    /// not the class names: what is under test is that the binding is what
    /// carries a template, so a test whose role happened to equal the simple
    /// name would pass under a generator that ignored the seed list entirely.
    fn seeds() -> Seeds {
        Seeds {
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
                            parameters: None,
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
                        returns: None,
                    },
                    method("draw", "()V"),
                    method("draw", "(I)V"),
                    method("resize", "()V"),
                    method("resize", "(I)V"),
                    method("wrap", "(I)V"),
                    method("wrap", "(J)V"),
                ],
                super_constructor: None,
            },
            Stub {
                class: "com/example/text/Label".to_owned(),
                flags: 1,
                extends: None,
                implements: Vec::new(),
                members: vec![method("of", "(Ljava/lang/String;)Lcom/example/text/Label;")],
                super_constructor: None,
            },
            Stub {
                class: "com/example/util/Held".to_owned(),
                flags: 1,
                extends: None,
                implements: Vec::new(),
                members: vec![method("tick", "()V")],
                super_constructor: None,
            },
        ]
    }

    #[test]
    fn an_overload_set_that_links_under_one_name_renders_that_name() {
        let name = member_name("Surface", "draw", &roles(&seeds()), &stubs(), &proguard())
            .expect("both overloads link under one name");

        assert_eq!(name, "b");
    }

    #[test]
    fn a_member_the_seeds_do_not_carry_is_refused_rather_than_passed_through() {
        let error = member_name("Surface", "paint", &roles(&seeds()), &stubs(), &proguard())
            .expect_err("a member no seed asked for is stubbed by nothing");

        assert!(matches!(error, GenerateError::Unresolved(_)), "{error}");
    }

    #[test]
    fn a_class_the_mapping_leaves_in_a_package_is_still_imported() {
        let referenced = std::iter::once("Clock".to_owned()).collect();
        let linked = linked_classes(
            &referenced,
            &roles(&seeds()),
            &stubs(),
            &proguard(),
            Purpose::Screen,
        )
        .expect("resolvable");

        assert_eq!(linked["Clock"], "com/example/util/Held");
    }

    #[test]
    fn a_class_the_stub_seeds_do_not_carry_is_refused() {
        let referenced = std::iter::once("Unseeded".to_owned()).collect();

        let error = linked_classes(
            &referenced,
            &roles(&seeds()),
            &stubs(),
            &proguard(),
            Purpose::Screen,
        )
        .expect_err("a template may only name a role the seeds bind");

        assert!(matches!(error, GenerateError::Unresolved(_)), "{error}");
    }

    /// The provenance header is `run-datafix-byte-preservation.sh`'s interface.
    ///
    /// Nothing the generator writes into `target/ravel-jvm` names the game
    /// version — the fact files beside the Java carry the Java release and the
    /// agent release — so that measurement reads the version off the first line
    /// of the screen source it is about to compile. The wording of this header
    /// is therefore load-bearing outside this crate, and reversing the scrape
    /// here is what makes a reworded header fail a test rather than a run
    /// nothing in the gate performs. Both name spaces are checked because the
    /// header rides on the package declaration under one and stands alone under
    /// the other.
    #[test]
    fn the_datafix_measurement_scrapes_the_header_this_generator_writes() {
        let root = repository_root();
        let script = fs::read_to_string(root.join("jvm/harness/run-datafix-byte-preservation.sh"))
            .expect("the measurement script is tracked");
        let (_, scrape) = script
            .split_once("sed -n '1s/^.*")
            .expect("the script scrapes the first line of the generated source");
        let (prefix, rest) = scrape
            .split_once("\\(.*\\)")
            .expect("the scrape captures the version between two literals");
        let (suffix, _) = rest
            .split_once(".*$/\\1/p")
            .expect("the scrape ends by printing the capture");
        let expected = format!(
            "{}{}{}",
            prefix.replace("\\.", "."),
            adapter().minecraft,
            suffix.replace("\\.", ".")
        );

        let shared = staged_screen("provenance-header", "{{package}}\nclass Shared {}\n");
        for space in [PACKAGED, UNNAMED] {
            let rendered = render(
                &tree(&shared, "0.0.0-none", Purpose::Screen),
                &adapter(),
                &roles(&seeds()),
                &stubs(),
                &proguard(),
                space,
            )
            .expect("the staged template names a bound role");
            let (_, source) = &rendered.java[0];
            let first = source.lines().next().expect("a rendered source has a line");
            assert!(
                first.contains(&expected),
                "{first} does not carry {expected}"
            );
        }
        remove_staged(&shared);
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
            writeln!(text, "{} -> o{index}:", stub.class.replace('/', "."))
                .expect("writing to a String cannot fail");
            let mut renamed: BTreeMap<&str, String> = BTreeMap::new();
            for member in &stub.members {
                if member.name == "<init>" {
                    continue;
                }
                let next = format!("m{index}_{}", renamed.len());
                let linked = renamed.entry(member.name.as_str()).or_insert(next).clone();
                if member.method {
                    let (parameters, returns) = classfile::signature(&member.descriptor)
                        .expect("a stubbed descriptor reads");
                    writeln!(
                        text,
                        "    {returns} {}({}) -> {linked}",
                        member.name,
                        parameters.join(",")
                    )
                    .expect("writing to a String cannot fail");
                } else {
                    let (kind, _) = classfile::java_type(&member.descriptor)
                        .expect("a stubbed descriptor reads");
                    writeln!(text, "    {kind} {} -> {linked}", member.name)
                        .expect("writing to a String cannot fail");
                }
            }
        }
        Mapping::read(MappingSource::OfficialProguardMappings, &text)
            .expect("a mapping derived from the stub set should parse")
    }

    /// Digests every source path and byte plus deterministic, root-relative metadata.
    fn modern_snapshot(root: &Path, sources: &[Sources; 4]) -> String {
        let mut snapshot = String::new();
        for (index, tree) in sources.iter().enumerate() {
            let templates: Vec<_> = tree
                .templates
                .iter()
                .map(|path| {
                    path.strip_prefix(root)
                        .expect("a tracked template is under root")
                })
                .collect();
            let added: Vec<_> = tree
                .added
                .iter()
                .map(|(role, path)| {
                    (
                        role,
                        path.strip_prefix(root)
                            .expect("an added template is under root"),
                    )
                })
                .collect();
            writeln!(
                snapshot,
                "{index}:{:?}",
                (
                    &tree.java,
                    &tree.names,
                    &tree.origins,
                    &tree.aliases,
                    &tree.referenced,
                    templates,
                    added,
                )
            )
            .expect("writing a snapshot to a String cannot fail");
        }
        inputs::sha256(snapshot.as_bytes())
    }

    #[test]
    fn both_modern_versions_render_byte_identically_to_the_pre_legacy_baseline() {
        let root = repository_root();
        // Frozen after comparing all four rendered trees with the pre-legacy
        // namespace algorithm from ymklsluq; ordered by tracked modern version.
        let expected = [
            "3d74a99ef71f52b931e15936a6332e59cdf243a5482f04ea24c8a6013febc8fa",
            "5b24c82eb0addbaf3e4e777ce92306456b69b81ae9a931d63f36fe1437c19639",
        ];
        let mut compared = 0;
        for version in inputs::tracked_versions(&root) {
            let pinned = inputs::pinned(&root, Some(&version))
                .expect("the tracked version data should parse");
            if pinned.adapter.is_legacy() {
                continue;
            }
            let lock = fs::read_to_string(pinned.directory.join("stubs.lock"))
                .expect("the modern stub lock is tracked");
            let stubs = super::super::stubs::from_lock(&lock, &pinned.adapter, &pinned.seeds)
                .expect("the modern stub lock resolves");
            let mapping = obfuscating(&stubs);
            let bound = Roles::new(&pinned.seeds, &pinned.surface, &pinned.inventory);
            let actual = super::super::sources(&root, &pinned, &bound, &stubs, &mapping)
                .expect("the modern sources render");
            assert_eq!(
                modern_snapshot(&root, &actual),
                *expected
                    .get(compared)
                    .expect("every modern version has a snapshot"),
                "{version}: source paths, Java bytes and metadata"
            );
            assert!(actual.iter().all(|tree| !tree.java.is_empty()));
            compared += 1;
        }
        assert_eq!(
            compared,
            expected.len(),
            "both modern versions must be compared"
        );
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
        for version in inputs::tracked_versions(&root) {
            let pinned = inputs::pinned(&root, Some(&version))
                .expect("the tracked version data should parse");
            let tracked = fs::read_to_string(pinned.directory.join("stubs.lock"))
                .expect("the tracked stub lock should be readable");
            let stubs = super::super::stubs::from_lock(&tracked, &pinned.adapter, &pinned.seeds)
                .expect("the tracked lock should resolve");
            let mapping = obfuscating(&stubs);

            let legacy = pinned.adapter.is_legacy();
            let shared = if legacy {
                pinned.directory.join("templates/screen")
            } else {
                root.join(super::super::SCREEN_TEMPLATES)
            };
            let selected = tree(&shared, &version, Purpose::Screen);
            let screens = render(
                &selected,
                &pinned.adapter,
                &Roles::new(&pinned.seeds, &pinned.surface, &pinned.inventory),
                &stubs,
                &mapping,
                UNNAMED,
            )
            .expect("the tracked templates should render against an obfuscated mapping");
            assert_ne!(screens.java, [], "{version} renders real screen support");
            if legacy {
                assert!(
                    screens.templates.iter().all(|path| {
                        path.starts_with(pinned.directory.join("templates/screen"))
                    })
                );
            }

            assert_unnamed_screen_references(&screens, &stubs);
        }
    }

    fn assert_unnamed_screen_references(screens: &Sources, stubs: &[Stub]) {
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

    /// Every live modern version's replacement is dirty before the storage sees it.
    ///
    /// Whether the game's own `set` marks it is a per-version fact and not a
    /// safe one to build on: one tracked version's does, another's only caches the object, and
    /// a `SavedData` that is not dirty is skipped when the world writes. That
    /// asymmetry is invisible in a launch — the replacement crosses, nothing
    /// fails, and the world's data folder simply never holds `ravel_mods.dat`,
    /// which is what the first obfuscated-version launch found. So the mark is Ravel's to
    /// make, in the one method that wraps replacement bytes, at every version.
    #[test]
    fn every_modern_versions_replacement_is_marked_dirty_before_the_storage_is_handed_it() {
        let root = repository_root();
        for version in inputs::tracked_versions(&root) {
            let pinned = inputs::pinned(&root, Some(&version))
                .expect("the tracked version data should parse");
            if pinned.adapter.is_legacy() {
                // Generation-only legacy support does not provide world-data replacement.
                // Its real screen templates are exercised by the namespace test above.
                continue;
            }
            let lock = fs::read_to_string(pinned.directory.join("stubs.lock"))
                .expect("the tracked stub lock should be readable");
            let stubs = super::super::stubs::from_lock(&lock, &pinned.adapter, &pinned.seeds)
                .expect("the tracked lock should resolve");
            let mapping = obfuscating(&stubs);
            let shared = root.join(super::super::SCREEN_TEMPLATES);
            let screens = render(
                &tree(&shared, &version, Purpose::Screen),
                &pinned.adapter,
                &roles(&pinned.seeds),
                &stubs,
                &mapping,
                UNNAMED,
            )
            .expect("the tracked templates should render");

            let dirty = member_name(
                "SavedData",
                "setDirty",
                &roles(&pinned.seeds),
                &stubs,
                &mapping,
            )
            .expect("every tracked version binds the saved datum's dirty mark");
            let (_, source) = screens
                .java
                .iter()
                .find(|(relative, _)| relative.ends_with("RavelWorldData.java"))
                .unwrap_or_else(|| panic!("{version} renders no world data source"));
            let wrapping = source.split_once("of(byte[] aggregate)").map_or_else(
                || panic!("{version}'s world data wraps no replacement bytes"),
                |(_, rest)| rest.split_once("\n    }").unwrap_or((rest, "")).0,
            );

            assert!(
                wrapping.contains(&format!(".{dirty}()")),
                "{version} hands the storage a replacement it never marked dirty, which this \
                 version's storage may not mark either: {wrapping}"
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
        assert_eq!(
            Placeholder::read("ravel RavelSettingsScreen").expect("a placeholder body"),
            Placeholder::Ravel("RavelSettingsScreen".to_owned())
        );
        // A body that names nothing and one that names two things are both
        // refused, because either would render as a class name the compiler
        // then fails to find rather than as the refusal the template earned.
        assert!(Placeholder::read("ravel ").is_err());
        assert!(Placeholder::read("ravel Two Names").is_err());
    }

    /// The property the whole indirection exists for: what a template renders is
    /// decided by the binding and not by the word. Both halves are checked,
    /// because a generator that resolved the role and then ignored the member
    /// binding would pass a class-only test.
    #[test]
    fn a_role_renders_the_class_and_member_the_seed_list_binds_it_to() {
        let referenced = std::iter::once("Surface".to_owned()).collect();
        let linked = linked_classes(
            &referenced,
            &roles(&seeds()),
            &stubs(),
            &proguard(),
            Purpose::Screen,
        )
        .expect("resolvable");
        assert_eq!(linked["Surface"], "a");

        let name = member_name("Surface", "size", &roles(&seeds()), &stubs(), &proguard())
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
    fn seeds_binding(member_role: &str, parameters: Option<inputs::Overload>) -> Seeds {
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
        let none = seeds_binding("scale", Some(inputs::Overload::Count(0)));
        assert_eq!(
            member_name("Surface", "scale", &roles(&none), &stubs(), &proguard())
                .expect("resize() is the zero-argument overload"),
            "c"
        );

        let one = seeds_binding("scale", Some(inputs::Overload::Count(1)));
        assert_eq!(
            member_name("Surface", "scale", &roles(&one), &stubs(), &proguard())
                .expect("resize(int) is the one-argument overload"),
            "d"
        );
    }

    /// A seed list binding one member role to an overload of `wrap`.
    ///
    /// `wrap` is the pair a count cannot separate: both overloads take one
    /// parameter and the invented mapping links them as `e` and `f`, which is
    /// the shape that reached a screen as a list of the wrong thing.
    fn seeds_wrapping(parameters: Option<inputs::Overload>) -> Seeds {
        let mut seeds = seeds();
        seeds.stubs[0].members = vec![SeedMember::Bound(inputs::BoundMember {
            role: "split".to_owned(),
            member: "wrap".to_owned(),
            parameters,
        })];
        seeds
    }

    /// The declared types name the overload a count names both of and neither.
    #[test]
    fn declared_parameter_types_pick_an_overload_a_count_cannot() {
        let ambiguous = seeds_wrapping(Some(inputs::Overload::Count(1)));
        let error = member_name(
            "Surface",
            "split",
            &roles(&ambiguous),
            &stubs(),
            &proguard(),
        )
        .expect_err("one parameter each, and two linked names");
        assert!(matches!(error, GenerateError::Unsupported(_)), "{error}");

        for (declared, linked) in [("int", "e"), ("long", "f")] {
            let seeds = seeds_wrapping(Some(inputs::Overload::Types(vec![declared.to_owned()])));
            assert_eq!(
                member_name("Surface", "split", &roles(&seeds), &stubs(), &proguard())
                    .expect("the declared type names one overload"),
                linked
            );
        }
    }

    /// Types that separate nothing are refused for the reason a count that
    /// separates nothing is: a binding naming no member is a defect, not a
    /// licence to fall back to the whole overload set.
    #[test]
    fn declared_parameter_types_matching_no_overload_are_refused() {
        let seeds = seeds_wrapping(Some(inputs::Overload::Types(vec!["boolean".to_owned()])));

        let error = member_name("Surface", "split", &roles(&seeds), &stubs(), &proguard())
            .expect_err("the panel declares no boolean wrap");

        assert!(matches!(error, GenerateError::Unresolved(_)), "{error}");
    }

    /// A count that separates nothing is a binding that names no member, and is
    /// refused rather than falling back to the whole overload set.
    #[test]
    fn a_parameter_count_matching_no_overload_is_refused() {
        let seeds = seeds_binding("scale", Some(inputs::Overload::Count(2)));

        let error = member_name("Surface", "scale", &roles(&seeds), &stubs(), &proguard())
            .expect_err("the panel declares no two-argument resize");

        assert!(matches!(error, GenerateError::Unresolved(_)), "{error}");
    }

    /// Without a count the two overloads are two answers, and the generator says
    /// so instead of picking. This is the refusal the count exists to let a seed
    /// list resolve deliberately.
    #[test]
    fn an_overload_set_linking_under_two_names_without_a_count_is_refused() {
        let seeds = seeds_binding("scale", None);

        let error = member_name("Surface", "scale", &roles(&seeds), &stubs(), &proguard())
            .expect_err("resize links under two names here");

        assert!(matches!(error, GenerateError::Unsupported(_)), "{error}");
    }

    /// The sweep that would have caught `SavedData#setDirty` before a template
    /// reached it: every seeded member is resolved, not only the referenced ones.
    #[test]
    fn a_seeded_member_no_template_names_is_still_required_to_resolve() {
        let ambiguous = seeds_binding("scale", None);

        let error = every_seeded_member_resolves(&roles(&ambiguous), &stubs(), &proguard())
            .expect_err("an ambiguous seed is a defect whether or not a template names it");

        assert!(matches!(error, GenerateError::Unsupported(_)), "{error}");
        let bound = seeds_binding("scale", Some(inputs::Overload::Count(0)));
        every_seeded_member_resolves(&roles(&bound), &stubs(), &proguard())
            .expect("a bound overload resolves");
    }

    #[test]
    fn a_member_role_the_seed_list_does_not_bind_is_refused() {
        let error = member_name("Surface", "width", &roles(&seeds()), &stubs(), &proguard())
            .expect_err("width is a member name here, and the role bound to it is size");

        assert!(matches!(error, GenerateError::Unresolved(_)), "{error}");
    }

    /// A staged template directory: one shared template, and whatever the caller
    /// wants beside it under a version.
    fn staged_templates(name: &str, overrides: &[(&str, &str)]) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("ravel-templates-{name}-{}", std::process::id()));
        remove_staged(&root);
        fs::create_dir_all(root.join("9.9.9")).expect("the staged directory should be made");
        fs::write(root.join("Shared.java.in"), "{{package}}\nshared\n")
            .expect("the shared template should be written");
        for (file, text) in overrides {
            fs::write(root.join("9.9.9").join(file), text).expect("the override should be written");
        }
        root
    }

    /// An override replaces the shared template of its own name, and the version
    /// that has none reads the shared one. Both halves, because a mechanism that
    /// applied nothing would pass a test that only checked the count.
    #[test]
    fn a_version_override_replaces_the_shared_template_of_its_name() {
        let root = staged_templates("applied", &[("Shared.java.in", "{{package}}\noverride\n")]);

        let applied =
            templates(&tree(&root, "9.9.9", Purpose::Screen)).expect("the override applies");
        assert_eq!(applied.rendered.len(), 1);
        assert!(
            applied.added.is_empty(),
            "an override that replaces one adds none"
        );
        assert_eq!(
            applied.overridden.len(),
            1,
            "the record has to say this version's own tree rendered it"
        );
        assert_eq!(
            fs::read_to_string(&applied.rendered[0]).expect("readable"),
            "{{package}}\noverride\n"
        );

        let plain = templates(&tree(&root, "0.0.0-none", Purpose::Screen))
            .expect("a version with no override");
        assert_eq!(plain.rendered.len(), 1);
        assert!(
            plain.overridden.is_empty(),
            "a version that overrides nothing has to say so"
        );
        assert_eq!(
            fs::read_to_string(&plain.rendered[0]).expect("readable"),
            "{{package}}\nshared\n"
        );

        remove_staged(&root);
    }

    /// An override that replaces nothing is a screen only one version draws, and
    /// the agent jar opens the same constants under every version.
    #[test]
    fn a_version_override_that_replaces_nothing_is_refused() {
        let root = staged_templates("orphan", &[("Orphan.java.in", "{{package}}\n")]);

        let error = templates(&tree(&root, "9.9.9", Purpose::Screen))
            .expect_err("an override must replace something");

        assert!(matches!(error, GenerateError::Input(_)), "{error}");
        assert!(
            error.to_string().contains("overrides no shared template"),
            "{error}"
        );
        remove_staged(&root);
    }

    /// A directory inside a version's override directory is refused rather than
    /// skipped: templates are read one level deep, so whatever sat below would
    /// render into nothing while saying nothing.
    #[test]
    fn a_directory_inside_a_version_override_is_refused() {
        let root = staged_templates("nested", &[]);
        fs::create_dir_all(root.join("9.9.9/inner")).expect("creatable");
        fs::write(root.join("9.9.9/inner/Shared.java.in"), "{{package}}\n").expect("writable");

        let error = templates(&tree(&root, "9.9.9", Purpose::Screen))
            .expect_err("a nested directory is refused");

        assert!(error.to_string().contains("one level only"), "{error}");
        remove_staged(&root);
    }

    #[test]
    fn a_constant_is_the_screen_class_name() {
        assert_eq!(constant("RavelSettingsScreen"), "RAVEL_SETTINGS_SCREEN");
        assert_eq!(constant("Wire"), "WIRE");
    }

    /// A template naming one role as an import and as a reference.
    ///
    /// `Ticker` is bound by no seed, so a resolver reading the seed list alone
    /// renders none of this.
    const TICKING: &str = "{{package}}\n{{import Ticker}}\nclass Shared { {{Ticker}} held; }\n";

    /// A template directory holding one shared template.
    fn staged_screen(name: &str, text: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("ravel-screen-{name}-{}", std::process::id()));
        remove_staged(&root);
        fs::create_dir_all(&root).expect("the staged directory should be made");
        fs::write(root.join("Shared.java.in"), text).expect("the template should be written");
        root
    }

    /// A tracked version's adapter, for the tests that render a staged template.
    fn adapter() -> Adapter {
        let root = repository_root();
        let version = inputs::tracked_versions(&root)
            .into_iter()
            .next()
            .expect("the repository tracks a version");
        inputs::pinned(&root, Some(&version))
            .expect("the tracked version data should parse")
            .adapter
    }

    /// A template directory holding one template of the caller's own name.
    fn staged_tree(name: &str, file: &str, text: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("ravel-tree-{name}-{}", std::process::id()));
        remove_staged(&root);
        fs::create_dir_all(&root).expect("the staged directory should be made");
        fs::write(root.join(file), text).expect("the template should be written");
        root
    }

    /// A stand-in is named and placed by the class its role links to, not by its
    /// template.
    ///
    /// Both halves of the name are checked and so is the path, because a
    /// generator that took the template's own word would put a class called
    /// `Clock` in the unnamed package and every one of those three would be
    /// wrong in a different way.
    #[test]
    fn a_stand_in_is_named_and_placed_by_the_role_it_declares() {
        let root = staged_tree(
            "declaring",
            "Clock.java.in",
            "{{package}}\npublic class {{Clock}} {\n}\n",
        );

        let rendered = render(
            &tree(&root, "0.0.0-none", Purpose::StandIn),
            &adapter(),
            &roles(&seeds()),
            &stubs(),
            &proguard(),
            PACKAGED,
        )
        .expect("the seeds bind the role the template is named after");

        let (relative, source) = &rendered.java[0];
        assert_eq!(relative, "com/example/util/Held.java");
        assert!(source.starts_with("package com.example.util;"), "{source}");
        assert!(source.contains("public class Held {"), "{source}");
        remove_staged(&root);
    }

    /// The same template under a mapping that puts the class in the unnamed
    /// package, which is the shape the harness exists to reach.
    #[test]
    fn a_stand_in_whose_class_has_no_package_declares_none() {
        let root = staged_tree(
            "declaring-unnamed",
            "Surface.java.in",
            "{{package}}\npublic class {{Surface}} {\n}\n",
        );

        let rendered = render(
            &tree(&root, "0.0.0-none", Purpose::StandIn),
            &adapter(),
            &roles(&seeds()),
            &stubs(),
            &proguard(),
            UNNAMED,
        )
        .expect("the seeds bind the role the template is named after");

        let (relative, source) = &rendered.java[0];
        assert_eq!(relative, "a.java");
        assert!(!source.starts_with("package "), "{source}");
        assert!(source.contains("public class a {"), "{source}");
        remove_staged(&root);
    }

    /// Where a version's name space is split, every stand-in is written in the
    /// unnamed package and the ones whose class was not says so.
    ///
    /// Both halves are asked of one render, because what a split version needs
    /// is not two right answers but one name space: a generator that moved the
    /// packaged class and left the unnamed one where it was would pass a test
    /// that only looked at the class it moved.
    #[test]
    fn a_split_name_space_is_written_in_the_one_package_both_halves_can_be_named_from() {
        let root = staged_tree(
            "split-declaring",
            "Clock.java.in",
            "{{package}}\npublic class {{Clock}} {\n}\n",
        );
        fs::write(
            root.join("Surface.java.in"),
            "{{package}}\npublic class {{Surface}} {\n}\n",
        )
        .expect("the second template should be written");

        let rendered = render(
            &tree(&root, "0.0.0-none", Purpose::StandIn),
            &adapter(),
            &roles(&seeds()),
            &stubs(),
            &proguard(),
            SPLIT,
        )
        .expect("the seeds bind both roles");

        let written = rendered.java.iter().map(|(path, _)| path.as_str());
        assert_eq!(written.collect::<Vec<_>>(), ["Held.java", "a.java"]);
        for (_, source) in &rendered.java {
            assert!(!source.starts_with("package "), "{source}");
        }
        assert_eq!(
            rendered.aliases,
            [("Held".to_owned(), "com/example/util/Held".to_owned())]
        );
        remove_staged(&root);
    }

    /// A version whose game is in one name space already is written where its
    /// classes link, and records no alias at all.
    #[test]
    fn an_unsplit_name_space_is_written_where_its_classes_link_and_aliases_nothing() {
        let root = staged_tree(
            "unsplit-declaring",
            "Clock.java.in",
            "{{package}}\npublic class {{Clock}} {\n}\n",
        );

        let rendered = render(
            &tree(&root, "0.0.0-none", Purpose::StandIn),
            &adapter(),
            &roles(&seeds()),
            &stubs(),
            &proguard(),
            PACKAGED,
        )
        .expect("the seeds bind the role the template is named after");

        assert_eq!(rendered.java[0].0, "com/example/util/Held.java");
        assert!(rendered.aliases.is_empty(), "{:?}", rendered.aliases);
        remove_staged(&root);
    }

    /// A source naming a class the split moved names it the way the unnamed
    /// package is named: bare, and imported by nothing.
    ///
    /// The screen Ravel draws is in the same line, and it is the control: its
    /// name is Ravel's own rather than the game's, so nothing about a split
    /// game may rewrite it.
    #[test]
    fn a_reference_to_a_moved_class_loses_its_import_and_a_ravel_screen_keeps_its_name() {
        let root = staged_tree(
            "split-driver",
            "Driven.java.in",
            "{{package}}\n{{import Clock}}\npublic class Driven {\n    {{Clock}} held;\n    \
             {{ravel RavelSettingsScreen}} screen;\n}\n",
        );

        let rendered = render(
            &tree(&root, "0.0.0-none", Purpose::Driver),
            &adapter(),
            &roles(&seeds()),
            &stubs(),
            &proguard(),
            SPLIT,
        )
        .expect("a driver names roles like anything else");

        let (_, source) = &rendered.java[0];
        assert!(!source.contains("import "), "{source}");
        assert!(source.contains("    Held held;"), "{source}");
        assert!(
            source.contains("    RavelSettingsScreen screen;"),
            "{source}"
        );
        remove_staged(&root);
    }

    /// The name the agent jar publishes for a screen is the name the half
    /// compiled beside the stand-ins was written under.
    ///
    /// The two are rendered from one name space rather than from their own
    /// roles, and that is the whole point: the screen jar's sources name a class
    /// this version leaves in a package, so a tree that answered from what it
    /// held would draw the screen in a package and publish that name, while the
    /// sibling beside the stand-ins drew the same class bare. The agent jar
    /// opens one name.
    #[test]
    fn the_published_screen_name_is_the_one_the_half_beside_the_stand_ins_was_written_under() {
        let root = staged_screen("split-published", TICKING);
        let bound = roles_over(&seeds(), &[("Ticker", "com.example.util.Held")], &[]);
        let sibling = Tree {
            root: &root,
            shared: &root,
            overrides: root.join("0.0.0-none"),
            purpose: Purpose::Screen,
            stand_ins: true,
        };

        let screens = render(
            &tree(&root, "0.0.0-none", Purpose::Screen),
            &adapter(),
            &bound,
            &stubs(),
            &proguard(),
            SPLIT,
        )
        .expect("the screen jar renders against the stubs");
        let siblings = render(&sibling, &adapter(), &bound, &stubs(), &proguard(), SPLIT)
            .expect("the sibling half renders against the stand-ins");

        assert_eq!(screens.names, siblings.names);
        assert_eq!(screens.java[0].0, siblings.java[0].0);
        // What the two halves may differ in is the game class they spell, and
        // they have to: the screen jar compiles against the stub tree, which
        // carries the linked name, and the sibling against the stand-in written
        // beside it.
        assert!(
            screens.java[0].1.contains("import com.example.util.Held;"),
            "{}",
            screens.java[0].1
        );
        assert!(
            !siblings.java[0].1.contains("import "),
            "{}",
            siblings.java[0].1
        );
        remove_staged(&root);
    }

    /// A tree reaching past the stand-in game to a class outside every package,
    /// under a version whose game is in packages, is refused here.
    ///
    /// It is the one case one name space for every tree does not cover, and the
    /// refusal is what keeps the answer safe to share: javac would reach it too,
    /// a compilation later and without the template in hand.
    #[test]
    fn a_packaged_version_naming_a_class_outside_every_package_is_refused() {
        let root = staged_screen(
            "packaged-outside",
            "{{package}}\n{{import Surface}}\nclass Shared { {{Surface}} held; }\n",
        );

        let Err(error) = render(
            &tree(&root, "0.0.0-none", Purpose::Screen),
            &adapter(),
            &roles(&seeds()),
            &stubs(),
            &proguard(),
            PACKAGED,
        ) else {
            panic!("a source in a package cannot name a class outside every package");
        };

        assert!(matches!(error, GenerateError::Unsupported(_)), "{error}");
        let message = error.to_string();
        assert!(message.contains('a'), "{message}");
        assert!(message.contains("outside every package"), "{message}");
        assert!(message.contains("Shared.java.in"), "{message}");
        remove_staged(&root);
    }

    /// The one answer is read off the stand-in game, and a stand-in game that
    /// spans both name spaces says so.
    #[test]
    fn the_stand_in_game_is_what_says_whether_the_name_space_is_split() {
        let root = staged_tree(
            "asked-split",
            "Clock.java.in",
            "{{package}}\npublic class {{Clock}} {\n}\n",
        );
        let one = name_space(
            &tree(&root, "0.0.0-none", Purpose::StandIn),
            &roles(&seeds()),
            &proguard(),
        )
        .expect("the seeds bind the role the template is named after");
        assert!(!one.split && !one.unnamed);

        fs::write(
            root.join("Surface.java.in"),
            "{{package}}\npublic class {{Surface}} {\n}\n",
        )
        .expect("the second template should be written");
        let both = name_space(
            &tree(&root, "0.0.0-none", Purpose::StandIn),
            &roles(&seeds()),
            &proguard(),
        )
        .expect("the seeds bind both roles");
        assert!(both.split && both.unnamed);

        remove_staged(&root);
    }

    /// Asking that question first does not cost the reader the diagnostic that
    /// names the file: a template named after a role nothing binds is refused
    /// with its own path, whichever of the two reaches it.
    #[test]
    fn a_stand_in_naming_a_role_nothing_binds_is_refused_with_its_file_before_anything_renders() {
        let root = staged_tree(
            "asked-unbound",
            "Unbound.java.in",
            "{{package}}\npublic class {{Unbound}} {\n}\n",
        );

        let error = name_space(
            &tree(&root, "0.0.0-none", Purpose::StandIn),
            &roles(&seeds()),
            &proguard(),
        )
        .expect_err("no admission list binds the role the template is named after");

        let message = error.to_string();
        assert!(message.contains("Unbound.java.in"), "{message}");
        assert!(message.contains("Unbound"), "{message}");
        remove_staged(&root);
    }

    /// A driver keeps the name its template gives it and the package a command
    /// line spells, which is none, even where the game it names is in packages.
    #[test]
    fn a_driver_is_named_by_its_template_and_placed_in_no_package() {
        let root = staged_tree(
            "driver",
            "Driven.java.in",
            "{{package}}\n{{import Clock}}\npublic class Driven {\n    {{Clock}} held;\n}\n",
        );

        let rendered = render(
            &tree(&root, "0.0.0-none", Purpose::Driver),
            &adapter(),
            &roles(&seeds()),
            &stubs(),
            &proguard(),
            PACKAGED,
        )
        .expect("a driver names roles like anything else");

        let (relative, source) = &rendered.java[0];
        assert_eq!(relative, "Driven.java");
        assert!(!source.starts_with("package "), "{source}");
        assert!(source.contains("import com.example.util.Held;"), "{source}");
        remove_staged(&root);
    }

    /// A version that leaves no game class in a package leaves Ravel's screens
    /// out of one too, so a source naming one writes the bare name.
    ///
    /// The whole name is what the packaged case renders, and it is the one the
    /// language refuses here: a class outside every package can be named by no
    /// qualified form at all, so a generator that always qualified would emit a
    /// driver that never compiles under an obfuscated version.
    #[test]
    fn a_screen_ravel_draws_is_named_bare_where_the_game_has_no_package() {
        let root = staged_tree(
            "driver-unnamed-screen",
            "Driven.java.in",
            "{{package}}\n{{import Surface}}\npublic class Driven {\n    {{Surface}} held;\n    \
             {{ravel RavelSettingsScreen}} screen;\n}\n",
        );

        let rendered = render(
            &tree(&root, "0.0.0-none", Purpose::Driver),
            &adapter(),
            &roles(&seeds()),
            &stubs(),
            &proguard(),
            UNNAMED,
        )
        .expect("the seeds bind the role the driver names");

        let (_, source) = &rendered.java[0];
        assert!(
            source.contains("    RavelSettingsScreen screen;"),
            "{source}"
        );
        assert!(!source.contains(SCREEN_PACKAGE), "{source}");
        remove_staged(&root);
    }

    /// A template named after a role no admission list binds is refused, and the
    /// refusal names the file rather than only the word: the word is in every
    /// template and the file is the one a reader opens.
    #[test]
    fn a_stand_in_naming_a_role_nothing_binds_is_refused_with_its_file() {
        let root = staged_tree(
            "declaring-unbound",
            "Unbound.java.in",
            "{{package}}\npublic class {{Unbound}} {\n}\n",
        );

        let Err(error) = render(
            &tree(&root, "0.0.0-none", Purpose::StandIn),
            &adapter(),
            &roles(&seeds()),
            &stubs(),
            &proguard(),
            PACKAGED,
        ) else {
            panic!("no admission list binds the role the template is named after");
        };

        let message = error.to_string();
        assert!(message.contains("Unbound.java.in"), "{message}");
        assert!(message.contains("Unbound"), "{message}");
        remove_staged(&root);
    }

    /// A version-only stand-in is what a version whose game gained a class means.
    /// The same file is still a screen nobody else can open under the screen
    /// tree, and a driver nothing can retire under the driver tree.
    #[test]
    fn a_version_only_template_is_a_stand_in_and_nothing_else() {
        let root = staged_templates("version-only", &[("Only.java.in", "{{package}}\n")]);

        let held = templates(&tree(&root, "9.9.9", Purpose::StandIn))
            .expect("a version's game may carry a class another version's does not");
        assert_eq!(held.rendered.len(), 2);
        assert_eq!(held.added.len(), 1, "the one it added rather than replaced");

        let error = templates(&tree(&root, "9.9.9", Purpose::Screen))
            .expect_err("a screen only one version draws is one the others cannot open");
        assert!(
            error.to_string().contains("overrides no shared template"),
            "{error}"
        );

        // A driver that replaces nothing is refused for a reason of its own: it
        // is compiled by the sweep that finds it and named by no role, so an
        // added one renders and compiles with nothing able to say it outlived
        // the source it was written for.
        let error = templates(&tree(&root, "9.9.9", Purpose::Driver))
            .expect_err("a driver only one version renders is reached by no role");
        let message = error.to_string();
        assert!(
            message.contains("overrides no shared template"),
            "{message}"
        );
        assert!(message.contains("reached by no role"), "{message}");
        remove_staged(&root);
    }

    /// A version-only stand-in has to be reached by something the version
    /// renders, and the source that reaches it may be in another tree.
    ///
    /// Both halves: the stand-in a driver names is kept, and the one nothing
    /// names is refused with its file. Without the second half an override left
    /// behind after its caller was deleted renders a class on one version that
    /// no compiler ever reads.
    #[test]
    fn a_version_only_stand_in_nothing_reaches_is_refused_with_its_file() {
        let shared = staged_tree(
            "reached-game",
            "Text.java.in",
            "{{package}}\npublic class {{Text}} {\n}\n",
        );
        let overrides = staged_tree(
            "reached-overrides",
            "Clock.java.in",
            "{{package}}\npublic class {{Clock}} {\n}\n",
        );
        let game = Tree {
            root: shared.parent().expect("staged tree parent"),
            shared: &shared,
            overrides: overrides.clone(),
            purpose: Purpose::StandIn,
            stand_ins: true,
        };
        let render_game = || {
            render(
                &game,
                &adapter(),
                &roles(&seeds()),
                &stubs(),
                &proguard(),
                SPLIT,
            )
            .expect("both roles are bound")
        };

        let naming = staged_tree(
            "reached-driver",
            "Driven.java.in",
            "{{package}}\n{{import Clock}}\npublic class Driven {\n    {{Clock}} held;\n}\n",
        );
        let drivers = version_only_stand_in_driver(&naming);
        every_added_template_is_reached(game.root, &[&render_game(), &drivers])
            .expect("the driver in the other tree reaches the stand-in");

        let silent = staged_tree(
            "reached-silent",
            "Driven.java.in",
            "{{package}}\npublic class Driven {\n}\n",
        );
        let quiet = version_only_stand_in_driver(&silent);

        let Err(error) = every_added_template_is_reached(game.root, &[&render_game(), &quiet])
        else {
            panic!("nothing this version renders names the added stand-in");
        };
        let message = error.to_string();
        assert!(message.contains("Clock.java.in"), "{message}");
        assert!(
            message.contains("outlived the template it shadowed"),
            "{message}"
        );

        for root in [shared, overrides, naming, silent] {
            remove_staged(&root);
        }
    }

    fn version_only_stand_in_driver(shared: &Path) -> Sources {
        let driver = Tree {
            root: shared,
            shared,
            overrides: shared.join("0.0.0-none"),
            purpose: Purpose::Driver,
            stand_ins: true,
        };
        render(
            &driver,
            &adapter(),
            &roles(&seeds()),
            &stubs(),
            &proguard(),
            SPLIT,
        )
        .expect("the driver names only bound roles")
    }

    /// A member role a runtime list declares renders without the stub seeds
    /// binding it, which is what a stand-in declaring a reflected member needs.
    ///
    /// The field is the one the invented mapping renames, so a resolver that
    /// passed the reviewed name through would answer `width` here.
    #[test]
    fn a_member_role_the_reflective_surface_declares_renders() {
        let bound = Roles::new(
            &seeds(),
            &inputs::Reflection {
                schema_version: 1,
                minecraft: String::from("0.0.0"),
                classes: vec![inputs::Reflected {
                    name: "Ticker".to_owned(),
                    class: "com.example.gui.Panel".to_owned(),
                    runtime: "com.example.gui.Panel".to_owned(),
                    members: vec![inputs::ReflectedMember {
                        name: "width".to_owned(),
                        lookup: inputs::Lookup::Descriptor("I".to_owned()),
                    }],
                }],
            },
            &inputs::Inventory {
                schema_version: 1,
                minecraft: String::from("0.0.0"),
                hooks: Vec::new(),
                non_goals: Vec::new(),
            },
        );

        let name = member_name("Ticker", "width", &bound, &stubs(), &proguard())
            .expect("the surface declares the member by descriptor");

        assert_eq!(name, "j");
    }

    #[test]
    fn a_role_only_the_reflective_surface_binds_renders_through_a_screen() {
        let root = staged_screen("reflected", TICKING);
        let bound = roles_over(&seeds(), &[("Ticker", "com.example.util.Held")], &[]);

        let screens = render(
            &tree(&root, "0.0.0-none", Purpose::Screen),
            &adapter(),
            &bound,
            &stubs(),
            &proguard(),
            PACKAGED,
        )
        .expect("reflection.toml binds the role the template names");

        let (_, source) = &screens.java[0];
        assert!(source.contains("import com.example.util.Held;"), "{source}");
        assert!(source.contains("Held held;"), "{source}");
        remove_staged(&root);
    }

    #[test]
    fn a_role_only_the_hook_inventory_binds_renders_through_a_screen() {
        let root = staged_screen("hooked", TICKING);
        let bound = roles_over(&seeds(), &[], &[("Ticker", "com.example.util.Held")]);

        let screens = render(
            &tree(&root, "0.0.0-none", Purpose::Screen),
            &adapter(),
            &bound,
            &stubs(),
            &proguard(),
            PACKAGED,
        )
        .expect("hooks.toml binds the role the template names");

        let (_, source) = &screens.java[0];
        assert!(source.contains("import com.example.util.Held;"), "{source}");
        assert!(source.contains("Held held;"), "{source}");
        remove_staged(&root);
    }

    #[test]
    fn two_lists_disagreeing_about_a_rendered_role_refuse_the_screen() {
        let root = staged_screen("disagreeing", TICKING);
        let bound = roles_over(
            &seeds(),
            &[("Ticker", "com.example.util.Held")],
            &[("Ticker", "com.example.gui.Panel")],
        );

        let Err(error) = render(
            &tree(&root, "0.0.0-none", Purpose::Screen),
            &adapter(),
            &bound,
            &stubs(),
            &proguard(),
            PACKAGED,
        ) else {
            panic!("two lists bind the role to two classes");
        };

        assert!(matches!(error, GenerateError::Input(_)), "{error}");
        let message = error.to_string();
        for named in [
            "com.example.util.Held",
            "com.example.gui.Panel",
            "reflection.toml",
            "hooks.toml",
        ] {
            assert!(message.contains(named), "{message}");
        }
        remove_staged(&root);
    }

    #[test]
    fn a_member_role_on_a_disagreeing_class_reports_the_disagreement() {
        let bound = roles_over(
            &seeds(),
            &[("Ticker", "com.example.util.Held")],
            &[("Ticker", "com.example.gui.Panel")],
        );

        let error = member_name("Ticker", "tick", &bound, &stubs(), &proguard())
            .expect_err("two lists bind the role to two classes");
        let message = error.to_string();
        for named in ["com.example.gui.Panel", "hooks.toml"] {
            assert!(message.contains(named), "{message}");
        }
    }

    /// A bound class with no member binding is a different defect from a role
    /// nothing binds, and the two say so differently.
    #[test]
    fn a_member_role_on_a_class_only_another_list_binds_names_that_list() {
        let bound = roles_over(&seeds(), &[("Ticker", "com.example.util.Held")], &[]);

        let held = member_name("Ticker", "tick", &bound, &stubs(), &proguard())
            .expect_err("only stubs.toml binds member roles");
        assert!(
            held.to_string()
                .contains("reflection.toml binds a class to that role"),
            "{held}"
        );

        let unbound = member_name("Unbound", "tick", &bound, &stubs(), &proguard())
            .expect_err("no list binds the role at all");
        assert!(
            unbound.to_string().contains("none of this version's"),
            "{unbound}"
        );
    }
}

#[cfg(test)]
mod path_redaction_tests {
    use super::{in_directory, template};
    #[test]
    fn shared_and_override_labels_remain_distinct_inside_the_repository() {
        let root = std::path::Path::new("/private/repository");
        let shared = root.join("jvm/screen/template/Menu.java.in");
        let overrides = root.join("minecraft/versions/test/screen/Menu.java.in");
        assert_eq!(
            super::diagnostic_path(root, &shared),
            "jvm/screen/template/Menu.java.in"
        );
        assert_eq!(
            super::diagnostic_path(root, &overrides),
            "minecraft/versions/test/screen/Menu.java.in"
        );
    }

    #[test]
    fn unreadable_templates_never_name_local_parent_directories() {
        let private = std::env::temp_dir()
            .join(format!(
                "ravel-private-template-path-{}",
                std::process::id()
            ))
            .join("absent.java.in");
        for error in [
            template(private.parent().expect("private parent"), &private)
                .err()
                .expect("missing template"),
            in_directory(private.parent().expect("private parent"), &private)
                .expect_err("missing directory"),
        ] {
            let message = error.to_string();
            assert!(message.contains("absent.java.in"), "{message}");
            assert!(
                !message.contains("ravel-private-template-path"),
                "{message}"
            );
        }
    }
}
