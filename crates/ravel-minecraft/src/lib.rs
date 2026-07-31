#![forbid(unsafe_code)]

//! Version-neutral Minecraft-facing capability contracts.

use std::fmt;

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

/// One argument a hook receives.
///
/// ADG-4 decided that the game keeps its state and Ravel duplicates nothing, so
/// a hook receives the values a call site already had rather than a handle onto
/// the object holding them. Every variant here is owned and self-contained:
/// nothing in it can go stale, because nothing in it refers back to a live game
/// object.
#[derive(Clone, Debug, PartialEq)]
pub enum HookValue {
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
    fn observe(&self, target: &HookTarget, values: &[HookValue]) -> HookOutcome;
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
    use super::{
        HookId, HookKind, HookObserver, HookOutcome, HookTarget, HookValue, VersionAdapter,
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
        fn observe(&self, target: &HookTarget, values: &[HookValue]) -> HookOutcome {
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
        ];

        assert_eq!(values.len(), 9);
        assert_ne!(values[0], values[1]);
        assert_eq!(values[8], HookValue::Text("hello".to_owned()));
    }
}
