#![forbid(unsafe_code)]

//! Version-neutral Minecraft-facing capability contracts.

use std::fmt;

use jni::objects::JObject;

/// Version-neutral hook identifier mapped to a target by generated bindings.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HookId(&'static str);

impl HookId {
    /// Names a hook target.
    #[must_use]
    pub const fn new(id: &'static str) -> Self {
        Self(id)
    }

    /// The identifier as the inventory writes it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for HookId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

/// Shape of a hook target, which decides whether it can be called at all.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HookKind {
    /// A method declared static.
    StaticMethod,
    /// A method declared on instances.
    InstanceMethod,
    /// A field declared static.
    StaticField,
    /// A field declared on instances.
    InstanceField,
}

impl HookKind {
    /// Whether the target is a method rather than a field.
    #[must_use]
    pub const fn is_method(self) -> bool {
        matches!(self, Self::StaticMethod | Self::InstanceMethod)
    }

    /// Whether the class file declares the target static.
    #[must_use]
    pub const fn is_static(self) -> bool {
        matches!(self, Self::StaticMethod | Self::StaticField)
    }
}

/// One resolved hook target, as a version's generated binding pins it.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HookTarget {
    id: HookId,
    class: &'static str,
    member: &'static str,
    descriptor: &'static str,
    kind: HookKind,
}

impl HookTarget {
    /// Pins one resolved target. Generated code is the only caller.
    #[must_use]
    pub const fn new(
        id: HookId,
        class: &'static str,
        member: &'static str,
        descriptor: &'static str,
        kind: HookKind,
    ) -> Self {
        Self {
            id,
            class,
            member,
            descriptor,
            kind,
        }
    }

    /// Which reviewed hook this target resolves.
    #[must_use]
    pub const fn id(&self) -> HookId {
        self.id
    }

    /// Internal class name, as the JVM writes it.
    #[must_use]
    pub const fn class(&self) -> &'static str {
        self.class
    }

    /// Declared member name.
    #[must_use]
    pub const fn member(&self) -> &'static str {
        self.member
    }

    /// Exact JVM descriptor.
    #[must_use]
    pub const fn descriptor(&self) -> &'static str {
        self.descriptor
    }

    /// Declared shape of the member.
    #[must_use]
    pub const fn kind(&self) -> HookKind {
        self.kind
    }
}

/// Opaque game-object reference borrowed for one hook call.
///
/// Handles cannot outlive their JNI frame, cross threads, or be retained by observers.
/// Game operations remain in Java adapters; observers can inspect only declared type and nullness.
///
/// A handle cannot escape the frame that owns its local reference:
///
/// ```compile_fail,E0515
/// use jni::objects::JObject;
/// use ravel_minecraft::JavaHandle;
///
/// fn keep<'local>(argument0: JObject<'local>) -> JavaHandle<'local> {
///     JavaHandle::new("java/lang/Object", &argument0)
/// }
/// ```
///
/// A locally borrowed handle cannot satisfy `Send`:
///
/// ```compile_fail,E0521
/// use ravel_minecraft::JavaHandle;
///
/// fn require_send<T: Send>(_: T) {}
///
/// fn hand_off(handle: JavaHandle<'_>) {
///     require_send(handle);
/// }
/// ```
///
/// Reading a handle requires no JNI types or JVM calls:
///
/// ```
/// use ravel_minecraft::HookValue;
///
/// fn describe(value: &HookValue<'_>) -> String {
///     match value {
///         HookValue::Handle(handle) if handle.is_null() => {
///             format!("no {}", handle.class())
///         }
///         HookValue::Handle(handle) => handle.class().to_owned(),
///         other => format!("{other:?}"),
///     }
/// }
///
/// assert_eq!(describe(&HookValue::Int(7)), "Int(7)");
/// ```
#[derive(Clone, Copy)]
pub struct JavaHandle<'local> {
    class: &'static str,
    object: &'local JObject<'local>,
}

impl<'local> JavaHandle<'local> {
    /// Borrows a hook's JNI reference with its descriptor-declared class.
    ///
    /// Public for generated bootstrap exports, not mod authors. Keep this constructor
    /// outside the mod dependency graph; see `docs/architecture/dependency-rules.md`.
    #[must_use]
    pub const fn new(class: &'static str, object: &'local JObject<'local>) -> Self {
        Self { class, object }
    }

    /// Declared type, not runtime type: an internal class name or an array descriptor.
    ///
    /// Object names use slashes; array descriptors start with `[` (for example, `[J`).
    #[must_use]
    pub const fn class(self) -> &'static str {
        self.class
    }

    /// Whether the call site passed a null reference.
    #[must_use]
    pub fn is_null(self) -> bool {
        self.object.as_raw().is_null()
    }
}

/// Compares non-null JNI reference addresses, ignoring declared class.
///
/// Equal handles identify one object; unequal handles may still refer to the same object.
/// Null equals nothing, including itself.
impl PartialEq for JavaHandle<'_> {
    fn eq(&self, other: &Self) -> bool {
        let raw = self.object.as_raw();
        !raw.is_null() && std::ptr::eq(raw, other.object.as_raw())
    }
}

/// Prints declared class and nullness, never the run-dependent JNI address.
impl fmt::Debug for JavaHandle<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JavaHandle")
            .field("class", &self.class)
            .field("null", &self.is_null())
            .finish()
    }
}

/// Hook argument value or a game-object reference borrowed for the call.
///
/// A borrowed handle cannot be retained beyond the hook call:
///
/// ```compile_fail,E0521
/// use ravel_minecraft::{HookValue, JavaHandle};
///
/// fn retain(values: &[HookValue<'_>], retained: &mut Vec<JavaHandle<'static>>) {
///     for value in values {
///         if let HookValue::Handle(handle) = value {
///             retained.push(*handle);
///         }
///     }
/// }
/// ```
#[derive(Clone, Debug, PartialEq)]
pub enum HookValue<'local> {
    /// A `boolean` argument.
    Boolean(bool),
    /// A `byte` argument.
    Byte(i8),
    /// A `char` argument, as the UTF-16 code unit the JVM passes.
    Char(u16),
    /// A `short` argument.
    Short(i16),
    /// An `int` argument.
    Int(i32),
    /// A `long` argument.
    Long(i64),
    /// A `float` argument.
    Float(f32),
    /// A `double` argument.
    Double(f64),
    /// A `java.lang.String` argument, copied out of the JVM.
    Text(String),
    /// A reference argument, borrowed for the duration of the call.
    Handle(JavaHandle<'local>),
}

/// Major and minor pack-format numbers supplied by version data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackFormat {
    major: u16,
    minor: u16,
}

impl PackFormat {
    /// Names a format by its major and minor.
    #[must_use]
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }

    /// Returns the number a pack declares as its format.
    #[must_use]
    pub const fn major(self) -> u16 {
        self.major
    }

    /// Returns the minor a version that splits its formats states beside it.
    #[must_use]
    pub const fn minor(self) -> u16 {
        self.minor
    }
}

/// How a Minecraft version's packs spell the format they declare.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackFormatSpelling {
    /// One `pack_format` integer.
    PackFormat,
    /// A `max_format` integer beside a `min_format` pair.
    MinMaxFormat,
}

/// Serializes `pack.mcmeta` using the version's format spelling.
///
/// # Panics
///
/// Panics if JSON display formatting violates the infallible `String` writer contract.
#[must_use]
pub fn pack_metadata(
    format: PackFormat,
    spelling: PackFormatSpelling,
    description: &str,
) -> String {
    let mut declared = serde_json::Map::new();
    declared.insert(
        "description".to_owned(),
        serde_json::Value::String(description.to_owned()),
    );
    match spelling {
        PackFormatSpelling::PackFormat => {
            declared.insert("pack_format".to_owned(), format.major.into());
        }
        PackFormatSpelling::MinMaxFormat => {
            declared.insert("max_format".to_owned(), format.major.into());
            declared.insert(
                "min_format".to_owned(),
                serde_json::Value::Array(vec![format.major.into(), format.minor.into()]),
            );
        }
    }
    let mut document = serde_json::Map::new();
    document.insert("pack".to_owned(), serde_json::Value::Object(declared));

    let mut text = String::with_capacity(128);
    fmt::write(
        &mut text,
        format_args!("{:#}\n", serde_json::Value::Object(document)),
    )
    .unwrap_or_else(|error| {
        unreachable!("formatting a JSON value into a String cannot fail: {error}")
    });
    text
}

#[cfg(test)]
mod tests {
    use super::{PackFormat, PackFormatSpelling, pack_metadata};

    #[test]
    fn each_spelling_renders_the_document_that_versions_own_packs_carry() {
        assert_eq!(
            pack_metadata(
                PackFormat::new(34, 0),
                PackFormatSpelling::PackFormat,
                "Ravel"
            ),
            "{\n  \"pack\": {\n    \"description\": \"Ravel\",\n    \"pack_format\": 34\n  }\n}\n"
        );
        assert_eq!(
            pack_metadata(
                PackFormat::new(101, 1),
                PackFormatSpelling::MinMaxFormat,
                "Ravel"
            ),
            "{\n  \"pack\": {\n    \"description\": \"Ravel\",\n    \"max_format\": 101,\n    \
             \"min_format\": [\n      101,\n      1\n    ]\n  }\n}\n"
        );
    }

    #[test]
    fn a_description_holding_any_control_character_still_renders_legal_json() {
        let hostile: String = (0..=0x1f_u8)
            .map(char::from)
            .chain(['"', '\\', '\u{7f}', '\u{2028}', 'é'])
            .collect();

        let rendered = pack_metadata(
            PackFormat::new(34, 0),
            PackFormatSpelling::PackFormat,
            &hostile,
        );

        let parsed: serde_json::Value =
            serde_json::from_str(&rendered).expect("the metadata is legal JSON");
        assert_eq!(
            parsed["pack"]["description"].as_str(),
            Some(hostile.as_str()),
            "the description reads back exactly as it went in"
        );
        assert_eq!(parsed["pack"]["pack_format"].as_u64(), Some(34));
        assert!(
            !rendered.bytes().any(|byte| byte < 0x20 && byte != b'\n'),
            "{rendered:?}"
        );
    }

    use jni::objects::JObject;

    use super::{HookId, HookKind, HookTarget, HookValue, JavaHandle};

    const CHAT: HookId = HookId::new("chat.allowed-character");
    const INPUT: HookId = HookId::new("input.smooth-last-amount");

    static TARGETS: [HookTarget; 2] = [
        HookTarget::new(
            CHAT,
            "com/example/util/StringUtil",
            "isAllowedChatCharacter",
            "(I)Z",
            HookKind::StaticMethod,
        ),
        HookTarget::new(
            INPUT,
            "com/example/util/SmoothDouble",
            "lastAmount",
            "D",
            HookKind::InstanceField,
        ),
    ];

    #[test]
    fn a_pinned_target_reports_every_name_the_agent_needs() {
        let target = &TARGETS[0];

        assert_eq!(target.id(), CHAT);
        assert_eq!(target.id().as_str(), "chat.allowed-character");
        assert_eq!(target.class(), "com/example/util/StringUtil");
        assert_eq!(target.member(), "isAllowedChatCharacter");
        assert_eq!(target.descriptor(), "(I)Z");
        assert_eq!(target.kind(), HookKind::StaticMethod);
        assert_eq!(CHAT.to_string(), "chat.allowed-character");
    }

    #[test]
    fn every_kind_reports_whether_it_is_a_method_and_whether_it_is_static() {
        for (kind, method, is_static) in [
            (HookKind::StaticMethod, true, true),
            (HookKind::InstanceMethod, true, false),
            (HookKind::StaticField, false, true),
            (HookKind::InstanceField, false, false),
        ] {
            assert_eq!(kind.is_method(), method, "{kind:?}");
            assert_eq!(kind.is_static(), is_static, "{kind:?}");
        }
    }

    #[test]
    fn every_argument_shape_the_generator_emits_has_a_value_representation() {
        let absent = JObject::null();
        let values = [
            HookValue::Boolean(true),
            HookValue::Byte(-1),
            HookValue::Char(65),
            HookValue::Short(-2),
            HookValue::Int(-3),
            HookValue::Long(-4),
            HookValue::Float(0.5),
            HookValue::Double(0.25),
            HookValue::Text("hello".to_owned()),
            HookValue::Handle(JavaHandle::new("com/example/core/MappedRegistry", &absent)),
        ];

        assert_eq!(values.len(), 10);
        assert_ne!(values[0], values[1]);
        assert!(
            matches!(values[9], HookValue::Handle(_)),
            "a handle is a value shape like any other"
        );
        assert_eq!(values[8], HookValue::Text("hello".to_owned()));

        let copied = values[9].clone();
        assert_ne!(
            values[9], copied,
            "the handle here is null, and a null handle equals nothing"
        );
    }

    #[test]
    fn a_handle_reports_the_class_it_was_declared_with_and_nothing_about_the_object() {
        let absent = JObject::null();
        let handle = JavaHandle::new("com/example/core/MappedRegistry", &absent);

        assert_eq!(handle.class(), "com/example/core/MappedRegistry");
        assert!(handle.is_null());

        let rendered = format!("{handle:?}");
        assert!(
            rendered.contains("com/example/core/MappedRegistry"),
            "{rendered}"
        );
        assert!(rendered.contains("null: true"), "{rendered}");
        assert!(
            !rendered.contains("0x"),
            "an address varies per run and belongs in no diagnostic: {rendered}"
        );
    }

    // Non-null identity requires a live JVM; unit tests can construct only null references.
    #[test]
    fn a_null_handle_equals_nothing_because_it_names_nothing() {
        let absent = JObject::null();
        let also_absent = JObject::null();
        let key = JavaHandle::new("com/example/resources/ResourceKey", &absent);
        let copied = key;

        assert_ne!(
            key,
            JavaHandle::new("com/example/core/RegistrationInfo", &also_absent),
            "two nulls in one call are not one object"
        );
        assert_ne!(
            key,
            JavaHandle::new("com/example/resources/ResourceKey", &absent),
            "not even the same null reference, declared the same way"
        );
        assert_ne!(key, copied, "null is not reflexive, exactly as NAN is not");
        assert_ne!(
            HookValue::Handle(key),
            HookValue::Text("com/example/resources/ResourceKey".to_owned())
        );
    }
}
