//! Shared JVM primitive vocabulary and generated crossing types.
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
    const PRIMITIVES: [Self; 8] = [
        Self::Boolean,
        Self::Byte,
        Self::Char,
        Self::Short,
        Self::Int,
        Self::Long,
        Self::Float,
        Self::Double,
    ];

    pub(super) fn primitive_from_descriptor(descriptor: &str) -> Option<Self> {
        Self::PRIMITIVES
            .into_iter()
            .find(|kind| kind.descriptor() == descriptor)
    }

    pub(super) fn primitive_from_keyword(keyword: &str) -> Option<Self> {
        Self::PRIMITIVES
            .into_iter()
            .find(|kind| kind.java() == keyword)
    }

    pub(super) const fn zero_literal(&self) -> &'static str {
        match self {
            Self::Boolean => "false",
            Self::Char => "'\\0'",
            Self::Byte | Self::Short | Self::Int => "0",
            Self::Long => "0L",
            Self::Float => "0.0f",
            Self::Double => "0.0",
            Self::Text | Self::Bytes | Self::Handle(_) => "null",
        }
    }

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

#[cfg(test)]
mod tests {
    use super::JavaType;

    #[test]
    fn primitive_vocabulary_round_trips_and_excludes_nonprimitives() {
        for primitive in JavaType::PRIMITIVES {
            assert_eq!(
                JavaType::primitive_from_descriptor(primitive.descriptor()),
                Some(primitive.clone())
            );
            assert_eq!(
                JavaType::primitive_from_keyword(primitive.java()),
                Some(primitive.clone())
            );
            assert_ne!(primitive.zero_literal(), "");
        }
        for value in ["V", "void", "[I", "Ljava/lang/Object;", "String"] {
            assert_eq!(JavaType::primitive_from_descriptor(value), None);
            assert_eq!(JavaType::primitive_from_keyword(value), None);
        }
    }
}
