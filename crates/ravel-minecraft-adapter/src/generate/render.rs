//! Deterministically renders resolved hooks, locks, and Java bridge declarations.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use super::inputs::{self, Adapter, Inventory, Lookup, Reflection, Spelling};
use super::mapping::Mapping;
use super::resolve::Resolved;
use super::stubs::Stub;
use super::{EXTRACTOR, GenerateError, HOOK_CLASS, LIBRARY, LIBRARY_PROPERTY};

/// A JVM type a hook argument can carry across the boundary.
///
/// Primitive and string values are copied; other references are borrowed opaque handles.
#[derive(Clone, Debug, Eq, PartialEq)]
enum JavaType {
    Boolean,
    Byte,
    Char,
    Short,
    Int,
    Long,
    Float,
    Double,
    Text,
    /// A `byte[]`, copied across as the opaque payload it is.
    ///
    /// A world's aggregate travels as bytes Java never reads, so this is
    /// the one array that is not an opaque handle: a handle would let a hook
    /// know a `byte[]` is there and nothing more.
    Bytes,
    /// A reference, carrying the internal class name its descriptor declared.
    Handle(String),
}

impl JavaType {
    /// The JVM descriptor of this type *in the generated hook's signature*.
    ///
    /// A handle erases to `java.lang.Object` here, and only here: the target's
    /// own descriptor keeps the declared class, because that is what the agent
    /// matches a method on, while the hook it calls takes the widest reference
    /// there is. That erasure is the opacity, expressed on the Java side rather
    /// than only promised by a doc comment — a generated hook that named a game
    /// class could offer a field, and one taking `Object` cannot.
    const fn descriptor(&self) -> &'static str {
        match self {
            Self::Boolean => "Z",
            Self::Byte => "B",
            Self::Char => "C",
            Self::Short => "S",
            Self::Int => "I",
            Self::Long => "J",
            Self::Float => "F",
            Self::Double => "D",
            Self::Text => "Ljava/lang/String;",
            Self::Bytes => "[B",
            Self::Handle(_) => "Ljava/lang/Object;",
        }
    }

    /// How the generated Java declares a parameter of this type.
    ///
    /// Nothing here names a game type, which is what lets `Hook.java` compile
    /// without the client jar on its class path, exactly as the transformer does.
    const fn java(&self) -> &'static str {
        match self {
            Self::Boolean => "boolean",
            Self::Byte => "byte",
            Self::Char => "char",
            Self::Short => "short",
            Self::Int => "int",
            Self::Long => "long",
            Self::Float => "float",
            Self::Double => "double",
            Self::Text => "String",
            Self::Bytes => "byte[]",
            Self::Handle(_) => "Object",
        }
    }

    /// How the generated Rust declares a parameter of this type.
    const fn rust(&self) -> &'static str {
        match self {
            Self::Boolean => "::jni::sys::jboolean",
            Self::Byte => "::jni::sys::jbyte",
            Self::Char => "::jni::sys::jchar",
            Self::Short => "::jni::sys::jshort",
            Self::Int => "::jni::sys::jint",
            Self::Long => "::jni::sys::jlong",
            Self::Float => "::jni::sys::jfloat",
            Self::Double => "::jni::sys::jdouble",
            Self::Text => "::jni::objects::JString<'local>",
            Self::Bytes => "::jni::objects::JByteArray<'local>",
            Self::Handle(_) => "::jni::objects::JObject<'local>",
        }
    }

    /// How the generated Rust turns a parameter into a hook value.
    ///
    /// A handle *borrows* its parameter rather than moving it. The exported
    /// frame keeps ownership of the local reference the JVM gave it, so the
    /// borrow checker ends the handle with the frame and no rule has to be
    /// remembered for it to.
    fn value(&self, name: &str) -> String {
        match self {
            Self::Boolean => format!("::ravel_minecraft::HookValue::Boolean({name})"),
            Self::Byte => format!("::ravel_minecraft::HookValue::Byte({name})"),
            Self::Char => format!("::ravel_minecraft::HookValue::Char({name})"),
            Self::Short => format!("::ravel_minecraft::HookValue::Short({name})"),
            Self::Int => format!("::ravel_minecraft::HookValue::Int({name})"),
            Self::Long => format!("::ravel_minecraft::HookValue::Long({name})"),
            Self::Float => format!("::ravel_minecraft::HookValue::Float({name})"),
            Self::Double => format!("::ravel_minecraft::HookValue::Double({name})"),
            Self::Text => {
                format!("::ravel_minecraft::HookValue::Text(crate::guard::text(env, &{name})?)")
            }
            // A byte array still reaches an observer as the handle every array
            // is: what `Bytes` changes is the crossing's declared type, not what
            // an observer can see.
            Self::Bytes => format!(
                "::ravel_minecraft::HookValue::Handle(\
                 ::ravel_minecraft::JavaHandle::new(\"[B\", &{name}))"
            ),
            Self::Handle(class) => format!(
                "::ravel_minecraft::HookValue::Handle(\
                 ::ravel_minecraft::JavaHandle::new(\"{class}\", &{name}))"
            ),
        }
    }
}

/// One method hook, with every name the generated artefacts need.
#[derive(Clone, Debug)]
pub(super) struct Shape {
    capability: String,
    constant: String,
    rust_function: String,
    java_method: String,
    parameters: Vec<JavaType>,
    runtime_class: String,
    runtime_member: String,
    runtime_descriptor: String,
    is_static: bool,
    refuses: bool,
}

impl Shape {
    /// The descriptor of the generated hook method.
    ///
    /// An ordinary hook returns nothing, because the injected call site then
    /// needs no branch or discarded value. A hook that has to stop the call
    /// throws, which the call site honours without any bytecode of its own.
    ///
    /// A target the reviewed inventory opts in returns a boolean instead:
    /// the verdict says only whether to return from the host method before
    /// its body runs, never what to return.
    fn hook_descriptor(&self) -> String {
        let mut text = String::from("(");
        for parameter in &self.parameters {
            text.push_str(parameter.descriptor());
        }
        text.push_str(if self.refuses { ")Z" } else { ")V" });
        text
    }

    /// How the generated Java declares this hook's return type.
    const fn java_returns(&self) -> &'static str {
        if self.refuses { "boolean" } else { "void" }
    }
}

/// Derives every method hook's generated names, refusing anything unrepresentable.
pub(super) fn shapes(
    inventory: &Inventory,
    resolved: &[Resolved],
) -> Result<Vec<Shape>, GenerateError> {
    let mut shapes = Vec::new();
    for (hook, target) in inventory.hooks.iter().zip(resolved) {
        if !hook.kind.is_method() {
            continue;
        }
        // Refusal permits a bare early return and nothing wider. A target that
        // returns a value would need one synthesized to leave early, which is
        // unsupported, so this is a hard error rather than a warning that
        // renders anyway.
        if hook.refuses && !target.runtime_descriptor.ends_with(")V") {
            return Err(GenerateError::Unsupported(format!(
                "{} may refuse its target, but {} does not return void and an early \
                 return would have to invent a value",
                hook.id, target.descriptor
            )));
        }
        shapes.push(Shape {
            capability: hook.capability.clone(),
            constant: constant_name(&hook.id),
            rust_function: rust_name(&hook.id),
            java_method: java_name(&hook.id),
            // The reviewed descriptor, because the only thing a parameter keeps
            // of its class is the string `JavaHandle::new` declares, and that
            // one is read by a mod rather than linked by the JVM.
            parameters: parameters(&target.descriptor)
                .map_err(|detail| GenerateError::Unsupported(format!("{}: {detail}", hook.id)))?,
            runtime_class: target.runtime_class.clone(),
            runtime_member: target.runtime_member.clone(),
            runtime_descriptor: target.runtime_descriptor.clone(),
            is_static: target.is_static,
            refuses: hook.refuses,
        });
    }

    // The generated Java class never overloads, which is what lets the exported
    // symbols use JNI's short mangled form. Two identifiers that differ only in
    // their separators would break that, so they are refused here rather than at
    // the first hook invocation.
    let mut names = shapes
        .iter()
        .map(|shape| shape.java_method.as_str())
        .collect::<Vec<_>>();
    names.sort_unstable();
    let total = names.len();
    names.dedup();
    if names.len() != total {
        return Err(GenerateError::Unsupported(
            "two reviewed hooks generate the same Java method name".to_owned(),
        ));
    }
    Ok(shapes)
}

/// Reads the parameter types out of a JVM method descriptor.
fn parameters(descriptor: &str) -> Result<Vec<JavaType>, String> {
    let mut rest = descriptor
        .strip_prefix('(')
        .and_then(|rest| rest.split(')').next())
        .ok_or_else(|| format!("{descriptor} is not a method descriptor"))?;

    let mut parameters = Vec::new();
    while !rest.is_empty() {
        let (parameter, remainder) = parameter(rest)?;
        parameters.push(parameter);
        rest = remainder;
    }
    Ok(parameters)
}

/// Reads one parameter descriptor, returning it and whatever follows it.
fn parameter(descriptor: &str) -> Result<(JavaType, &str), String> {
    let first = descriptor
        .chars()
        .next()
        .ok_or_else(|| "a parameter descriptor is empty".to_owned())?;
    let primitive = match first {
        'Z' => Some(JavaType::Boolean),
        'B' => Some(JavaType::Byte),
        'C' => Some(JavaType::Char),
        'S' => Some(JavaType::Short),
        'I' => Some(JavaType::Int),
        'J' => Some(JavaType::Long),
        'F' => Some(JavaType::Float),
        'D' => Some(JavaType::Double),
        _ => None,
    };
    if let Some(kind) = primitive {
        return Ok((kind, &descriptor[1..]));
    }

    match first {
        'L' => {
            let end = descriptor
                .find(';')
                .ok_or_else(|| format!("{descriptor} names a class that is never terminated"))?;
            let name = &descriptor[1..end];
            // The inventory's descriptor is written by hand and reviewed, so a
            // slip that names no class at all is worth catching here rather than
            // rendering a handle onto nothing.
            if name.is_empty() {
                return Err(format!("{descriptor} names no class between L and ;"));
            }
            let kind = if name == "java/lang/String" {
                // A string is copied rather than handled: it is immutable, so a
                // copy of one cannot go stale, and an owned `String` is what a
                // hook wants to look at anyway.
                JavaType::Text
            } else {
                JavaType::Handle(name.to_owned())
            };
            Ok((kind, &descriptor[end + 1..]))
        }
        '[' => {
            // An array is an object, so it crosses as a handle like any other.
            // The element descriptor is parsed only to find where this one ends
            // and to refuse an array of nothing valid.
            let (_, after) = parameter(&descriptor[1..])?;
            // `[B` and nothing else is the exception, because a world's aggregate
            // is bytes rather than an object a hook looks at. `[[B` is still a
            // handle: it is an array of arrays, not a payload.
            if descriptor.len() - after.len() == 2 && descriptor.starts_with("[B") {
                return Ok((JavaType::Bytes, after));
            }
            let end = descriptor.len() - after.len();
            Ok((JavaType::Handle(descriptor[..end].to_owned()), after))
        }
        other => Err(format!("{other} is not a parameter descriptor")),
    }
}

/// Turns a hook identifier into a Rust constant name.
fn constant_name(id: &str) -> String {
    id.replace(['.', '-'], "_").to_uppercase()
}

/// Turns a hook identifier into a Rust function name.
fn rust_name(id: &str) -> String {
    id.replace(['.', '-'], "_").to_lowercase()
}

/// Turns a hook identifier into a Java method name.
fn java_name(id: &str) -> String {
    let mut name = String::with_capacity(id.len());
    for (index, segment) in id
        .split(['.', '-', '_'])
        .filter(|s| !s.is_empty())
        .enumerate()
    {
        let mut characters = segment.chars();
        let Some(first) = characters.next() else {
            continue;
        };
        if index == 0 {
            name.extend(first.to_lowercase());
        } else {
            name.extend(first.to_uppercase());
        }
        name.push_str(characters.as_str());
    }
    name
}

/// Renders the mapping provenance lock.
pub(super) fn lock(adapter: &Adapter, resolved: &[Resolved]) -> String {
    let mut text = String::with_capacity(resolved.len() * 192 + 512);
    let _ = writeln!(
        text,
        "# Generated mapping provenance for Minecraft {}. Do not edit.\n\
         #\n\
         # Every line pins one resolved member. A change to the jar, the\n\
         # inventory, or the extractor changes this file, which is how drift is\n\
         # noticed before a build trusts a binding.\n\
         \n\
         schemaVersion = {}\n\
         minecraft = \"{}\"\n\
         mappingSource = \"{}\"\n\
         extractor = \"{EXTRACTOR}\"\n\
         clientJarSha256 = \"{}\"",
        adapter.minecraft,
        adapter.schema_version,
        adapter.minecraft,
        adapter.mapping_source.as_str(),
        adapter.inputs.client_jar.sha256.as_deref().unwrap_or("")
    );
    // A source that reads no mapping has none to pin, so the key exists only
    // where one decided the linked names below.
    if let Some(mappings) = &adapter.inputs.client_mappings {
        let _ = writeln!(
            text,
            "clientMappingsSha256 = \"{}\"",
            mappings.sha256.as_deref().unwrap_or("")
        );
    }
    for target in resolved {
        let _ = writeln!(
            text,
            "\n[target.\"{}\"]\nclass = \"{}\"\nmember = \"{}\"\ndescriptor = \"{}\"\nstatic = {}",
            target.id, target.mojang_class, target.member, target.descriptor, target.is_static
        );
        // A linked name is written only where the mapping source made one, so a
        // lock under a source that translates nothing says each name once and a
        // reader can see at a glance which names moved.
        for (key, reviewed, linked) in [
            ("runtimeClass", &target.mojang_class, &target.runtime_class),
            ("runtimeMember", &target.member, &target.runtime_member),
            (
                "runtimeDescriptor",
                &target.descriptor,
                &target.runtime_descriptor,
            ),
        ] {
            if reviewed != linked {
                let _ = writeln!(text, "{key} = \"{linked}\"");
            }
        }
    }
    text
}

/// Renders the Rust target table the adapter includes.
pub(super) fn bindings(adapter: &Adapter, inventory: &Inventory, resolved: &[Resolved]) -> String {
    let mut text = String::with_capacity(resolved.len() * 320 + 768);
    let _ = writeln!(
        text,
        "// Generated from the pinned Minecraft {} inputs. Do not edit.\n\
         //\n\
         // Source of names: {}. Regenerate by building this crate.\n\
         \n\
         use ravel_minecraft::{{HookId, HookKind, HookTarget, PackFormat, PackFormatSpelling}};\n\
         \n\
         /// The Minecraft version every target below was resolved against.\n\
         pub const MINECRAFT_VERSION: &str = \"{}\";\n",
        adapter.minecraft,
        adapter.mapping_source.as_str(),
        adapter.minecraft
    );

    let format = &adapter.pack_format;
    let _ = writeln!(
        text,
        "/// What a resource pack this version reads declares as its format.\n\
         pub const RESOURCE_PACK_FORMAT: PackFormat = PackFormat::new({}, {});\n\
         \n\
         /// What a data pack this version reads declares as its format.\n\
         pub const DATA_PACK_FORMAT: PackFormat = PackFormat::new({}, {});\n\
         \n\
         /// The shape this version's own packs write their format in.\n\
         pub const PACK_FORMAT_SPELLING: PackFormatSpelling = PackFormatSpelling::{};\n",
        format.resource,
        format.resource_minor,
        format.data,
        format.data_minor,
        match format.spelling {
            Spelling::PackFormat => "PackFormat",
            Spelling::MinMaxFormat => "MinMaxFormat",
        }
    );

    let _ = writeln!(
        text,
        "/// Capabilities this version reviewed and deliberately does not carry.\n\
         ///\n\
         /// A caller naming one of these is naming a decision rather than a typo.\n\
         pub const NON_GOALS: [HookId; {}] = [{}];\n",
        inventory.non_goals.len(),
        inventory
            .non_goals
            .iter()
            .map(|non_goal| format!("HookId::new(\"{}\")", non_goal.id))
            .collect::<Vec<_>>()
            .join(", ")
    );

    for (hook, target) in inventory.hooks.iter().zip(resolved) {
        let _ = writeln!(
            text,
            "/// {}\n\
             pub const {}: HookTarget = HookTarget::new(\n\
             \x20   HookId::new(\"{}\"),\n\
             \x20   \"{}\",\n\
             \x20   \"{}\",\n\
             \x20   \"{}\",\n\
             \x20   HookKind::{},\n\
             );\n",
            hook.capability,
            constant_name(&hook.id),
            target.id,
            target.runtime_class,
            target.runtime_member,
            target.runtime_descriptor,
            hook.kind.variant()
        );
    }

    let _ = writeln!(
        text,
        "/// Every target the reviewed inventory pinned, in inventory order.\n\
         pub static TARGETS: [HookTarget; {}] = [",
        resolved.len()
    );
    for target in resolved {
        let _ = writeln!(text, "    {},", constant_name(&target.id));
    }
    let _ = writeln!(text, "];");
    text
}

/// Renders the JNI export stubs the bootstrap includes.
pub(super) fn exports(shapes: &[Shape]) -> String {
    let mut text = String::with_capacity(shapes.len() * 512 + 768);
    text.push_str(
        "// Generated JNI exports for the pinned hook surface. Do not edit.\n\
         //\n\
         // One export per hook shape: a single dispatcher\n\
         // taking an argument array would allocate on every crossing, and the\n\
         // measured crossing allocates nothing. Every declaration is emitted by\n\
         // the generator, which is what lets this crate keep\n\
         // `#![forbid(unsafe_code)]` verbatim.\n\n",
    );

    for shape in shapes {
        let names = (0..shape.parameters.len())
            .map(|index| format!("argument{index}"))
            .collect::<Vec<_>>();
        let carries_text = shape.parameters.contains(&JavaType::Text);

        // A refusing export answers the injected branch, so its verdict is the
        // whole point of calling it and dropping one is a bug.
        let must_use = if shape.refuses { "#[must_use]\n" } else { "" };
        let _ = writeln!(
            text,
            "/// {}\n\
             #[::jni::jni_mangle(\"{HOOK_CLASS}\", \"{}\")]\n\
             {must_use}\
             pub fn {}<'local>(\n\
             \x20   env: ::jni::EnvUnowned<'local>,\n\
             \x20   _class: ::jni::objects::JClass<'local>,",
            shape.capability, shape.java_method, shape.rust_function
        );
        for (name, parameter) in names.iter().zip(&shape.parameters) {
            let _ = writeln!(text, "    {name}: {},", parameter.rust());
        }

        let values = if names.is_empty() {
            "::std::vec::Vec::new()".to_owned()
        } else {
            let rendered = names
                .iter()
                .zip(&shape.parameters)
                .map(|(name, parameter)| parameter.value(name))
                .collect::<Vec<_>>()
                .join(",\n            ");
            format!("::std::vec![\n            {rendered},\n        ]")
        };

        // A refusing export answers the injected branch, so it returns the
        // guard's verdict rather than discarding it.
        let (returns, entry, terminator) = if shape.refuses {
            (" -> ::jni::sys::jboolean", "enter_refusable", "")
        } else {
            ("", "enter", ";")
        };
        let _ = writeln!(
            text,
            "){returns} {{\n\
             \x20   crate::guard::{entry}(\n\
             \x20       env,\n\
             \x20       &::ravel_minecraft_adapter::pinned::{},\n\
             \x20       |{}| ::std::result::Result::Ok({values}),\n\
             \x20   ){terminator}\n\
             }}\n",
            shape.constant,
            if carries_text { "env" } else { "_env" }
        );
    }
    text
}

/// The library load the generated `Hook` performs, and why it names a file.
///
/// Uses an absolute path rather than launcher-managed library search paths.
fn library_load_java() -> String {
    format!(
        "\
        \x20   /** Native library carrying the generated exports. */\n\
         \x20   public static final String LIBRARY = \"{LIBRARY}\";\n\n\
         \x20   /** System property naming the file to load the library from. */\n\
         \x20   public static final String LIBRARY_PROPERTY = \"{LIBRARY_PROPERTY}\";\n\n\
         \x20   static {{\n\
         \x20       loadLibrary();\n\
         \x20   }}\n\n\
         \x20   // JNI binds a loaded library to the class loader that loaded the class\n\
         \x20   // whose code called System.load, and resolves native methods only against\n\
         \x20   // that loader's copy of the declaring class. Loading it here, from the\n\
         \x20   // declaring class's own initializer, makes those two loaders the same one\n\
         \x20   // by construction rather than by convention, so a second copy of this class\n\
         \x20   // under another loader cannot bind to a library this one loaded. What binds\n\
         \x20   // the library is which class's code called, not which overload it called.\n\
         \x20   //\n\
         \x20   // The file is named rather than searched because the launcher owns\n\
         \x20   // java.library.path: it appends its own after the instance's and the last\n\
         \x20   // one wins, and it rebuilds the directory that path names on every launch.\n\
         \x20   // A library reached by that search is one another program decides the\n\
         \x20   // existence of.\n\
         \x20   //\n\
         \x20   // The load is a restricted method under JEP 472. It is exactly the\n\
         \x20   // operation this class exists to perform, and the launch enables native\n\
         \x20   // access for it, so the warning is acknowledged here rather than silenced\n\
         \x20   // across the whole class.\n\
         \x20   @SuppressWarnings(\"restricted\")\n\
         \x20   private static void loadLibrary() {{\n\
         \x20       String named = System.getProperty(LIBRARY_PROPERTY);\n\
         \x20       if (named == null || named.isEmpty()) {{\n\
         \x20           throw new IllegalStateException(\n\
         \x20                   \"-D\"\n\
         \x20                           + LIBRARY_PROPERTY\n\
         \x20                           + \" is unset, so nothing names the \"\n\
         \x20                           + LIBRARY\n\
         \x20                           + \" library this class loads\");\n\
         \x20       }}\n\
         \x20       java.io.File file = new java.io.File(named);\n\
         \x20       if (!file.isFile()) {{\n\
         \x20           throw new IllegalStateException(\n\
         \x20                   \"-D\"\n\
         \x20                           + LIBRARY_PROPERTY\n\
         \x20                           + \" names \"\n\
         \x20                           + named\n\
         \x20                           + \", which is not a file, so the \"\n\
         \x20                           + LIBRARY\n\
         \x20                           + \" library cannot be loaded\");\n\
         \x20       }}\n\
         \x20       System.load(file.getAbsolutePath());\n\
         \x20   }}\n\n"
    )
}

/// Renders the generated Java hook class.
pub(super) fn hook_java(adapter: &Adapter, shapes: &[Shape]) -> String {
    let mut text = String::with_capacity(shapes.len() * 384 + 2048);
    let load = library_load_java();
    let _ = writeln!(
        text,
        "// Generated from the pinned Minecraft {} inputs. Do not edit.\n\
         package dev.ravel.jvm;\n\n\
         /**\n\
         \x20* Hook surface the agent injects calls to.\n\
         \x20*\n\
         \x20* <p>Every method here is static, and all but the opted-in ones return\n\
         \x20* void: the injected call site loads the target's own arguments, calls one\n\
         \x20* of these, and continues. A hook that has to stop the call throws, which\n\
         \x20* needs no bytecode of its own at the call site.\n\
         \x20*\n\
         \x20* <p>A target the reviewed inventory opts in answers a boolean instead,\n\
         \x20* and the call site returns from the host method when it is true. The\n\
         \x20* verdict says only whether to return, never what to return, so a target\n\
         \x20* that returns anything but void cannot opt in at all.\n\
         \x20*/\n\
         public final class Hook {{\n\n\
{load}\
         \x20   /** Targets the agent rewrites, in reviewed inventory order. */\n\
         \x20   public static final HookTarget[] TARGETS = {{",
        adapter.minecraft
    );

    for shape in shapes {
        let _ = writeln!(
            text,
            "        new HookTarget(\n\
             \x20               \"{}\",\n\
             \x20               \"{}\",\n\
             \x20               \"{}\",\n\
             \x20               {},\n\
             \x20               \"{}\",\n\
             \x20               \"{}\",\n\
             \x20               {}),",
            shape.runtime_class,
            shape.runtime_member,
            shape.runtime_descriptor,
            shape.is_static,
            shape.java_method,
            shape.hook_descriptor(),
            shape.refuses
        );
    }

    let _ = writeln!(
        text,
        "    }};\n\n\
         \x20   private Hook() {{\n\
         \x20   }}\n\n\
         \x20   /** Forces this class to initialize, and with it the library load. */\n\
         \x20   public static void initialize() {{\n\
         \x20   }}\n"
    );

    for shape in shapes {
        let declared = shape
            .parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| format!("{} argument{index}", parameter.java()))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            text,
            "    /** {} */\n\
             \x20   public static native {} {}({declared});\n",
            shape.capability,
            shape.java_returns(),
            shape.java_method
        );
    }

    // Written rather than generated, like the count below them: these belong to
    // the bootstrap rather than to any version's hook surface. They are declared
    // here because Ravel has one exported symbol surface and it is this class —
    // a screen with natives of its own would be a second one.
    let _ = writeln!(text, "{}", written_declarations());
    text
}

/// Renders the game names the agent jar's hand-written adapters resolve.
pub(super) fn game_names(adapter: &Adapter, surface: &Reflection) -> String {
    let mut text = String::with_capacity(surface.classes.len() * 160 + 1024);
    let _ = writeln!(
        text,
        "// Generated from the pinned Minecraft {} inputs. Do not edit.\n\
         package dev.ravel.jvm;\n\n\
         /**\n\
         \x20* Game classes the hand-written adapters resolve by name.\n\
         \x20*\n\
         \x20* <p>Every value is the name the JVM links, and every comment is the\n\
         \x20* Mojang name the reviewed surface declared it under. The two are the\n\
         \x20* same under an unobfuscated version and differ under an obfuscated one,\n\
         \x20* which is why no source spells either of them by hand.\n\
         \x20*/\n\
         final class GameNames {{\n",
        adapter.minecraft
    );
    for reflected in &surface.classes {
        let _ = writeln!(
            text,
            "    /** {}. */\n\
             \x20   static final String {} = \"{}\";\n",
            reflected.class, reflected.name, reflected.runtime
        );
    }
    let _ = writeln!(
        text,
        "    private GameNames() {{\n\
         \x20   }}\n\
         }}"
    );
    text
}

/// Renders the linked members the agent jar's hand-written adapters resolve.
///
/// Values pair linked names and descriptors with `:`; argument-count lookups emit no constant.
///
/// # Errors
///
/// Returns an error for invalid lookups, unresolved members, or duplicate constants.
pub(super) fn game_members(
    adapter: &Adapter,
    surface: &Reflection,
    mapping: &Mapping,
) -> Result<String, GenerateError> {
    let mut text = String::with_capacity(surface.classes.len() * 200 + 1024);
    let _ = writeln!(
        text,
        "// Generated from the pinned Minecraft {} inputs. Do not edit.\n\
         package dev.ravel.jvm;\n\n\
         /**\n\
         \x20* Members of the game classes the hand-written adapters resolve by name.\n\
         \x20*\n\
         \x20* <p>Every value is the linked name and the linked descriptor the JVM\n\
         \x20* declares, separated by a colon, and every comment is the reviewed\n\
         \x20* member the surface declared it under. The descriptor is carried\n\
         \x20* because a name alone does not pick an overload, and under an\n\
         \x20* obfuscated version it does not even pick a method.\n\
         \x20*/\n\
         final class GameMembers {{\n",
        adapter.minecraft
    );
    let mut written = BTreeSet::new();
    for reflected in &surface.classes {
        let owner = reflected.class.replace('.', "/");
        for member in &reflected.members {
            let lookup = member.lookup().map_err(|detail| {
                GenerateError::Drift(format!(
                    "the reflective surface declares a member of {}: {detail}",
                    reflected.class
                ))
            })?;
            let Lookup::Descriptor(descriptor) = lookup else {
                continue;
            };
            let (name, linked) = mapping.runtime_member(&owner, &member.name, descriptor)?;
            let constant = member_constant(&reflected.name, &member.name);
            if !written.insert(constant.clone()) {
                return Err(GenerateError::Drift(format!(
                    "{constant} would be written down twice, so one of the two members it names \
                     would be unreachable"
                )));
            }
            let _ = writeln!(
                text,
                "    /** {}#{}. */\n\
                 \x20   static final String {constant} = \"{name}:{linked}\";\n",
                reflected.class, member.name
            );
        }
    }
    let _ = writeln!(
        text,
        "    private GameMembers() {{\n\
         \x20   }}\n\
         }}"
    );
    Ok(text)
}

/// The constant one reviewed member of one class role is written down under.
fn member_constant(role: &str, member: &str) -> String {
    if member == "<init>" {
        return format!("{role}_NEW");
    }
    let mut text = String::with_capacity(role.len() + member.len() + 8);
    text.push_str(role);
    text.push('_');
    let mut previous = '_';
    for character in member.chars() {
        if character.is_ascii_uppercase() && previous.is_ascii_lowercase() {
            text.push('_');
        }
        text.push(character.to_ascii_uppercase());
        previous = character;
    }
    text
}

/// Renders the title-menu placement this version's column cannot supply.
///
/// Void-returning seams supply no row, so affected title menus need explicit placement.
pub(super) fn menu_layout(adapter: &Adapter, inventory: &Inventory) -> String {
    let row = inventory
        .hooks
        .iter()
        .find(|hook| hook.id == inputs::TITLE_MENU_SEAM)
        .and_then(|hook| hook.title_menu_row);
    let mut text = String::with_capacity(1024);
    let _ = writeln!(
        text,
        "// Generated from the pinned Minecraft {} inputs. Do not edit.\n\
         package dev.ravel.jvm;\n\n\
         /**\n\
         \x20* Where Ravel's own row goes on this version's title menu.\n\
         \x20*\n\
         \x20* <p>Measured down from the top the column builder is handed. It is\n\
         \x20* declared rather than derived because a column builder that returns\n\
         \x20* nothing says nothing about what is free below it, and a title screen\n\
         \x20* that places its own lower row at a constant reserves nothing when a\n\
         \x20* row is added to the column.\n\
         \x20*\n\
         \x20* <p>{} is what this version declares.\n\
         \x20*/\n\
         final class MenuLayout {{\n\n\
         \x20   /**\n\
         \x20    * What a version whose column returns its own row declares.\n\
         \x20    *\n\
         \x20    * <p>Out of the range of any row, because a row may be negative: a\n\
         \x20    * version whose menu has no space below the column has Ravel's row\n\
         \x20    * above it, and an ordinary sentinel would be a legal offset.\n\
         \x20    */\n\
         \x20   static final int DERIVED = Integer.MIN_VALUE;\n\n\
         \x20   /**\n\
         \x20    * The row, or DERIVED where this version's column hands one back.\n\
         \x20    */\n\
         \x20   static final int RAVEL_ROW = {};\n\n\
         \x20   private MenuLayout() {{\n\
         \x20   }}\n\
         }}",
        adapter.minecraft,
        row.map_or_else(
            || "No row, because this version's column hands one back".to_owned(),
            |row| format!("A row {row} pixels below the column top")
        ),
        row.map_or_else(|| "DERIVED".to_owned(), |row| row.to_string())
    );
    text
}

/// Renders the screen class names the agent jar loads reflectively.
pub(super) fn screen_names(adapter: &Adapter, names: &[(String, String)]) -> String {
    let mut text = String::with_capacity(names.len() * 120 + 768);
    let _ = writeln!(
        text,
        "// Generated from the pinned Minecraft {} inputs. Do not edit.\n\
         package dev.ravel.jvm;\n\n\
         /**\n\
         \x20* Ravel's own screen classes, as the version they were generated for\n\
         \x20* names them.\n\
         \x20*\n\
         \x20* <p>A screen extends a game class, so it is generated rather than\n\
         \x20* written and it is emitted beside the names that version links. Under a\n\
         \x20* version whose game is in the unnamed package the screens are there too,\n\
         \x20* which is why no source spells one of these by hand.\n\
         \x20*/\n\
         final class ScreenNames {{\n",
        adapter.minecraft
    );
    for (constant, binary) in names {
        let simple = binary.rsplit('.').next().unwrap_or(binary);
        let _ = writeln!(
            text,
            "    /** Ravel's {simple}, where this version's names put it. */\n\
             \x20   static final String {constant} = \"{binary}\";\n"
        );
    }
    let _ = writeln!(
        text,
        "    private ScreenNames() {{\n\
         \x20   }}\n\
         }}"
    );
    text
}

/// The declarations that belong to the bootstrap rather than to a hook surface.
fn written_declarations() -> &'static str {
    "    /**\n\
         \x20    * Constructs, configures, drives, and installs the composed mod set.\n\
         \x20    *\n\
         \x20    * <p>Called once, from the moment the transformer sees the first\n\
         \x20    * distribution entry point, which is where the running side first\n\
         \x20    * exists as a fact. A premain class-path probe cannot answer it: the\n\
         \x20    * published server jar is a bundler and its real jars are not on any\n\
         \x20    * loader yet.\n\
         \x20    *\n\
         \x20    * @param side the code {@code dev.ravel.jvm.Agent} named the side by\n\
         \x20    */\n\
         \x20   public static native void ravelInitialize(int side);\n\n\
         \x20   /** Hook invocations the native side has counted. */\n\
         \x20   public static native long nativeInvocations();\n\n\
         \x20   /**\n\
         \x20    * One mod's settings, as the screen draws them.\n\
         \x20    *\n\
         \x20    * <p>There are two batched crossings per settings session and this is\n\
         \x20    * the first: everything a screen needs to build its widgets arrives in\n\
         \x20    * one document, so nothing crosses per widget and nothing per frame.\n\
         \x20    *\n\
         \x20    * <p>Both payloads are length-prefixed fields rather than JSON. A\n\
         \x20    * setting's title, description and value are free-form text, so a\n\
         \x20    * delimited format would need escaping and the screen has no JSON\n\
         \x20    * parser it may borrow; a length says where a field ends whatever is\n\
         \x20    * inside it.\n\
         \x20    *\n\
         \x20    * @param owner the canonical mod id whose settings to read\n\
         \x20    * @return the settings document, or {@code null} when this process\n\
         \x20    *     holds no configuration for that owner\n\
         \x20    */\n\
         \x20   public static native String ravelSettingsDocument(String owner);\n\n\
         \x20   /**\n\
         \x20    * Records what the player changed, and answers what was refused.\n\
         \x20    *\n\
         \x20    * <p>The second of the two crossings. A value the owning mod refuses\n\
         \x20    * leaves the previous one in place, and the answer says which keys\n\
         \x20    * those were and why, so the screen can tell the player rather than\n\
         \x20    * letting an edit vanish between one open and the next.\n\
         \x20    *\n\
         \x20    * @param payload the player's values for one owner\n\
         \x20    * @return the refused keys and their reasons, empty when every edit\n\
         \x20    *     landed\n\
         \x20    */\n\
         \x20   public static native String ravelSaveSettings(String payload);\n\n\
         \x20   /** Settings document and save crossings in this process. */\n\
         \x20   public static native String ravelSettingsCrossings();\n\n\
         \x20   /**\n\
         \x20    * The installed mod set, in resolved load order.\n\
         \x20    *\n\
         \x20    * <p>The mod list uses the settings screen's mechanism,\n\
         \x20    * so it is one batched read on the same framing: the whole list and\n\
         \x20    * every failure's attribution arrive together, and nothing crosses\n\
         \x20    * per row or per frame.\n\
         \x20    *\n\
         \x20    * <p>A failure crosses as attribution rather than as a sentence —\n\
         \x20    * kind, surface, dependency chain, the state it failed in — and the\n\
         \x20    * screen composes what it shows from those. The detail text has\n\
         \x20    * already been through the loader's redaction policy, which is what\n\
         \x20    * makes it something a player may be shown at all.\n\
         \x20    *\n\
         \x20    * @return the mod list, or {@code null} when no set is installed\n\
         \x20    */\n\
         \x20   public static native String ravelModSurface();\n\n\
         \x20   /**\n\
         \x20    * Records whether this launch asked for opt-in profiling.\n\
         \x20    *\n\
         \x20    * <p>Called once from {@code premain}, before any mod exists.\n\
         \x20    * The property is read here because Java is where a system\n\
         \x20    * property is. The crossing runs from Java to Rust,\n\
         \x20    * not the other way, so this is the direction the switch can\n\
         \x20    * travel in. Reading it once is what lets the collectors hold a\n\
         \x20    * plain flag instead of an atomic on every crossing.\n\
         \x20    *\n\
         \x20    * @param value {@code System.getProperty(\"ravel.profiling\")},\n\
         \x20    *     or {@code null} when the launch set nothing\n\
         \x20    */\n\
         \x20   public static native void ravelEnableProfiling(String value);\n\n\
         \x20   /**\n\
         \x20    * What opt-in profiling collected, and what it could not.\n\
         \x20    *\n\
         \x20    * <p>The third read on this framing, and the same mechanism as\n\
         \x20    * the other two: one batched read, nothing per row and nothing\n\
         \x20    * per frame.\n\
         \x20    *\n\
         \x20    * <p>The numbers cross with the sentences that qualify them, and\n\
         \x20    * every metric crosses rather than only the measured ones. A\n\
         \x20    * metric a collector cannot measure and one that measured zero\n\
         \x20    * look identical if the first is simply left out.\n\
         \x20    *\n\
         \x20    * @return the report, or {@code null} when no set is installed\n\
         \x20    */\n\
         \x20   public static native String ravelProfilingReport();\n\n\
         \x20   /**\n\
         \x20    * Admits or refuses one world's Ravel aggregate.\n\
         \x20    *\n\
         \x20    * <p>The world's open crossing. The aggregate is bytes Java never reads:\n\
         \x20    * it is handed over as it was stored and every question about what\n\
         \x20    * is inside it is answered natively, which is what keeps admission\n\
         \x20    * one authority rather than two.\n\
         \x20    *\n\
         \x20    * <p>A null array is a world holding no Ravel data. A zero-length\n\
         \x20    * one is not: it is data that is present and unreadable, and it\n\
         \x20    * refuses, because treating it as absence would let a truncated file\n\
         \x20    * silently reset the player's world.\n\
         \x20    *\n\
         \x20    * <p>The answer is empty when the world is admitted and carries the\n\
         \x20    * player-facing diagnostic when it is refused. Null is neither: it is\n\
         \x20    * what the JVM receives when the native failed, and it refuses, so a\n\
         \x20    * panic stops a world load rather than admitting one unchecked.\n\
         \x20    *\n\
         \x20    * @param aggregate the stored aggregate, or {@code null} when the\n\
         \x20    *     world holds none\n\
         \x20    * @param identity what the game calls this world, unmassaged\n\
         \x20    * @param freshWorld the game's advisory marker for a world being\n\
         \x20    *     created, which is diagnostic only and never decides admission\n\
         \x20    * @return empty when admitted, the diagnostic when refused\n\
         \x20    */\n\
         \x20   public static native String ravelOpenWorldPersistence(\n\
         \x20           byte[] aggregate, String identity, boolean freshWorld);\n\n\
         \x20   /**\n\
         \x20    * Writes the mods' data pack into a world before its packs are read.\n\
         \x20    *\n\
         \x20    * <p>A world's packs are chosen from a repository built over its\n\
         \x20    * `datapacks` directory before the open crossing above is reached,\n\
         \x20    * so a pack written there is read on the world's next open and the\n\
         \x20    * session that wrote it has none of the mods' tags, recipes or loot\n\
         \x20    * tables. This crossing is the earlier seam that closes it, and it\n\
         \x20    * carries the directory rather than a handle because the native side\n\
         \x20    * writes files.\n\
         \x20    *\n\
         \x20    * <p>Writing twice is the intended shape rather than a cost: the\n\
         \x20    * open crossing still writes, and both write the same tree from the\n\
         \x20    * same installed set.\n\
         \x20    *\n\
         \x20    * @param world the world's own directory, as the storage access\n\
         \x20    *     spells it\n\
         \x20    */\n\
         \x20   public static native void ravelWriteWorldDatapack(String world);\n\n\
         \x20   /**\n\
         \x20    * Writes the mods' data pack where the create-world screen stages\n\
         \x20    * packs.\n\
         \x20    *\n\
         \x20    * <p>Being selected and being loaded are different things. A world\n\
         \x20    * being created runs no data load of its own; it is handed the\n\
         \x20    * resources the screen loaded when it opened, so a pack that only\n\
         \x20    * reaches the world's directory is listed and unread for the whole\n\
         \x20    * first session. The screen stages packs in a directory of its own\n\
         \x20    * and copies that directory's contents into the world it makes, so\n\
         \x20    * the pack goes in there and the screen is asked to load again.\n\
         \x20    *\n\
         \x20    * @param directory the screen's staging directory, which holds pack\n\
         \x20    *     directories rather than a {@code datapacks} directory\n\
         \x20    * @return {@code 0} when the screen has no pack to load, which is\n\
         \x20    *     what no mod supplying data and a refused write both answer;\n\
         \x20    *     {@code 1} when the pack is there and holds what it already\n\
         \x20    *     held; {@code 2} when its files are not the ones that were\n\
         \x20    */\n\
         \x20   public static native int ravelWriteStagedDatapack(String directory);\n\n\
         \x20   /**\n\
         \x20    * The id the game will give the pack Ravel writes.\n\
         \x20    *\n\
         \x20    * <p>The game composes it from the name of the directory it found\n\
         \x20    * the pack in, and that name is the native side's. Asking for it\n\
         \x20    * is what keeps one launch's pack one id: the same directory is\n\
         \x20    * what a player's enabled list names and what a world being\n\
         \x20    * created has to select.\n\
         \x20    *\n\
         \x20    * @return the id, which is the same for every crossing that names\n\
         \x20    *     the pack\n\
         \x20    */\n\
         \x20   public static native String ravelPackId();\n\n\
         \x20   /**\n\
         \x20    * Offers the aggregate the saving world should now store.\n\
         \x20    *\n\
         \x20    * <p>The save crossing has the opposite failure priority to the\n\
         \x20    * open crossing: element zero tags the answer, the remainder is its\n\
         \x20    * payload, and null means the native failed and nothing is written.\n\
         \x20    * Losing one Ravel checkpoint is the safe side here, because writing\n\
         \x20    * a guess would not be.\n\
         \x20    *\n\
         \x20    * <p>{@code 0x00} unchanged, and the array is exactly one byte.\n\
         \x20    * {@code 0x01} replacement, and the remainder is the aggregate to\n\
         \x20    * store. {@code 0x02} failed and {@code 0x03} identity mismatch,\n\
         \x20    * and the remainder is a UTF-8 diagnostic; the two stay distinct\n\
         \x20    * because only the second says the lifecycle is wrong rather than a\n\
         \x20    * mod. Only {@code 0x01} writes anything.\n\
         \x20    *\n\
         \x20    * @param identity what the game calls the saving world, unmassaged\n\
         \x20    * @return the tagged answer, or {@code null} when the native failed\n\
         \x20    */\n\
         \x20   public static native byte[] ravelSaveWorldPersistence(String identity);\n\n\
         \x20   /**\n\
         \x20    * Everything Ravel's registration barrier admitted, to register.\n\
         \x20    *\n\
         \x20    * <p>The barrier decides that a mod owns an identifier and a\n\
         \x20    * registry in the running game is where that becomes a block, so\n\
         \x20    * this is the read the content adapter performs at the last point\n\
         \x20    * the built-in registries still accept entries. A hook could not\n\
         \x20    * do it: hooks only observe or veto, and registering is an action.\n\
         \x20    *\n\
         \x20    * <p>The payload carries only the kinds a registry can take,\n\
         \x20    * because which claims are registrable is an admission question\n\
         \x20    * and admission is native. Empty is a launch with no set\n\
         \x20    * installed. Null is neither: it is what the JVM receives when\n\
         \x20    * the native failed, and nothing is registered on it.\n\
         \x20    *\n\
         \x20    * @return the admitted content, empty before a set is installed\n\
         \x20    */\n\
         \x20   public static native String ravelAdmittedContent();\n\n\
         \x20   /**\n\
         \x20    * Every action a mod asked for since this was last called.\n\
         \x20    *\n\
         \x20    * <p>The other direction of the same rule\n\
         \x20    * {@code ravelAdmittedContent} carries: a hook only observes or\n\
         \x20    * vetoes, so an action a mod asks for crosses as an instruction\n\
         \x20    * and the Java adapter performs it. Every outbound action uses\n\
         \x20    * one mechanism, generic over the verb rather than written per verb.\n\
         \x20    *\n\
         \x20    * <p>Draining is what this does: an action is answered once and\n\
         \x20    * never again. Empty is a tick nobody asked for anything on.\n\
         \x20    * Null is what the JVM receives when the native failed, and\n\
         \x20    * nothing is performed on it.\n\
         \x20    *\n\
         \x20    * @return the queued actions, empty when there are none\n\
         \x20    */\n\
         \x20   public static native String ravelOutwardActions();\n\
         }"
}

/// Renders the stub provenance lock.
pub(super) fn stub_lock(adapter: &Adapter, stubs: &[Stub]) -> String {
    let mut text = String::with_capacity(stubs.len() * 256 + 1536);
    let _ = writeln!(
        text,
        "# Generated stub provenance for Minecraft {}. Do not edit.\n\
         #\n\
         # Every entry below is one compile-time stub the settings screen is built\n\
         # against, read out of the pinned jars: the client jar first, then the\n\
         # serialization library the game's own signatures name. Descriptors and\n\
         # access flags are the jar's own, which is the point: a hand-written stub\n\
         # transcribes a generic signature, and the JVM links on the erased one.\n\
         #\n\
         # What this file does not promise:\n\
         #\n\
         # - It is checked only where the pinned jars are. A machine without them\n\
         #   generates from this file, so on that machine the lock is the source and\n\
         #   cannot disagree with anything. Drift is caught where the jars exist.\n\
         # - It pins against jar hashes, so it is only as current as those pins. A\n\
         #   Minecraft build nobody has re-pinned against is unchecked.\n\
         # - It carries no hierarchy the stub set does not already hold. A supertype\n\
         #   outside the set is dropped, so a change to one is invisible here.\n\
         # - It covers the members `stubs.toml` seeds and nothing else. A member the\n\
         #   screen starts calling is unpinned until it is seeded.\n\
         #\n\
         # This is a different table from mappings.lock and stays one. That file\n\
         # pins call sites the agent rewrites; this one pins declarations the screen\n\
         # compiles against, and one table meaning both is how the next person\n\
         # misreads it.\n\
         \n\
         schemaVersion = {}\n\
         minecraft = \"{}\"\n\
         extractor = \"{EXTRACTOR}\"\n\
         clientJarSha256 = \"{}\"\n\
         dataFixerJarSha256 = \"{}\"",
        adapter.minecraft,
        adapter.schema_version,
        adapter.minecraft,
        adapter.inputs.client_jar.sha256.as_deref().unwrap_or(""),
        adapter
            .inputs
            .data_fixer_jar
            .sha256
            .as_deref()
            .unwrap_or("")
    );

    for stub in stubs {
        let _ = writeln!(
            text,
            "\n[[stub]]\nclass = \"{}\"\nflags = {}",
            stub.class, stub.flags
        );
        if let Some(parent) = &stub.extends {
            let _ = writeln!(text, "extends = \"{parent}\"");
        }
        if let Some(chained) = &stub.super_constructor {
            let _ = writeln!(text, "superConstructor = \"{chained}\"");
        }
        if !stub.implements.is_empty() {
            let rendered = stub
                .implements
                .iter()
                .map(|name| format!("\"{name}\""))
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(text, "implements = [{rendered}]");
        }
        for member in &stub.members {
            let _ = writeln!(
                text,
                "\n[[stub.member]]\nname = \"{}\"\ndescriptor = \"{}\"\nflags = {}\nmethod = {}",
                member.name, member.descriptor, member.flags, member.method
            );
            if let Some(element) = &member.returns {
                let _ = writeln!(text, "returns = \"{element}\"");
            }
        }
    }
    text
}

/// Splits an internal class name into its package and the name Java declares.
///
/// A nested class keeps its `$`: the declared name is the binary one, which is
/// what the JVM links against and what javac writes into the class file. A class
/// with no package at all is the unnamed one, which an obfuscated version puts
/// most of the game in.
pub(super) fn split(class: &str) -> (Option<String>, &str) {
    class
        .rsplit_once('/')
        .map_or((None, class), |(package, simple)| {
            (Some(package.replace('/', ".")), simple)
        })
}

/// Whether these linked names span the unnamed package and named ones at once.
pub(super) fn name_space_is_split<'a>(classes: impl IntoIterator<Item = &'a str>) -> bool {
    let (mut named, mut unnamed) = (false, false);
    for class in classes {
        if split(class).0.is_none() {
            unnamed = true;
        } else {
            named = true;
        }
    }
    named && unnamed
}

/// The internal name a source declares and names one linked class by.
///
/// Java source inside a package cannot name a class outside every package — the
/// language has no syntax for it — so a version that splits its game across the
/// two has no source form for the packaged half at all. Where it is split, every
/// class is written in the unnamed package instead, which is the only name space
/// both halves can be spelled in and leaves javac one name per class. The
/// package is put back into the compiled class files afterwards, so what the JVM
/// links is still the linked name.
///
/// The alias runs this way round rather than lifting the unnamed half into a
/// package of Ravel's own because the packaged half is the small one: an
/// obfuscated version leaves a handful of classes named and moves the rest out
/// of every package, so writing them all as the majority already is renames the
/// fewest classes and leaves the harness's compiled names the ones the adapter
/// resolves for all but those few.
pub(super) fn alias(class: &str, split_name_space: bool) -> String {
    if split_name_space {
        split(class).1.to_owned()
    } else {
        class.to_owned()
    }
}

/// An internal class name as Java source writes it.
pub(super) fn source_name(class: &str) -> String {
    class.replace('/', ".")
}

/// Reads a method descriptor as the Java source types it declares.
pub(super) fn signature(descriptor: &str) -> Result<(Vec<String>, String), String> {
    let mut rest = descriptor
        .strip_prefix('(')
        .ok_or_else(|| format!("{descriptor} is not a method descriptor"))?;
    let mut parameters = Vec::new();
    while !rest.starts_with(')') {
        let (kind, remainder) = java_type(rest)?;
        parameters.push(kind);
        rest = remainder;
    }
    let (returns, after) = java_type(&rest[1..])?;
    if !after.is_empty() {
        return Err(format!("{descriptor} declares more than one return type"));
    }
    Ok((parameters, returns))
}

/// Reads one type descriptor as Java source, returning it and whatever follows.
pub(super) fn java_type(descriptor: &str) -> Result<(String, &str), String> {
    let first = descriptor
        .chars()
        .next()
        .ok_or_else(|| format!("{descriptor} ends before a type does"))?;
    let primitive = match first {
        'V' => Some("void"),
        'Z' => Some("boolean"),
        'B' => Some("byte"),
        'C' => Some("char"),
        'S' => Some("short"),
        'I' => Some("int"),
        'J' => Some("long"),
        'F' => Some("float"),
        'D' => Some("double"),
        _ => None,
    };
    if let Some(kind) = primitive {
        return Ok((kind.to_owned(), &descriptor[1..]));
    }
    match first {
        'L' => {
            let end = descriptor
                .find(';')
                .ok_or_else(|| format!("{descriptor} names a class that is never terminated"))?;
            let name = &descriptor[1..end];
            if name.is_empty() {
                return Err(format!("{descriptor} names no class between L and ;"));
            }
            Ok((source_name(name), &descriptor[end + 1..]))
        }
        '[' => {
            let (element, after) = java_type(&descriptor[1..])?;
            Ok((format!("{element}[]"), after))
        }
        other => Err(format!("{other} is not a type descriptor")),
    }
}

#[cfg(test)]
mod tests {
    use super::super::inputs::{
        Adapter, Hook, Input, Inputs, Inventory, Kind, MappingSource, PackFormat, Reflected,
        Reflection, Spelling,
    };
    use super::super::resolve::Resolved;
    use super::super::stub_render::stub_java;
    use super::super::stubs::{Member as StubMember, Stub};
    use super::{
        JavaType, alias, bindings, constant_name, exports, game_names, hook_java, java_name,
        name_space_is_split, parameters, rust_name, shapes,
    };
    use super::{Mapping, java_type, signature, stub_lock};

    /// Adapter inputs standing in for the tracked ones.
    ///
    /// Rendering needs a version string and nothing else from them, and building
    /// one here is what keeps a candidate hook out of `hooks.toml`: growing the
    /// reviewed inventory is a separate, deliberate step that re-pins a hash and
    /// regenerates the lock, and proving the generator can carry a shape is not.
    fn adapter() -> Adapter {
        let unpinned = || Input {
            path: String::new(),
            sha256: None,
            bytes: None,
            upstream: None,
        };
        Adapter {
            schema_version: 1,
            minecraft: "0.0.0-test".to_owned(),
            mapping_source: MappingSource::UnobfuscatedClassFiles,
            java_release: 25,
            pack_format: PackFormat {
                resource: 7,
                resource_minor: 0,
                data: 11,
                data_minor: 0,
                spelling: Spelling::PackFormat,
            },
            inputs: Inputs {
                client_jar: unpinned(),
                client_mappings: None,
                data_fixer_jar: unpinned(),
                version_manifest: unpinned(),
                hook_inventory: unpinned(),
                stub_seeds: unpinned(),
                reflective_surface: unpinned(),
                mappings_lock: unpinned(),
                stubs_lock: unpinned(),
            },
        }
    }

    /// The alias moves a class only where the two name spaces meet, and moves
    /// it to the one the other half is already in.
    #[test]
    fn a_split_name_space_writes_every_class_where_the_unnamed_half_already_is() {
        assert!(name_space_is_split(["a", "com/example/Held"]));
        assert!(!name_space_is_split(["a", "b"]));
        assert!(!name_space_is_split([
            "com/example/Held",
            "com/example/Kept"
        ]));

        assert_eq!(alias("com/example/Held", true), "Held");
        assert_eq!(alias("a", true), "a");
        assert_eq!(alias("com/example/Held", false), "com/example/Held");
    }

    /// What the agent resolves is the name the version links, whatever name the
    /// stand-in beside it was written under.
    ///
    /// The two are one string under every version the harness can run, and a
    /// generator that aliased this one would make the harness agree with itself
    /// and with nothing the game loads.
    #[test]
    fn the_names_the_adapters_resolve_are_the_linked_ones_under_a_split_name_space() {
        let surface = Reflection {
            schema_version: 1,
            minecraft: String::from("0.0.0"),
            classes: vec![
                Reflected {
                    name: String::from("SERVER"),
                    class: String::from("com.example.server.Held"),
                    runtime: String::from("com/example/server/Held"),
                    members: Vec::new(),
                },
                Reflected {
                    name: String::from("SURFACE"),
                    class: String::from("com.example.gui.Panel"),
                    runtime: String::from("a"),
                    members: Vec::new(),
                },
            ],
        };

        let java = game_names(&adapter(), &surface);

        assert!(
            java.contains("SERVER = \"com/example/server/Held\";"),
            "{java}"
        );
        assert!(java.contains("SURFACE = \"a\";"), "{java}");
    }

    /// An inventory and its resolution, from entries written the same way the
    /// tracked ones are.
    fn reviewed(entries: &[(&str, &str, &str, &str)]) -> (Inventory, Vec<Resolved>) {
        let hooks = entries
            .iter()
            .map(|(id, class, member, descriptor)| Hook {
                id: (*id).to_owned(),
                class: (*class).to_owned(),
                member: (*member).to_owned(),
                descriptor: (*descriptor).to_owned(),
                kind: Kind::InstanceMethod,
                capability: format!("A candidate {id} hook."),
                refuses: false,
                title_menu_row: None,
            })
            .collect::<Vec<_>>();
        let resolved = hooks
            .iter()
            .map(|hook| Resolved {
                id: hook.id.clone(),
                runtime_class: hook.class.replace('.', "/"),
                mojang_class: hook.class.replace('.', "/"),
                runtime_member: hook.member.clone(),
                member: hook.member.clone(),
                runtime_descriptor: hook.descriptor.clone(),
                descriptor: hook.descriptor.clone(),
                is_static: false,
            })
            .collect();
        (
            Inventory {
                schema_version: 1,
                minecraft: "0.0.0-test".to_owned(),
                hooks,
                non_goals: Vec::new(),
            },
            resolved,
        )
    }

    #[test]
    fn a_primitive_descriptor_reads_back_as_the_arguments_it_declares() {
        assert_eq!(parameters("(I)Z").expect("valid"), vec![JavaType::Int]);
        assert_eq!(
            parameters("(DD)D").expect("valid"),
            vec![JavaType::Double, JavaType::Double]
        );
        assert_eq!(parameters("()V").expect("valid"), Vec::new());
        assert_eq!(
            parameters("(ZBCSIJFD)V").expect("valid"),
            vec![
                JavaType::Boolean,
                JavaType::Byte,
                JavaType::Char,
                JavaType::Short,
                JavaType::Int,
                JavaType::Long,
                JavaType::Float,
                JavaType::Double,
            ]
        );
    }

    #[test]
    fn a_string_is_copied_and_every_other_reference_becomes_a_handle() {
        assert_eq!(
            parameters("(Ljava/lang/String;Z)Ljava/lang/String;").expect("valid"),
            vec![JavaType::Text, JavaType::Boolean]
        );
        assert_eq!(
            parameters("(Lcom/example/resources/ResourceKey;Ljava/lang/Object;)V")
                .expect("a reference to an object the game owns crosses as a handle"),
            vec![
                JavaType::Handle("com/example/resources/ResourceKey".to_owned()),
                JavaType::Handle("java/lang/Object".to_owned()),
            ]
        );
    }

    #[test]
    fn an_array_is_an_object_and_crosses_as_a_handle_naming_its_own_descriptor() {
        assert_eq!(
            parameters("([J[[ILjava/lang/String;)V").expect("valid"),
            vec![
                JavaType::Handle("[J".to_owned()),
                JavaType::Handle("[[I".to_owned()),
                JavaType::Text,
            ]
        );
        assert_eq!(
            parameters("([Ljava/lang/String;)V").expect("valid"),
            vec![JavaType::Handle("[Ljava/lang/String;".to_owned())],
            "an array of strings is an array, and an array is never copied"
        );
    }

    #[test]
    fn a_byte_array_crosses_as_bytes_and_no_other_array_does() {
        assert_eq!(
            parameters("([BI)V").expect("valid"),
            vec![JavaType::Bytes, JavaType::Int],
            "the saved-data aggregate is a byte payload rather than a borrowed object"
        );
        assert_eq!(
            parameters("([[B[Ljava/lang/Byte;)V").expect("valid"),
            vec![
                JavaType::Handle("[[B".to_owned()),
                JavaType::Handle("[Ljava/lang/Byte;".to_owned()),
            ],
            "only `[B` is the payload; an array of them is an array like any other"
        );
        assert_eq!(JavaType::Bytes.descriptor(), "[B");
        assert_eq!(JavaType::Bytes.java(), "byte[]");
        assert_eq!(JavaType::Bytes.rust(), "::jni::objects::JByteArray<'local>");
    }

    #[test]
    fn a_descriptor_that_is_not_one_is_refused_and_says_what_about_it_is_wrong() {
        assert!(
            parameters("(Lcom/example/core/Registry)V")
                .expect_err("an unterminated class name must be refused")
                .contains("never terminated")
        );
        assert!(
            parameters("([)V")
                .expect_err("an array of nothing must be refused")
                .contains("empty")
        );
        assert!(
            parameters("(L;)V")
                .expect_err("a class descriptor naming no class must be refused")
                .contains("names no class")
        );
        assert!(
            parameters("(Q)V")
                .expect_err("an unknown descriptor must be refused")
                .contains("not a parameter descriptor")
        );
        assert!(
            parameters("(V)V")
                .expect_err("void is not a parameter")
                .contains("not a parameter descriptor")
        );
        assert!(
            parameters("I)V")
                .expect_err("this is not a method descriptor")
                .contains("not a method descriptor")
        );
    }

    #[test]
    fn every_admitted_type_round_trips_through_the_hook_signature_it_renders() {
        for parameter in [
            JavaType::Boolean,
            JavaType::Byte,
            JavaType::Char,
            JavaType::Short,
            JavaType::Int,
            JavaType::Long,
            JavaType::Float,
            JavaType::Double,
            JavaType::Text,
            JavaType::Bytes,
            JavaType::Handle("java/lang/Object".to_owned()),
        ] {
            let descriptor = format!("({}){}", parameter.descriptor(), parameter.descriptor());

            assert_eq!(
                parameters(&descriptor).expect("valid"),
                vec![parameter.clone()]
            );
            assert_ne!(parameter.java(), "");
            assert!(parameter.rust().starts_with("::jni::"));
            assert!(parameter.value("x").contains("HookValue::"));
        }
    }

    #[test]
    fn a_handle_keeps_its_declared_class_in_rust_and_loses_it_in_the_hook_signature() {
        let registry = JavaType::Handle("com/example/core/MappedRegistry".to_owned());

        assert_eq!(registry.descriptor(), "Ljava/lang/Object;");
        assert_eq!(registry.java(), "Object");
        assert_eq!(registry.rust(), "::jni::objects::JObject<'local>");
        assert_eq!(
            registry.value("argument0"),
            "::ravel_minecraft::HookValue::Handle(::ravel_minecraft::JavaHandle::new(\
             \"com/example/core/MappedRegistry\", &argument0))"
        );
    }

    /// The real M3 targets this enables, plus one array shape the resolved set
    /// has no parameter for yet.
    ///
    /// `MappedRegistry.register` is the entry that decided ADG-4's amendment: a
    /// registry hook that cannot name what is being registered is not a hook.
    const CANDIDATES: [(&str, &str, &str, &str); 3] = [
        (
            "registry.register",
            "com.example.core.MappedRegistry",
            "register",
            "(Lcom/example/resources/ResourceKey;Ljava/lang/Object;\
             Lcom/example/core/RegistrationInfo;)Lcom/example/core/Holder$Reference;",
        ),
        (
            "server.tick",
            "com.example.server.MinecraftServer",
            "tickServer",
            "(Ljava/util/function/BooleanSupplier;)V",
        ),
        (
            "server.tick-times",
            "com.example.server.MinecraftServer",
            "logTickTimes",
            "([JZ)V",
        ),
    ];

    #[test]
    fn a_candidate_carrying_objects_renders_a_borrowed_handle_per_reference() {
        let (inventory, resolved) = reviewed(&CANDIDATES);

        let shapes = shapes(&inventory, &resolved).expect("every candidate should render");
        let rust = exports(&shapes);

        assert_eq!(shapes.len(), 3);
        assert!(
            rust.contains("argument0: ::jni::objects::JObject<'local>,"),
            "{rust}"
        );
        assert!(
            rust.contains(
                "::ravel_minecraft::JavaHandle::new(\"com/example/resources/ResourceKey\", \
                 &argument0)"
            ),
            "{rust}"
        );
        assert!(
            rust.contains("::ravel_minecraft::JavaHandle::new(\"[J\", &argument0)"),
            "an array is an object and names its own descriptor: {rust}"
        );
        assert!(
            rust.contains("|_env|"),
            "no handle needs the environment, so no export asks for it: {rust}"
        );
    }

    /// The class a handle declares is what a mod reads back off it, so it comes
    /// from the reviewed descriptor even where the JVM links an obfuscated one.
    #[test]
    fn a_handle_declares_the_reviewed_class_where_the_mapping_obfuscates_it() {
        let (inventory, mut resolved) = reviewed(&CANDIDATES[..1]);
        resolved[0].runtime_class = "ezb".to_owned();
        resolved[0].runtime_member = "a".to_owned();
        resolved[0].runtime_descriptor = "(Lfhk;Ljava/lang/Object;Lezc;)Lezd;".to_owned();

        let shapes = shapes(&inventory, &resolved).expect("the candidate should render");
        let rust = exports(&shapes);
        let table = bindings(&adapter(), &inventory, &resolved);

        assert!(
            rust.contains(
                "::ravel_minecraft::JavaHandle::new(\"com/example/resources/ResourceKey\", \
                 &argument0)"
            ),
            "{rust}"
        );
        assert!(
            !rust.contains("JavaHandle::new(\"fhk\""),
            "no linked name reaches an observer: {rust}"
        );
        assert!(
            table.contains("(Lfhk;Ljava/lang/Object;Lezc;)Lezd;"),
            "the target table still links the runtime descriptor: {table}"
        );
    }

    #[test]
    fn the_generated_java_takes_object_and_names_no_game_type() {
        let (inventory, resolved) = reviewed(&CANDIDATES);

        let shapes = shapes(&inventory, &resolved).expect("every candidate should render");
        let java = hook_java(&adapter(), &shapes);

        assert!(
            java.contains(
                "public static native void registryRegister(Object argument0, \
                 Object argument1, Object argument2);"
            ),
            "{java}"
        );
        assert!(
            java.contains("\"(Ljava/lang/Object;Ljava/lang/Object;Ljava/lang/Object;)V\""),
            "the hook descriptor the agent injects erases every class: {java}"
        );
        assert!(
            java.contains("\"(Ljava/lang/Object;Z)V\""),
            "an array is one parameter and erases to Object beside the boolean: {java}"
        );
        for declaration in java.lines().filter(|line| line.contains("static native")) {
            assert!(
                !declaration.contains("minecraft"),
                "Hook.java compiles without the client jar, so no declaration names a \
                 game type: {declaration}"
            );
        }
        assert!(
            java.contains(
                "\"(Lcom/example/resources/ResourceKey;Ljava/lang/Object;\
                 Lcom/example/core/RegistrationInfo;)Lcom/example/core/Holder$Reference;\""
            ),
            "the target keeps its own descriptor, which is what the agent matches on: {java}"
        );
    }

    /// The reviewed inventory with one entry opted into refusing its target.
    fn opted_in(index: usize) -> (Inventory, Vec<Resolved>) {
        let (mut inventory, resolved) = reviewed(&CANDIDATES);
        inventory.hooks[index].refuses = true;
        (inventory, resolved)
    }

    #[test]
    fn an_opted_in_target_answers_a_boolean_and_every_other_one_still_answers_nothing() {
        // `server.tick` is the void candidate, so it is the one that may opt in.
        let (inventory, resolved) = opted_in(1);

        let shapes = shapes(&inventory, &resolved).expect("a void target may refuse");
        let java = hook_java(&adapter(), &shapes);
        let rust = exports(&shapes);

        assert!(
            java.contains("public static native boolean serverTick(Object argument0);"),
            "{java}"
        );
        assert!(
            java.contains(
                "\"serverTick\",\n                \"(Ljava/lang/Object;)Z\",\n                true)"
            ),
            "the opt-in reaches the target table beside the descriptor it changes: {java}"
        );
        assert!(
            rust.contains(") -> ::jni::sys::jboolean {\n    crate::guard::enter_refusable("),
            "{rust}"
        );

        // The other two candidates are untouched by their neighbour's opt-in.
        assert!(
            java.contains(
                "public static native void registryRegister(Object argument0, \
                 Object argument1, Object argument2);"
            ),
            "{java}"
        );
        assert!(
            java.contains("\"(Ljava/lang/Object;Z)V\",\n                false)"),
            "a target nobody opted in still answers nothing and says so: {java}"
        );
        assert_eq!(
            rust.matches("crate::guard::enter(").count(),
            2,
            "only the opted-in export takes the refusable path: {rust}"
        );
    }

    #[test]
    fn a_target_that_returns_a_value_is_refused_the_opt_in_rather_than_given_a_default() {
        // `registry.register` returns a `Holder$Reference`, so an early return
        // would have to invent one.
        let (inventory, resolved) = opted_in(0);

        let error = shapes(&inventory, &resolved)
            .expect_err("a value-returning target must not be refusable");

        let message = error.to_string();
        assert!(message.contains("registry.register"), "{message}");
        assert!(message.contains("does not return void"), "{message}");
        assert!(message.contains("invent a value"), "{message}");
    }

    /// One stub, written the way the resolver hands them over.
    fn stub(class: &str, flags: u16, members: &[(&str, &str, u16, bool)]) -> Stub {
        Stub {
            class: class.to_owned(),
            flags,
            extends: None,
            implements: Vec::new(),
            members: members
                .iter()
                .map(|(name, descriptor, flags, method)| StubMember {
                    name: (*name).to_owned(),
                    descriptor: (*descriptor).to_owned(),
                    flags: *flags,
                    method: *method,
                    returns: None,
                })
                .collect(),
            super_constructor: None,
        }
    }

    #[test]
    fn a_stub_declares_the_members_it_carries_with_the_descriptors_the_jar_declared() {
        let screen = stub(
            "com/example/client/gui/screens/Screen",
            0x0421, // public abstract class
            &[
                ("<init>", "(Lcom/example/network/chat/Component;)V", 4, true),
                ("init", "()V", 4, true),
                ("init", "(II)V", 0x0011, true),
                (
                    "getTitle",
                    "()Lcom/example/network/chat/Component;",
                    1,
                    true,
                ),
            ],
        );

        let (path, java) =
            stub_java(&adapter(), &screen, &Mapping::Identity).expect("a stub should render");

        assert_eq!(path, "com/example/client/gui/screens/Screen.java");
        assert!(
            java.contains("package com.example.client.gui.screens;"),
            "{java}"
        );
        assert!(java.contains("public abstract class Screen {"), "{java}");
        assert!(
            java.contains("protected Screen(com.example.network.chat.Component argument0) {"),
            "a constructor takes the class's simple name: {java}"
        );
        assert!(java.contains("protected void init() {"), "{java}");
        assert!(
            java.contains("public final void init(int argument0, int argument1) {"),
            "an overload is stubbed beside the one it overloads: {java}"
        );
        assert!(
            java.contains(
                "public com.example.network.chat.Component getTitle() {\n        \
                 throw new UnsupportedOperationException(\"stub\");"
            ),
            "a stub body is never reached and says so if it is: {java}"
        );
    }

    /// The defect the element type exists for: a stub generated from the erased
    /// descriptor alone declares a raw list, every element of it is an `Object`,
    /// and a caller that reads one back compiles against any type it likes.
    /// Rendering the element is what turns that into a compile error.
    #[test]
    fn a_java_util_return_is_declared_over_the_element_the_class_file_records() {
        let mut splitter = stub(
            "com/example/text/Splitter",
            0x0031,
            &[(
                "splitLines",
                "(Ljava/lang/String;)Ljava/util/List;",
                1,
                true,
            )],
        );
        splitter.members[0].returns = Some("Lcom/example/text/Line;".to_owned());

        let (_, java) = stub_java(&adapter(), &splitter, &Mapping::Identity).expect("renders");

        assert!(
            java.contains(
                "public java.util.List<com.example.text.Line> splitLines(java.lang.String \
                 argument0)"
            ),
            "{java}"
        );
    }

    /// A wildcard is the shape the pinned screens' own superclass declares, and
    /// a stub that dropped the bound would declare a list nothing assignable
    /// could be read out of.
    #[test]
    fn a_wildcard_element_keeps_the_bound_the_class_file_records() {
        let mut panel = stub(
            "com/example/gui/Panel",
            0x0031,
            &[("children", "()Ljava/util/List;", 1, true)],
        );
        panel.members[0].returns = Some("+Lcom/example/gui/Listener;".to_owned());

        let (_, java) = stub_java(&adapter(), &panel, &Mapping::Identity).expect("renders");

        assert!(
            java.contains("public java.util.List<? extends com.example.gui.Listener> children()"),
            "{java}"
        );
    }

    #[test]
    fn a_stubbed_supertype_is_declared_and_an_interface_method_gets_a_body() {
        let mut component = stub(
            "com/example/network/chat/Component",
            0x0601, // public abstract interface
            &[(
                "translatable",
                "(Ljava/lang/String;)Lcom/example/network/chat/MutableComponent;",
                0x0009,
                true,
            )],
        );
        component.implements = vec!["com/example/network/chat/FormattedText".to_owned()];
        let mut mutable = stub("com/example/network/chat/MutableComponent", 0x0031, &[]);
        mutable.implements = vec!["com/example/network/chat/Component".to_owned()];

        let (_, interface) =
            stub_java(&adapter(), &component, &Mapping::Identity).expect("renders");
        let (_, class) = stub_java(&adapter(), &mutable, &Mapping::Identity).expect("renders");

        assert!(
            interface.contains(
                "public interface Component extends com.example.network.chat.FormattedText {"
            ),
            "{interface}"
        );
        assert!(
            interface.contains(
                "static com.example.network.chat.MutableComponent \
                 translatable(java.lang.String argument0) {"
            ),
            "an interface member is never declared public, which javac warns on: {interface}"
        );
        assert!(
            class.contains(
                "public final class MutableComponent implements \
                 com.example.network.chat.Component {"
            ),
            "the edge that makes a returned value assignable is what the stub keeps: {class}"
        );
    }

    #[test]
    fn a_non_static_interface_method_is_given_a_body_rather_than_left_abstract() {
        let component = stub(
            "com/example/network/chat/Component",
            0x0601,
            &[("getString", "()Ljava/lang/String;", 0x0401, true)],
        );

        let (_, java) = stub_java(&adapter(), &component, &Mapping::Identity).expect("renders");

        assert!(
            java.contains("default java.lang.String getString() {"),
            "a stub that implements this interface must not have to implement it: {java}"
        );
    }

    #[test]
    fn a_stub_that_cannot_be_written_down_is_refused_rather_than_written_wrongly() {
        for (broken, expected) in [
            (
                stub("a/B", 0x0201, &[("VALUE", "I", 0x0019, false)]),
                "field on an interface",
            ),
            (
                stub("a/B", 1, &[("of", "(Q)V", 1, true)]),
                "not a type descriptor",
            ),
        ] {
            let error = stub_java(&adapter(), &broken, &Mapping::Identity)
                .expect_err("this must not render");
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    /// The pinned version reaches every interactive widget through a nested type,
    /// so a stub
    /// set that could not name one would compile a screen with no button on it.
    /// The stub is a top-level class carrying the binary name, because the binary
    /// name is what the JVM links the screen against at runtime.
    #[test]
    fn a_nested_class_is_declared_under_the_binary_name_the_jvm_links_on() {
        let builder = stub(
            "com/example/client/gui/components/Button$Builder",
            0x0021,
            &[(
                "bounds",
                "(IIII)Lcom/example/client/gui/components/Button$Builder;",
                1,
                true,
            )],
        );

        let (path, java) = stub_java(&adapter(), &builder, &Mapping::Identity).expect("renders");

        assert!(
            path.ends_with("com/example/client/gui/components/Button$Builder.java"),
            "{path}"
        );
        assert!(java.contains("public class Button$Builder"), "{java}");
        assert!(
            java.contains(
                "public com.example.client.gui.components.Button$Builder bounds(int argument0,"
            ),
            "a nested type is named the same way wherever it appears: {java}"
        );
    }

    #[test]
    fn a_field_stub_keeps_its_declared_type_and_drops_nothing_else() {
        let font = stub(
            "com/example/client/gui/Font",
            1,
            &[("lineHeight", "I", 1, false), ("glyphs", "[[J", 4, false)],
        );

        let (_, java) = stub_java(&adapter(), &font, &Mapping::Identity).expect("renders");

        assert!(java.contains("public int lineHeight;"), "{java}");
        assert!(java.contains("protected long[][] glyphs;"), "{java}");
    }

    #[test]
    fn the_stub_lock_records_what_was_read_and_says_what_it_does_not_promise() {
        let mut screen = stub(
            "com/example/client/gui/screens/Screen",
            0x0421,
            &[("init", "()V", 4, true)],
        );
        screen.extends = Some("com/example/A".to_owned());
        screen.implements = vec!["com/example/B".to_owned()];

        let lock = stub_lock(&adapter(), &[screen]);

        assert!(lock.contains("extractor = \"classfile\""), "{lock}");
        assert!(
            lock.contains(
                "[[stub]]\nclass = \"com/example/client/gui/screens/Screen\"\n\
                           flags = 1057\nextends = \"com/example/A\"\n\
                           implements = [\"com/example/B\"]"
            ),
            "{lock}"
        );
        assert!(
            lock.contains(
                "[[stub.member]]\nname = \"init\"\ndescriptor = \"()V\"\nflags = 4\nmethod = true"
            ),
            "{lock}"
        );
        assert!(
            lock.contains("checked only where the pinned jars are"),
            "the header has to say what the lock is worth: {lock}"
        );
        assert!(
            lock.contains("dataFixerJarSha256 = "),
            "the lock records both sources' provenance: {lock}"
        );
        assert!(
            lock.contains("different table from mappings.lock"),
            "{lock}"
        );
    }

    #[test]
    fn a_descriptor_reads_back_as_the_java_types_a_stub_writes_down() {
        assert_eq!(
            signature("(Ljava/lang/String;[I)J").expect("valid"),
            (
                vec!["java.lang.String".to_owned(), "int[]".to_owned()],
                "long".to_owned()
            )
        );
        assert_eq!(
            signature("()V").expect("valid"),
            (Vec::new(), "void".to_owned())
        );
        assert_eq!(
            java_type("[[Ljava/lang/Object;").expect("valid"),
            ("java.lang.Object[][]".to_owned(), "")
        );
        assert!(
            signature("Ljava/lang/String;")
                .expect_err("this is not a method descriptor")
                .contains("not a method descriptor")
        );
        assert!(
            signature("()VV")
                .expect_err("one return type")
                .contains("more than one return type")
        );
        assert!(
            java_type("Lcom/example/A")
                .expect_err("unterminated")
                .contains("never terminated")
        );
        assert!(
            java_type("L;")
                .expect_err("names nothing")
                .contains("names no class")
        );
        assert!(
            java_type("")
                .expect_err("empty")
                .contains("ends before a type")
        );
    }

    #[test]
    fn one_identifier_becomes_a_constant_a_function_and_a_java_method() {
        assert_eq!(constant_name("chat.filter-text"), "CHAT_FILTER_TEXT");
        assert_eq!(rust_name("chat.filter-text"), "chat_filter_text");
        assert_eq!(java_name("chat.filter-text"), "chatFilterText");
        assert_eq!(
            java_name("input.smooth-last-amount"),
            "inputSmoothLastAmount"
        );
        assert_eq!(java_name(""), "");
    }
}
