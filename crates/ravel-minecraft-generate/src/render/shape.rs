//! Validates hook descriptors and derives the shared Java/JNI hook shape.

use crate::classfile::method_types;
use crate::error::GenerateError;
use crate::inputs::Inventory;
use crate::resolve::Resolved;

/// A JVM type a hook argument can carry across the boundary.
///
/// Primitive and string values are copied; other references are borrowed opaque handles.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum JavaType {
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
    pub(super) const fn descriptor(&self) -> &'static str {
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
    pub(super) const fn java(&self) -> &'static str {
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
    pub(super) const fn rust(&self) -> &'static str {
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
    pub(super) fn value(&self, name: &str) -> String {
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
pub(crate) struct Shape {
    pub(super) capability: String,
    pub(super) constant: String,
    pub(super) rust_function: String,
    pub(super) java_method: String,
    pub(super) parameters: Vec<JavaType>,
    pub(super) runtime_class: String,
    pub(super) runtime_member: String,
    pub(super) runtime_descriptor: String,
    pub(super) is_static: bool,
    pub(super) refuses: bool,
}

impl Shape {
    /// Whether this hook returns a veto verdict to its Java caller.
    pub(crate) const fn refuses(&self) -> bool {
        self.refuses
    }
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
    pub(super) fn hook_descriptor(&self) -> String {
        let mut text = String::from("(");
        for parameter in &self.parameters {
            text.push_str(parameter.descriptor());
        }
        text.push_str(if self.refuses { ")Z" } else { ")V" });
        text
    }

    /// How the generated Java declares this hook's return type.
    pub(super) const fn java_returns(&self) -> &'static str {
        if self.refuses { "boolean" } else { "void" }
    }
}

/// Derives every method hook's generated names, refusing anything unrepresentable.
pub(crate) fn shapes(
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
pub(super) fn parameters(descriptor: &str) -> Result<Vec<JavaType>, String> {
    let (parameters, _) = method_types(descriptor)?;
    Ok(parameters
        .into_iter()
        .map(|kind| {
            if kind.dimensions != 0 {
                return if kind.descriptor == "[B" {
                    JavaType::Bytes
                } else {
                    JavaType::Handle(kind.descriptor.to_owned())
                };
            }
            if kind.descriptor.starts_with('L') {
                return if kind.name == "java/lang/String" {
                    JavaType::Text
                } else {
                    JavaType::Handle(kind.name.to_owned())
                };
            }
            match kind.name {
                "boolean" => JavaType::Boolean,
                "byte" => JavaType::Byte,
                "char" => JavaType::Char,
                "short" => JavaType::Short,
                "int" => JavaType::Int,
                "long" => JavaType::Long,
                "float" => JavaType::Float,
                "double" => JavaType::Double,
                name => JavaType::Handle(name.to_owned()),
            }
        })
        .collect())
}

/// Turns a hook identifier into a Rust constant name.
pub(super) fn constant_name(id: &str) -> String {
    id.replace(['.', '-'], "_").to_uppercase()
}

/// Turns a hook identifier into a Rust function name.
pub(super) fn rust_name(id: &str) -> String {
    id.replace(['.', '-'], "_").to_lowercase()
}

/// Turns a hook identifier into a Java method name.
pub(super) fn java_name(id: &str) -> String {
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
