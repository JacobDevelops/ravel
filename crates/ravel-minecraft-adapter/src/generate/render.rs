//! Rendering the four generated artefacts from one resolution.
//!
//! Everything here is string emission from data already resolved, which is what
//! makes generation reproducible: the same inputs render the same bytes, and the
//! generator refuses to write if two runs disagree.

use std::fmt::Write as _;

use super::classfile::{ACC_FINAL, ACC_PROTECTED, ACC_PUBLIC, ACC_STATIC};
use super::inputs::{Adapter, Inventory};
use super::resolve::Resolved;
use super::stubs::Stub;
use super::{EXTRACTOR, GenerateError, HOOK_CLASS, LIBRARY};

/// A JVM type a hook argument can carry across the boundary.
///
/// ADG-4 says a hook receives what the game owns rather than a copy Ravel keeps.
/// Primitives satisfy that by being values; a `java.lang.String` is copied
/// because it is immutable and a copy of one cannot go stale; and every other
/// reference — an ordinary object or an array alike — crosses as the opaque
/// handle the gate's M3-W0 amendment admits, borrowed for the call and readable
/// through nothing.
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
    internal_class: String,
    member: String,
    descriptor: String,
    is_static: bool,
}

impl Shape {
    /// The descriptor of the generated hook method, which always returns void.
    ///
    /// A hook returns nothing because the injected call site then needs no
    /// branch, no discarded value, and no invented contract for what a returned
    /// value would mean while ADG-2 is undecided. A hook that has to stop the
    /// call throws, which the call site honours without any bytecode of its own.
    fn hook_descriptor(&self) -> String {
        let mut text = String::from("(");
        for parameter in &self.parameters {
            text.push_str(parameter.descriptor());
        }
        text.push_str(")V");
        text
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
        shapes.push(Shape {
            capability: hook.capability.clone(),
            constant: constant_name(&hook.id),
            rust_function: rust_name(&hook.id),
            java_method: java_name(&hook.id),
            parameters: parameters(&target.descriptor)
                .map_err(|detail| GenerateError::Unsupported(format!("{}: {detail}", hook.id)))?,
            internal_class: target.internal_class.clone(),
            member: target.member.clone(),
            descriptor: target.descriptor.clone(),
            is_static: target.is_static,
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
///
/// Splitting one parameter off the front rather than stepping character by
/// character is what makes an array readable: `[` is a prefix on another
/// descriptor, so it needs the rest parsed before it knows where it ends.
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
        adapter.mapping_source,
        adapter.inputs.client_jar.sha256.as_deref().unwrap_or("")
    );
    for target in resolved {
        let _ = writeln!(
            text,
            "\n[target.\"{}\"]\nclass = \"{}\"\nmember = \"{}\"\ndescriptor = \"{}\"\nstatic = {}",
            target.id, target.internal_class, target.member, target.descriptor, target.is_static
        );
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
         use ravel_minecraft::{{HookId, HookKind, HookTarget}};\n\
         \n\
         /// The Minecraft version every target below was resolved against.\n\
         pub const MINECRAFT_VERSION: &str = \"{}\";\n",
        adapter.minecraft, adapter.mapping_source, adapter.minecraft
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
            target.internal_class,
            target.member,
            target.descriptor,
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
         // One export per hook shape, as ADG-1 requires: a single dispatcher\n\
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

        let _ = writeln!(
            text,
            "/// {}\n\
             #[::jni::jni_mangle(\"{HOOK_CLASS}\", \"{}\")]\n\
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

        let _ = writeln!(
            text,
            ") {{\n\
             \x20   crate::guard::enter(\n\
             \x20       env,\n\
             \x20       &::ravel_minecraft_adapter::pinned::{},\n\
             \x20       |{}| ::std::result::Result::Ok({values}),\n\
             \x20   );\n\
             }}\n",
            shape.constant,
            if carries_text { "env" } else { "_env" }
        );
    }
    text
}

/// Renders the generated Java hook class.
pub(super) fn hook_java(adapter: &Adapter, shapes: &[Shape]) -> String {
    let mut text = String::with_capacity(shapes.len() * 384 + 2048);
    let _ = writeln!(
        text,
        "// Generated from the pinned Minecraft {} inputs. Do not edit.\n\
         package dev.ravel.jvm;\n\n\
         /**\n\
         \x20* Hook surface the agent injects calls to.\n\
         \x20*\n\
         \x20* <p>Every method here is static and returns void: the injected call site\n\
         \x20* loads the target's own arguments, calls one of these, and continues. A\n\
         \x20* hook that has to stop the call throws, which needs no bytecode of its\n\
         \x20* own at the call site.\n\
         \x20*/\n\
         public final class Hook {{\n\n\
         \x20   /** Native library carrying the generated exports. */\n\
         \x20   public static final String LIBRARY = \"{LIBRARY}\";\n\n\
         \x20   static {{\n\
         \x20       loadLibrary();\n\
         \x20   }}\n\n\
         \x20   // JNI binds a loaded library to the class loader that loaded the class\n\
         \x20   // whose code called System.loadLibrary, and resolves native methods only\n\
         \x20   // against that loader's copy of the declaring class. Loading it here, from\n\
         \x20   // the declaring class's own initializer, makes those two loaders the same\n\
         \x20   // one by construction rather than by convention, so a second copy of this\n\
         \x20   // class under another loader cannot bind to a library this one loaded.\n\
         \x20   //\n\
         \x20   // The load is a restricted method under JEP 472. It is exactly the\n\
         \x20   // operation this class exists to perform, and the launch enables native\n\
         \x20   // access for it, so the warning is acknowledged here rather than silenced\n\
         \x20   // across the whole class.\n\
         \x20   @SuppressWarnings(\"restricted\")\n\
         \x20   private static void loadLibrary() {{\n\
         \x20       System.loadLibrary(LIBRARY);\n\
         \x20   }}\n\n\
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
             \x20               \"{}\"),",
            shape.internal_class,
            shape.member,
            shape.descriptor,
            shape.is_static,
            shape.java_method,
            shape.hook_descriptor()
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
             \x20   public static native void {}({declared});\n",
            shape.capability, shape.java_method
        );
    }

    // Written rather than generated, like the count below them: these belong to
    // the bootstrap rather than to any version's hook surface. They are declared
    // here because ADG-1 gives Ravel one exported symbol surface and it is this
    // class — a screen with natives of its own would be a second one.
    let _ = writeln!(
        text,
        "    /** Hook invocations the native side has counted. */\n\
         \x20   public static native long nativeInvocations();\n\n\
         \x20   /**\n\
         \x20    * One mod's settings, as the screen draws them.\n\
         \x20    *\n\
         \x20    * <p>ADG-6 budgets two batched crossings per screen session and this is\n\
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
         \x20    * @return the settings document, or {{@code null}} when this process\n\
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
         \x20   /**\n\
         \x20    * The installed mod set, in resolved load order.\n\
         \x20    *\n\
         \x20    * <p>M3-W5's mod list is ADG-6's mechanism rather than a second one,\n\
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
         \x20    * @return the mod list, or {{@code null}} when no set is installed\n\
         \x20    */\n\
         \x20   public static native String ravelModSurface();\n\
         }}"
    );
    text
}

/// Renders the stub provenance lock.
///
/// The header states what the lock is worth, because a file named `.lock` reads
/// like a stronger promise than this one makes.
pub(super) fn stub_lock(adapter: &Adapter, stubs: &[Stub]) -> String {
    let mut text = String::with_capacity(stubs.len() * 256 + 1536);
    let _ = writeln!(
        text,
        "# Generated stub provenance for Minecraft {}. Do not edit.\n\
         #\n\
         # Every entry below is one compile-time stub the settings screen is built\n\
         # against, read out of the pinned client jar. Descriptors and access flags\n\
         # are the jar's own, which is the point: a hand-written stub transcribes a\n\
         # generic signature, and the JVM links on the erased one.\n\
         #\n\
         # What this file does not promise:\n\
         #\n\
         # - It is checked only where the client jar is. A machine without the jar\n\
         #   generates from this file, so on that machine the lock is the source and\n\
         #   cannot disagree with anything. Drift is caught where the jar exists.\n\
         # - It pins against a jar hash, so it is only as current as that pin. A\n\
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
         clientJarSha256 = \"{}\"",
        adapter.minecraft,
        adapter.schema_version,
        adapter.minecraft,
        adapter.inputs.client_jar.sha256.as_deref().unwrap_or("")
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
        }
    }
    text
}

/// Renders one stub as a Java source file, with the path it belongs at.
pub(super) fn stub_java(adapter: &Adapter, stub: &Stub) -> Result<(String, String), GenerateError> {
    let unsupported =
        |detail: String| GenerateError::Unsupported(format!("{}: {detail}", stub.class));
    let (package, simple) = split(&stub.class).map_err(&unsupported)?;

    let mut text = String::with_capacity(stub.members.len() * 192 + 1024);
    let _ = writeln!(
        text,
        "// Generated from the pinned Minecraft {} class files. Do not edit.\n\
         //\n\
         // A compile-time stub, not Minecraft. It is compiled against and then\n\
         // discarded: `build-screen.sh` refuses to package one, because a stub in\n\
         // the screen jar would shadow the game's own class on the loader they\n\
         // share. See minecraft/versions/{}/stubs.lock.\n\
         package {package};\n",
        adapter.minecraft, adapter.minecraft
    );

    let mut declaration = String::new();
    if stub.flags & ACC_PUBLIC != 0 {
        declaration.push_str("public ");
    }
    if stub.is_interface() {
        declaration.push_str("interface ");
    } else {
        if stub.is_abstract() {
            declaration.push_str("abstract ");
        }
        if stub.is_final() {
            declaration.push_str("final ");
        }
        declaration.push_str("class ");
    }
    declaration.push_str(simple);
    if let Some(parent) = &stub.extends {
        let _ = write!(declaration, " extends {}", source_name(parent));
    }
    if !stub.implements.is_empty() {
        // An interface writes every supertype after `extends`; a class separates
        // its one superclass from the interfaces it is also assignable to.
        declaration.push_str(if stub.is_interface() && stub.extends.is_none() {
            " extends "
        } else if stub.is_interface() {
            ", "
        } else {
            " implements "
        });
        declaration.push_str(
            &stub
                .implements
                .iter()
                .map(|name| source_name(name))
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    let _ = writeln!(text, "{declaration} {{");

    for member in &stub.members {
        let modifiers = modifiers(member.flags, stub.is_interface(), member.method);
        if member.method {
            let (parameters, returns) = signature(&member.descriptor).map_err(&unsupported)?;
            let declared = parameters
                .iter()
                .enumerate()
                .map(|(index, parameter)| format!("{parameter} argument{index}"))
                .collect::<Vec<_>>()
                .join(", ");
            if member.name == "<init>" {
                let _ = writeln!(text, "\n    {modifiers}{simple}({declared}) {{\n    }}");
            } else if returns == "void" {
                let _ = writeln!(
                    text,
                    "\n    {modifiers}void {}({declared}) {{\n    }}",
                    member.name
                );
            } else {
                // A stub body is never reached: the real class is what the game
                // loads. Throwing rather than returning a default is what makes
                // that visible if one ever is.
                let _ = writeln!(
                    text,
                    "\n    {modifiers}{returns} {}({declared}) {{\n        \
                     throw new UnsupportedOperationException(\"stub\");\n    }}",
                    member.name
                );
            }
        } else if stub.is_interface() {
            return Err(unsupported(format!(
                "{} is a field on an interface, which is implicitly final and \
                 would need a value no stub can invent",
                member.name
            )));
        } else {
            let (kind, rest) = java_type(&member.descriptor).map_err(&unsupported)?;
            if !rest.is_empty() {
                return Err(unsupported(format!(
                    "{} declares {} which is not one type",
                    member.name, member.descriptor
                )));
            }
            let _ = writeln!(text, "\n    {modifiers}{kind} {};", member.name);
        }
    }

    let _ = writeln!(text, "}}");
    Ok((format!("{}.java", stub.class), text))
}

/// The modifiers a stub reproduces, as Java source writes them.
///
/// `abstract` is never among them. Every stub member has a body, so a class that
/// extends a stub needs to implement nothing it did not mean to.
fn modifiers(flags: u16, interface: bool, method: bool) -> String {
    let mut text = String::new();
    if flags & ACC_PUBLIC != 0 && !interface {
        text.push_str("public ");
    } else if flags & ACC_PROTECTED != 0 {
        text.push_str("protected ");
    }
    if flags & ACC_STATIC != 0 {
        text.push_str("static ");
    } else if interface && method {
        // An interface method with a body has to say so, and giving it one is
        // what keeps a stub that implements the interface from having to.
        text.push_str("default ");
    }
    if flags & ACC_FINAL != 0 && method {
        text.push_str("final ");
    }
    text
}

/// Splits an internal class name into its package and its simple name.
/// Splits an internal class name into its package and the name Java declares.
///
/// A nested class keeps its `$`: the declared name is the binary one, which is
/// what the JVM links against and what javac writes into the class file.
fn split(class: &str) -> Result<(String, &str), String> {
    let (package, simple) = class
        .rsplit_once('/')
        .ok_or_else(|| format!("{class} is in the default package"))?;
    Ok((package.replace('/', "."), simple))
}

/// An internal class name as Java source writes it.
fn source_name(class: &str) -> String {
    class.replace('/', ".")
}

/// Reads a method descriptor as the Java source types it declares.
fn signature(descriptor: &str) -> Result<(Vec<String>, String), String> {
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
fn java_type(descriptor: &str) -> Result<(String, &str), String> {
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
    use super::super::inputs::{Adapter, Hook, Input, Inputs, Inventory, Kind};
    use super::super::resolve::Resolved;
    use super::super::stubs::{Member as StubMember, Stub};
    use super::{
        JavaType, constant_name, exports, hook_java, java_name, parameters, rust_name, shapes,
    };
    use super::{java_type, signature, stub_java, stub_lock};

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
            mapping_source: "unobfuscated-class-files".to_owned(),
            inputs: Inputs {
                client_jar: unpinned(),
                version_manifest: unpinned(),
                hook_inventory: unpinned(),
                stub_seeds: unpinned(),
            },
        }
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
            })
            .collect::<Vec<_>>();
        let resolved = hooks
            .iter()
            .map(|hook| Resolved {
                id: hook.id.clone(),
                internal_class: hook.class.replace('.', "/"),
                member: hook.member.clone(),
                descriptor: hook.descriptor.clone(),
                is_static: false,
            })
            .collect();
        (
            Inventory {
                schema_version: 1,
                minecraft: "0.0.0-test".to_owned(),
                hooks,
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
            parameters("(Lnet/minecraft/resources/ResourceKey;Ljava/lang/Object;)V")
                .expect("ADG-4 admits a handle onto an object the game owns"),
            vec![
                JavaType::Handle("net/minecraft/resources/ResourceKey".to_owned()),
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
    fn a_descriptor_that_is_not_one_is_refused_and_says_what_about_it_is_wrong() {
        assert!(
            parameters("(Lnet/minecraft/core/Registry)V")
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
            JavaType::Handle("java/lang/Object".to_owned()),
        ] {
            let descriptor = format!("({}){}", parameter.descriptor(), parameter.descriptor());

            assert_eq!(
                parameters(&descriptor).expect("valid"),
                vec![parameter.clone()]
            );
            assert!(!parameter.java().is_empty());
            assert!(parameter.rust().starts_with("::jni::"));
            assert!(parameter.value("x").contains("HookValue::"));
        }
    }

    #[test]
    fn a_handle_keeps_its_declared_class_in_rust_and_loses_it_in_the_hook_signature() {
        let registry = JavaType::Handle("net/minecraft/core/MappedRegistry".to_owned());

        assert_eq!(registry.descriptor(), "Ljava/lang/Object;");
        assert_eq!(registry.java(), "Object");
        assert_eq!(registry.rust(), "::jni::objects::JObject<'local>");
        assert_eq!(
            registry.value("argument0"),
            "::ravel_minecraft::HookValue::Handle(::ravel_minecraft::JavaHandle::new(\
             \"net/minecraft/core/MappedRegistry\", &argument0))"
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
            "net.minecraft.core.MappedRegistry",
            "register",
            "(Lnet/minecraft/resources/ResourceKey;Ljava/lang/Object;\
             Lnet/minecraft/core/RegistrationInfo;)Lnet/minecraft/core/Holder$Reference;",
        ),
        (
            "server.tick",
            "net.minecraft.server.MinecraftServer",
            "tickServer",
            "(Ljava/util/function/BooleanSupplier;)V",
        ),
        (
            "server.tick-times",
            "net.minecraft.server.MinecraftServer",
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
                "::ravel_minecraft::JavaHandle::new(\"net/minecraft/resources/ResourceKey\", \
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
                "\"(Lnet/minecraft/resources/ResourceKey;Ljava/lang/Object;\
                 Lnet/minecraft/core/RegistrationInfo;)Lnet/minecraft/core/Holder$Reference;\""
            ),
            "the target keeps its own descriptor, which is what the agent matches on: {java}"
        );
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
                })
                .collect(),
        }
    }

    #[test]
    fn a_stub_declares_the_members_it_carries_with_the_descriptors_the_jar_declared() {
        let screen = stub(
            "net/minecraft/client/gui/screens/Screen",
            0x0421, // public abstract class
            &[
                (
                    "<init>",
                    "(Lnet/minecraft/network/chat/Component;)V",
                    4,
                    true,
                ),
                ("init", "()V", 4, true),
                ("init", "(II)V", 0x0011, true),
                (
                    "getTitle",
                    "()Lnet/minecraft/network/chat/Component;",
                    1,
                    true,
                ),
            ],
        );

        let (path, java) = stub_java(&adapter(), &screen).expect("a stub should render");

        assert_eq!(path, "net/minecraft/client/gui/screens/Screen.java");
        assert!(
            java.contains("package net.minecraft.client.gui.screens;"),
            "{java}"
        );
        assert!(java.contains("public abstract class Screen {"), "{java}");
        assert!(
            java.contains("protected Screen(net.minecraft.network.chat.Component argument0) {"),
            "a constructor takes the class's simple name: {java}"
        );
        assert!(java.contains("protected void init() {"), "{java}");
        assert!(
            java.contains("public final void init(int argument0, int argument1) {"),
            "an overload is stubbed beside the one it overloads: {java}"
        );
        assert!(
            java.contains(
                "public net.minecraft.network.chat.Component getTitle() {\n        \
                 throw new UnsupportedOperationException(\"stub\");"
            ),
            "a stub body is never reached and says so if it is: {java}"
        );
    }

    #[test]
    fn a_stubbed_supertype_is_declared_and_an_interface_method_gets_a_body() {
        let mut component = stub(
            "net/minecraft/network/chat/Component",
            0x0601, // public abstract interface
            &[(
                "translatable",
                "(Ljava/lang/String;)Lnet/minecraft/network/chat/MutableComponent;",
                0x0009,
                true,
            )],
        );
        component.implements = vec!["net/minecraft/network/chat/FormattedText".to_owned()];
        let mut mutable = stub("net/minecraft/network/chat/MutableComponent", 0x0031, &[]);
        mutable.implements = vec!["net/minecraft/network/chat/Component".to_owned()];

        let (_, interface) = stub_java(&adapter(), &component).expect("renders");
        let (_, class) = stub_java(&adapter(), &mutable).expect("renders");

        assert!(
            interface.contains(
                "public interface Component extends net.minecraft.network.chat.FormattedText {"
            ),
            "{interface}"
        );
        assert!(
            interface.contains(
                "static net.minecraft.network.chat.MutableComponent \
                 translatable(java.lang.String argument0) {"
            ),
            "an interface member is never declared public, which javac warns on: {interface}"
        );
        assert!(
            class.contains(
                "public final class MutableComponent implements \
                 net.minecraft.network.chat.Component {"
            ),
            "the edge that makes a returned value assignable is what the stub keeps: {class}"
        );
    }

    #[test]
    fn a_non_static_interface_method_is_given_a_body_rather_than_left_abstract() {
        let component = stub(
            "net/minecraft/network/chat/Component",
            0x0601,
            &[("getString", "()Ljava/lang/String;", 0x0401, true)],
        );

        let (_, java) = stub_java(&adapter(), &component).expect("renders");

        assert!(
            java.contains("default java.lang.String getString() {"),
            "a stub that implements this interface must not have to implement it: {java}"
        );
    }

    #[test]
    fn a_stub_that_cannot_be_written_down_is_refused_rather_than_written_wrongly() {
        for (broken, expected) in [
            (stub("Toplevel", 1, &[]), "is in the default package"),
            (
                stub("a/B", 0x0201, &[("VALUE", "I", 0x0019, false)]),
                "field on an interface",
            ),
            (
                stub("a/B", 1, &[("of", "(Q)V", 1, true)]),
                "not a type descriptor",
            ),
        ] {
            let error = stub_java(&adapter(), &broken).expect_err("this must not render");
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
            "net/minecraft/client/gui/components/Button$Builder",
            0x0021,
            &[(
                "bounds",
                "(IIII)Lnet/minecraft/client/gui/components/Button$Builder;",
                1,
                true,
            )],
        );

        let (path, java) = stub_java(&adapter(), &builder).expect("renders");

        assert!(
            path.ends_with("net/minecraft/client/gui/components/Button$Builder.java"),
            "{path}"
        );
        assert!(java.contains("public class Button$Builder"), "{java}");
        assert!(
            java.contains(
                "public net.minecraft.client.gui.components.Button$Builder bounds(int argument0,"
            ),
            "a nested type is named the same way wherever it appears: {java}"
        );
    }

    #[test]
    fn a_field_stub_keeps_its_declared_type_and_drops_nothing_else() {
        let font = stub(
            "net/minecraft/client/gui/Font",
            1,
            &[("lineHeight", "I", 1, false), ("glyphs", "[[J", 4, false)],
        );

        let (_, java) = stub_java(&adapter(), &font).expect("renders");

        assert!(java.contains("public int lineHeight;"), "{java}");
        assert!(java.contains("protected long[][] glyphs;"), "{java}");
    }

    #[test]
    fn the_stub_lock_records_what_was_read_and_says_what_it_does_not_promise() {
        let mut screen = stub(
            "net/minecraft/client/gui/screens/Screen",
            0x0421,
            &[("init", "()V", 4, true)],
        );
        screen.extends = Some("net/minecraft/A".to_owned());
        screen.implements = vec!["net/minecraft/B".to_owned()];

        let lock = stub_lock(&adapter(), &[screen]);

        assert!(lock.contains("extractor = \"classfile\""), "{lock}");
        assert!(
            lock.contains(
                "[[stub]]\nclass = \"net/minecraft/client/gui/screens/Screen\"\n\
                           flags = 1057\nextends = \"net/minecraft/A\"\n\
                           implements = [\"net/minecraft/B\"]"
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
            lock.contains("checked only where the client jar is"),
            "the header has to say what the lock is worth: {lock}"
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
            java_type("Lnet/minecraft/A")
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
