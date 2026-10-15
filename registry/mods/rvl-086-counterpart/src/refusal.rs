//! The two behaviour changes over existing tools: sections eleven and twelve.
//!
//! Both are pure predicates over an item plus the surface being asked about, and
//! both answer the same way — the action is refused. Where they attach is
//! Ravel's observation and interception; what they decide is here.
//!
//! Section twelve's damage refusal is read from the typed source rather than
//! from the message identifier the original compares as a string.

use crate::repair::Shape;
use crate::settings::Settings;

/// The points of durability the nearly-destroyed rule keeps back.
pub const RESERVE: u32 = 2;

/// The surfaces either rule is asked about.
///
/// Seven, and the shapes they apply to differ per surface. The differences are
/// read from the original and are not tidied into one list.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Surface {
    /// How fast a block is mined.
    BreakSpeed,
    /// Breaking a block.
    BreakBlock,
    /// Attacking an entity.
    AttackEntity,
    /// Using an item in the air.
    UseInAir,
    /// Using an item on a block.
    UseOnBlock,
    /// Interacting with an entity.
    InteractEntity,
    /// The shared library's cancellable crop-harvest event.
    CropHarvest,
}

impl Surface {
    /// Every surface, in the order the specification lists them.
    pub const ALL: [Self; 7] = [
        Self::BreakSpeed,
        Self::BreakBlock,
        Self::AttackEntity,
        Self::UseInAir,
        Self::UseOnBlock,
        Self::InteractEntity,
        Self::CropHarvest,
    ];

    /// Whether the nearly-destroyed rule watches this surface for this shape.
    ///
    /// Five surfaces watch the digging, sword and shearing shapes; using an item
    /// in the air watches the shearing shape and ranged weapons instead; and
    /// interacting with an entity watches the shearing shape alone.
    #[must_use]
    pub const fn watches(self, shape: Shape) -> bool {
        match self {
            Self::UseInAir => matches!(shape, Shape::Shearing | Shape::Ranged),
            Self::InteractEntity => matches!(shape, Shape::Shearing),
            Self::BreakSpeed
            | Self::BreakBlock
            | Self::AttackEntity
            | Self::UseOnBlock
            | Self::CropHarvest => matches!(shape, Shape::Digging | Shape::Sword | Shape::Shearing),
        }
    }
}

/// How the player is playing, which decides whether either rule applies at all.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Playing {
    /// Ordinarily, where durability and materials are spent.
    Ordinarily,
    /// Building without consuming anything, which is exempt from both rules.
    WithoutConsuming,
}

/// Whether an item is within the reserve of being destroyed.
///
/// An item whose maximum is two or less is exempt, because reserving two points
/// of a two-point tool would leave it unusable from the moment it is crafted.
#[must_use]
pub const fn nearly_destroyed(damage: u32, maximum_damage: u32) -> bool {
    maximum_damage > RESERVE && damage >= maximum_damage - RESERVE
}

/// Whether the nearly-destroyed rule refuses this action.
#[must_use]
pub fn refuses(
    settings: &Settings,
    surface: Surface,
    shape: Shape,
    damage: u32,
    maximum_damage: u32,
    playing: Playing,
) -> bool {
    settings.tweak_nearly_destroyed
        && playing == Playing::Ordinarily
        && surface.watches(shape)
        && nearly_destroyed(damage, maximum_damage)
}

/// Whether an item is on the ineffective list and of a shape the rule ruins.
///
/// The rule has no switch of its own: an empty list is the whole of what turns
/// it off. The list is a list of things to ruin rather than of things to
/// protect, so a tool that is not on it is untouched. Section twelve applies it
/// only to a player playing ordinarily, so a listed tool in the hands of one
/// building without consuming anything is not ruined at all.
#[must_use]
pub fn ineffective(
    settings: &Settings,
    entry: &str,
    shape: Shape,
    playing: Playing,
    member_of: &dyn Fn(&str) -> bool,
) -> bool {
    playing == Playing::Ordinarily
        && (shape.tiered() || matches!(shape, Shape::Ranged))
        && settings.tweak_ineffective.matches(entry, member_of)
}

/// What an ineffective tool is refused, once a player is holding it ordinarily.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ruined {
    /// The mining speed such a tool has, which is none.
    pub mining_speed: f64,
    /// Whether an attack it makes is refused.
    pub attack_refused: bool,
    /// Whether using it on a block, in the air or on an entity is refused.
    pub use_refused: bool,
    /// Whether the shared library's crop-harvest event is cancelled.
    pub harvest_refused: bool,
}

impl Ruined {
    /// The speed a tool the rule has ruined mines at.
    pub const NO_SPEED: f64 = 0.0;

    /// The multiplier a tool the rule leaves alone mines at.
    pub const FULL_SPEED: f64 = 1.0;

    /// What the rule does to a tool it has ruined.
    #[must_use]
    pub const fn all() -> Self {
        Self {
            mining_speed: Self::NO_SPEED,
            attack_refused: true,
            use_refused: true,
            harvest_refused: true,
        }
    }

    /// What the rule does to a ruined tool in this player's hands.
    ///
    /// Nothing at all when they are building without consuming anything, which
    /// is section twelve's exemption and applies to every one of the four
    /// effects rather than to the damage cancellation alone.
    #[must_use]
    pub const fn of(playing: Playing) -> Self {
        match playing {
            Playing::Ordinarily => Self::all(),
            Playing::WithoutConsuming => Self {
                mining_speed: Self::FULL_SPEED,
                attack_refused: false,
                use_refused: false,
                harvest_refused: false,
            },
        }
    }
}

/// What is hurting an entity, read as a kind rather than as a message name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Hurt {
    /// A player swinging something.
    PlayerAttack,
    /// Anything else, which the rule never cancels.
    Otherwise,
}

/// Whether an ineffective tool's damage event is cancelled.
///
/// The original compares the damage source's message identifier against the
/// string for a player attack. The typed source is read here instead, which is
/// the same decision without a string that a host rename would break. What is
/// reproduced is the scope: the whole damage event is cancelled, not this tool's
/// contribution to it.
#[must_use]
pub const fn cancels_damage(hurt: Hurt, ruined: bool, playing: Playing) -> bool {
    ruined && matches!(playing, Playing::Ordinarily) && matches!(hurt, Hurt::PlayerAttack)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matching::Matcher;

    fn untagged(_tag: &str) -> bool {
        false
    }

    #[test]
    fn the_last_two_points_of_durability_are_the_ones_refused() {
        assert!(!nearly_destroyed(97, 100));
        assert!(nearly_destroyed(98, 100));
        assert!(nearly_destroyed(99, 100));
        assert!(nearly_destroyed(100, 100));
    }

    #[test]
    fn an_item_with_a_maximum_of_two_or_less_is_exempt() {
        for maximum in 0..=RESERVE {
            assert!(
                !nearly_destroyed(maximum, maximum),
                "a maximum of {maximum} is exempt"
            );
        }
        assert!(nearly_destroyed(1, 3));
    }

    #[test]
    fn five_surfaces_watch_three_shapes_and_two_watch_their_own() {
        for surface in [
            Surface::BreakSpeed,
            Surface::BreakBlock,
            Surface::AttackEntity,
            Surface::UseOnBlock,
            Surface::CropHarvest,
        ] {
            assert!(surface.watches(Shape::Digging));
            assert!(surface.watches(Shape::Sword));
            assert!(surface.watches(Shape::Shearing));
            assert!(!surface.watches(Shape::Ranged));
        }
        assert!(Surface::UseInAir.watches(Shape::Shearing));
        assert!(Surface::UseInAir.watches(Shape::Ranged));
        assert!(!Surface::UseInAir.watches(Shape::Digging));
        assert!(Surface::InteractEntity.watches(Shape::Shearing));
        assert!(!Surface::InteractEntity.watches(Shape::Sword));
        assert_eq!(Surface::ALL.len(), 7);
    }

    #[test]
    fn a_player_building_without_consuming_anything_is_never_refused() {
        let settings = Settings::default();
        assert!(refuses(
            &settings,
            Surface::BreakBlock,
            Shape::Digging,
            99,
            100,
            Playing::Ordinarily
        ));
        assert!(!refuses(
            &settings,
            Surface::BreakBlock,
            Shape::Digging,
            99,
            100,
            Playing::WithoutConsuming
        ));
    }

    #[test]
    fn switching_the_rule_off_stops_every_surface_refusing() {
        let settings = Settings {
            tweak_nearly_destroyed: false,
            ..Settings::default()
        };
        for surface in Surface::ALL {
            assert!(!refuses(
                &settings,
                surface,
                Shape::Shearing,
                99,
                100,
                Playing::Ordinarily
            ));
        }
    }

    #[test]
    fn the_ineffective_rule_is_governed_only_by_its_list_being_non_empty() {
        let empty = Settings::default();
        assert!(!ineffective(
            &empty,
            "example:pick",
            Shape::Digging,
            Playing::Ordinarily,
            &untagged
        ));

        let listed = Settings {
            tweak_ineffective: Matcher::parse("example:pick"),
            ..Settings::default()
        };
        assert!(ineffective(
            &listed,
            "example:pick",
            Shape::Digging,
            Playing::Ordinarily,
            &untagged
        ));
        assert!(!ineffective(
            &listed,
            "example:shovel",
            Shape::Digging,
            Playing::Ordinarily,
            &untagged
        ));
    }

    #[test]
    fn only_a_tiered_ranged_or_shearing_shape_can_be_made_ineffective() {
        let listed = Settings {
            tweak_ineffective: Matcher::parse("example:*"),
            ..Settings::default()
        };
        for shape in [Shape::Digging, Shape::Sword, Shape::Shearing, Shape::Ranged] {
            assert!(
                ineffective(
                    &listed,
                    "example:thing",
                    shape,
                    Playing::Ordinarily,
                    &untagged
                ),
                "{shape:?} can be ruined"
            );
        }
        for shape in [Shape::Armour, Shape::Other] {
            assert!(
                !ineffective(
                    &listed,
                    "example:thing",
                    shape,
                    Playing::Ordinarily,
                    &untagged
                ),
                "{shape:?} cannot"
            );
        }
    }

    #[test]
    fn a_ruined_tool_mines_at_nothing_and_is_refused_every_use() {
        let ruined = Ruined::all();
        assert!((ruined.mining_speed - Ruined::NO_SPEED).abs() < f64::EPSILON);
        assert!(ruined.attack_refused);
        assert!(ruined.use_refused);
        assert!(ruined.harvest_refused);
    }

    #[test]
    fn a_player_building_without_consuming_is_exempt_from_all_four_effects() {
        let listed = Settings {
            tweak_ineffective: Matcher::parse("example:*"),
            ..Settings::default()
        };
        assert!(!ineffective(
            &listed,
            "example:pick",
            Shape::Digging,
            Playing::WithoutConsuming,
            &untagged
        ));

        let exempt = Ruined::of(Playing::WithoutConsuming);
        assert!((exempt.mining_speed - Ruined::FULL_SPEED).abs() < f64::EPSILON);
        assert!(!exempt.attack_refused);
        assert!(!exempt.use_refused);
        assert!(!exempt.harvest_refused);
        assert_eq!(Ruined::of(Playing::Ordinarily), Ruined::all());
    }

    #[test]
    fn only_a_player_attack_from_a_ruined_tool_cancels_a_damage_event() {
        assert!(cancels_damage(
            Hurt::PlayerAttack,
            true,
            Playing::Ordinarily
        ));
        assert!(!cancels_damage(Hurt::Otherwise, true, Playing::Ordinarily));
        assert!(!cancels_damage(
            Hurt::PlayerAttack,
            false,
            Playing::Ordinarily
        ));
        assert!(!cancels_damage(
            Hurt::PlayerAttack,
            true,
            Playing::WithoutConsuming
        ));
    }
}
