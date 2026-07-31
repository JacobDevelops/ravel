#![forbid(unsafe_code)]

//! Version-neutral Minecraft-facing capability contracts.

use std::fmt;

use jni::objects::JObject;

/// Stable identifier of one hook target, as the reviewed inventory names it.
///
/// The identifier is the only name that survives a game update: a class, a
/// member, and a descriptor all belong to one version's data, and the generated
/// binding is what maps this identifier onto them.
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

/// An opaque reference to an object the game owns, valid for one hook call.
///
/// ADG-4's M3-W0 amendment admits exactly this and nothing more: a reference to
/// state the *game* keeps, which the loader may hold for the duration of a call
/// and hand back, and cannot read into. It is not the mirror that gate refused.
/// A handle names the one authoritative object rather than copying a field out
/// of it, so it cannot go stale; it costs a pointer rather than a per-entity
/// copy; and it is opaque, so its shape is the same in every game version while
/// a copy's field layout is not.
///
/// The residual risk the amendment names is lifetime, and three separate things
/// close it. Each has its own test below, and each pins the failure it wants, so
/// a change that makes one of them start compiling is caught by that one rather
/// than absorbed by another.
///
/// A handle *borrows* the local reference the JVM handed the exported frame, so
/// it cannot leave the frame that built it:
///
/// ```compile_fail,E0515
/// use jni::objects::JObject;
/// use ravel_minecraft::JavaHandle;
///
/// // A generated export, not anything written by hand: the frame owns the
/// // reference the JVM passed it, so nothing borrowed from it can leave. This
/// // is the one place a JNI type is named, and it does not compile.
/// fn keep<'local>(argument0: JObject<'local>) -> JavaHandle<'local> {
///     JavaHandle::new("java/lang/Object", &argument0)
/// }
/// ```
///
/// A JNI local reference belongs to one thread, and the borrow carries that too.
/// The test uses a *scoped* thread deliberately: an ordinary `spawn` also
/// requires `'static`, so it would fail even for a handle that had lost its
/// lifetime entirely, and would keep passing for the wrong reason. A scoped
/// thread requires only `Send`, which isolates the thread route:
///
/// ```compile_fail,E0277
/// use ravel_minecraft::JavaHandle;
///
/// fn hand_off(handle: JavaHandle<'_>) {
///     std::thread::scope(|scope| {
///         scope.spawn(move || drop(handle));
///     });
/// }
/// ```
///
/// And an observer cannot keep one past the call it was shown. This is the
/// escape an author would actually attempt, and the one the amendment leans on.
/// The compiler's own words for it are "`values` escapes the method body here"
/// and "argument requires that `'1` must outlive `'static`":
///
/// ```compile_fail,E0521
/// use std::sync::Mutex;
/// use ravel_minecraft::{HookObserver, HookOutcome, HookTarget, HookValue, JavaHandle};
///
/// struct Hoarder(Mutex<Vec<JavaHandle<'static>>>);
///
/// impl HookObserver for Hoarder {
///     fn observe(&self, _target: &HookTarget, values: &[HookValue<'_>]) -> HookOutcome {
///         for value in values {
///             if let HookValue::Handle(handle) = value {
///                 self.0.lock().expect("not poisoned").push(*handle);
///             }
///         }
///         HookOutcome::Proceed
///     }
/// }
/// ```
///
/// What a handle offers is what needs no call into the JVM, and reading it names
/// no JNI type at all — only the generated export that constructs one does:
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
///
/// Reading a field through a handle and keeping the result is a mirror, and
/// every rule ADG-4 wrote about one still applies to it. Calling into the game
/// at all is ADG-6's to settle, and until it is settled a handle is only
/// something an observer can recognise and pass on.
#[derive(Clone, Copy)]
pub struct JavaHandle<'local> {
    class: &'static str,
    object: &'local JObject<'local>,
}

impl<'local> JavaHandle<'local> {
    /// Borrows one reference the JVM handed to a hook.
    ///
    /// This is public for one reason: the generated exports that call it are
    /// compiled into `ravel-bootstrap`, so `pub(crate)` cannot reach them. It is
    /// the only signature in this crate that names a JNI type, and nothing above
    /// this crate names one — an observer sees a handle's declared class and
    /// whether it is null, and no `JObject`. That holds today because no mod can
    /// depend on `ravel-minecraft`. A change that makes this crate reachable from
    /// a mod has to seal this constructor rather than move the dependency;
    /// `docs/architecture/dependency-rules.md` records that as a boundary.
    ///
    /// `class` is the internal name the target's descriptor declares, which is
    /// how an observer tells one handle from another without asking the JVM
    /// anything.
    #[must_use]
    pub const fn new(class: &'static str, object: &'local JObject<'local>) -> Self {
        Self { class, object }
    }

    /// The type the call site's descriptor declared, in two conventions.
    ///
    /// For an object it is the internal class name with slashes and no
    /// decoration — `net/minecraft/core/MappedRegistry`. For an array it is the
    /// array's own descriptor instead — `[J`, `[[I`, `[Ljava/lang/String;` —
    /// because an array has no class name to give, only a descriptor. Matching
    /// on a leading `[` is what tells the two apart.
    ///
    /// Either way this is the *declared* type, not the runtime one. A subclass
    /// arrives through the same parameter and reads back as the declared name
    /// here; asking what the object actually is means calling the JVM.
    #[must_use]
    pub const fn class(self) -> &'static str {
        self.class
    }

    /// Whether the call site passed no object at all.
    ///
    /// A reference parameter may be null, and every other question about a
    /// handle needs the JVM. This one does not, so it is the one an observer can
    /// ask today.
    #[must_use]
    pub fn is_null(self) -> bool {
        self.object.as_raw().is_null()
    }
}

/// Two handles are equal when they are the same JNI reference to a real object.
///
/// The answer is one-sided and deliberately so. The JVM may hand out two
/// distinct references to one object, so equal handles do name the same object
/// while unequal ones say nothing; asking the other question is a call into the
/// JVM, and a handle does not offer one.
///
/// **A null handle equals nothing, including itself.** Null is the absence of an
/// object rather than an object every absence shares, and treating it as a value
/// breaks the property above in the one case an observer would be misled by:
/// `MappedRegistry.register` can pass a null key and a null registration info in
/// one call, and comparing handles is the only identity operation this type
/// offers, so equal-nulls would have an observer conclude that two parameters
/// naming nothing name the same thing. This makes `==` non-reflexive on null,
/// exactly as it is on `f64::NAN`, which is why [`HookValue`] is `PartialEq` and
/// not `Eq`.
///
/// Declared class is deliberately not compared. One object can arrive through
/// two parameters declared with different types, and it is still one object; the
/// class is a fact about the call site, not about what the reference points at.
impl PartialEq for JavaHandle<'_> {
    fn eq(&self, other: &Self) -> bool {
        let raw = self.object.as_raw();
        !raw.is_null() && std::ptr::eq(raw, other.object.as_raw())
    }
}

/// Names the class and whether anything is there, never the address.
///
/// A JNI reference address is different on every run and says nothing about what
/// went wrong, so printing it would make a diagnostic irreproducible in exchange
/// for no information.
impl fmt::Debug for JavaHandle<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JavaHandle")
            .field("class", &self.class)
            .field("null", &self.is_null())
            .finish()
    }
}

/// One argument a hook receives.
///
/// ADG-4 decided that the game keeps its state and Ravel duplicates nothing, so
/// every variant here is either a value the call site already had or, since the
/// M3-W0 amendment, a [`JavaHandle`] onto an object the game owns. Nothing here
/// can go stale: the owned variants refer back to no live object, and the handle
/// refers to the authoritative one rather than to a copy of it.
///
/// The lifetime is the hook call. It is what lets a handle be carried at all;
/// what stops one being *stored* past the call is [`HookObserver::observe`]'s
/// signature and an observer being `'static`, which is written down there.
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

/// What an observer decided about the call it was shown.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HookOutcome {
    /// The game call runs as it would have.
    Proceed,
    /// The game call is refused, and the reason is raised into the JVM.
    ///
    /// A veto is a thrown exception rather than a returned flag, because the
    /// injected call site then needs no branch and no return value to honour it.
    Veto(String),
}

/// Something the loader shows every hook invocation to.
///
/// Implementors are called from the game's own threads, so an observer has to be
/// usable from any of them.
pub trait HookObserver: Send + Sync {
    /// Shown one invocation of `target` carrying `values`.
    ///
    /// **This signature is what confines a handle, and its exact shape is load
    /// bearing.** The lifetime in `&[HookValue<'_>]` is elided, so it is
    /// late-bound: every call binds a fresh region that the implementor cannot
    /// name, cannot unify with the borrow of `&self`, and cannot return. That,
    /// together with an observer being `'static` — it is stored as
    /// `Box<dyn HookObserver>` — is what makes retaining a handle past the call
    /// a compile error, and it is what ADG-4's amendment leans on.
    ///
    /// Writing it as `fn observe<'a>(&'a self, …, values: &'a [HookValue<'a>])`
    /// would tie the values to the observer's own borrow and give that up, while
    /// looking like a harmless generalisation. Do not.
    fn observe(&self, target: &HookTarget, values: &[HookValue<'_>]) -> HookOutcome;
}

/// What a per-version adapter offers the version-neutral loader.
pub trait VersionAdapter {
    /// The Minecraft version this adapter's data was resolved against.
    fn minecraft_version(&self) -> &str;

    /// Every target the reviewed inventory pinned, in inventory order.
    fn targets(&self) -> &[HookTarget];

    /// The target one reviewed hook resolved to, if this adapter carries it.
    fn target(&self, id: HookId) -> Option<&HookTarget> {
        self.targets().iter().find(|target| target.id() == id)
    }
}

#[cfg(test)]
mod tests {
    use jni::objects::JObject;

    use super::{
        HookId, HookKind, HookObserver, HookOutcome, HookTarget, HookValue, JavaHandle,
        VersionAdapter,
    };

    const CHAT: HookId = HookId::new("chat.allowed-character");
    const INPUT: HookId = HookId::new("input.smooth-last-amount");

    static TARGETS: [HookTarget; 2] = [
        HookTarget::new(
            CHAT,
            "net/minecraft/util/StringUtil",
            "isAllowedChatCharacter",
            "(I)Z",
            HookKind::StaticMethod,
        ),
        HookTarget::new(
            INPUT,
            "net/minecraft/util/SmoothDouble",
            "lastAmount",
            "D",
            HookKind::InstanceField,
        ),
    ];

    struct Pinned;

    impl VersionAdapter for Pinned {
        fn minecraft_version(&self) -> &'static str {
            "0.0.0-test"
        }

        fn targets(&self) -> &[HookTarget] {
            &TARGETS
        }
    }

    struct Refusing;

    impl HookObserver for Refusing {
        fn observe(&self, target: &HookTarget, values: &[HookValue<'_>]) -> HookOutcome {
            if values == [HookValue::Int(7)] {
                HookOutcome::Veto(format!("{} refused 7", target.id()))
            } else {
                HookOutcome::Proceed
            }
        }
    }

    #[test]
    fn a_pinned_target_reports_every_name_the_agent_needs() {
        let target = &TARGETS[0];

        assert_eq!(target.id(), CHAT);
        assert_eq!(target.id().as_str(), "chat.allowed-character");
        assert_eq!(target.class(), "net/minecraft/util/StringUtil");
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
    fn an_adapter_finds_the_targets_it_carries_and_refuses_the_ones_it_does_not() {
        let adapter = Pinned;

        assert_eq!(adapter.minecraft_version(), "0.0.0-test");
        assert_eq!(adapter.targets().len(), 2);
        assert_eq!(
            adapter.target(INPUT).map(HookTarget::member),
            Some("lastAmount")
        );
        assert_eq!(adapter.target(HookId::new("absent")), None);
    }

    #[test]
    fn an_observer_decides_per_invocation_from_the_values_it_is_shown() {
        let observer = Refusing;

        assert_eq!(
            observer.observe(&TARGETS[0], &[HookValue::Int(7)]),
            HookOutcome::Veto("chat.allowed-character refused 7".to_owned())
        );
        assert_eq!(
            observer.observe(&TARGETS[0], &[HookValue::Int(8)]),
            HookOutcome::Proceed
        );
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
            HookValue::Handle(JavaHandle::new(
                "net/minecraft/core/MappedRegistry",
                &absent,
            )),
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
        let handle = JavaHandle::new("net/minecraft/core/MappedRegistry", &absent);

        assert_eq!(handle.class(), "net/minecraft/core/MappedRegistry");
        assert!(handle.is_null());

        let rendered = format!("{handle:?}");
        assert!(
            rendered.contains("net/minecraft/core/MappedRegistry"),
            "{rendered}"
        );
        assert!(rendered.contains("null: true"), "{rendered}");
        assert!(
            !rendered.contains("0x"),
            "an address varies per run and belongs in no diagnostic: {rendered}"
        );
    }

    /// The equal case is not testable here and deliberately has no test.
    ///
    /// `JObject::null` is the only reference this crate can build without a JVM,
    /// so a unit test can only produce null handles. Two handles onto one real
    /// object need a live local reference frame, which is a launch. What is
    /// testable is the case that was wrong, and it is the one that mattered.
    #[test]
    fn a_null_handle_equals_nothing_because_it_names_nothing() {
        let absent = JObject::null();
        let also_absent = JObject::null();
        let key = JavaHandle::new("net/minecraft/resources/ResourceKey", &absent);
        let copied = key;

        // The failure this guards: `MappedRegistry.register` can pass a null key
        // and a null registration info in one call, and comparing handles is the
        // only identity operation the type offers, so an observer correlating
        // its arguments must not be told that two absences are one object.
        assert_ne!(
            key,
            JavaHandle::new("net/minecraft/core/RegistrationInfo", &also_absent),
            "two nulls in one call are not one object"
        );
        assert_ne!(
            key,
            JavaHandle::new("net/minecraft/resources/ResourceKey", &absent),
            "not even the same null reference, declared the same way"
        );
        assert_ne!(key, copied, "null is not reflexive, exactly as NAN is not");
        assert_ne!(
            HookValue::Handle(key),
            HookValue::Text("net/minecraft/resources/ResourceKey".to_owned())
        );
    }
}
