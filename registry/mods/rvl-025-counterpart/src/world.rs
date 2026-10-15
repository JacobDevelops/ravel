//! The ore of §6: three blocks that behave alike, four placements, and the two
//! placement modifiers that make the fourth one possible.
//!
//! §6 counts the placement numbers twice and states the *effective* figures
//! throughout, because one of the four shape documents the original ships is
//! named by nothing and its vein size never reaches generation. The table here
//! is the effective one, and [`Variant::declared_vein_size`] carries the number
//! the unreferenced document declares so the divergence is visible rather than
//! lost.
//!
//! The one defect §6 tells a counterpart not to copy is the modifiers reporting
//! the standard biome check as their kind. [`Modifier::kind`] reports each
//! modifier's own kind, so anything that writes a placed feature back out keeps
//! the dimension test.

/// Which of the three ore blocks a variant places.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OreBlock {
    /// The one that generates in the overworld and in other dimensions.
    Overworld,
    /// The one that generates in the nether.
    Nether,
    /// The one that generates in the end.
    End,
}

impl OreBlock {
    /// Every ore block, in the order §6 lists them.
    pub const ALL: [Self; 3] = [Self::Overworld, Self::Nether, Self::End];

    /// Returns this block's own registered name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Overworld => "surface_ore",
            Self::Nether => "sweltering_ore",
            Self::End => "outer_ore",
        }
    }
}

/// How hard every ore block is to break.
pub const HARDNESS: f32 = 3.0;
/// How well every ore block resists a blast.
pub const BLAST_RESISTANCE: f32 = 5.0;
/// How brightly every ore block glows.
pub const LIGHT_LEVEL: u8 = 7;
/// How many particles breaking one spawns on a client.
pub const PARTICLES: u8 = 10;
/// What the particle velocity's Gaussian draw is scaled by.
pub const PARTICLE_SCALE: f32 = 1.0 / 3.0;
/// The lowest count a break without a silk-touch tool drops.
pub const RAW_MINIMUM: u8 = 4;
/// The highest count a break without a silk-touch tool drops.
pub const RAW_MAXIMUM: u8 = 5;
/// The multiplier the fortune bonus is drawn with.
pub const FORTUNE_MULTIPLIER: u8 = 1;

/// One of the four placements §6 counts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Variant {
    /// The overworld itself.
    Overworld,
    /// The nether.
    Nether,
    /// The end.
    End,
    /// Every other dimension that uses overworld biomes.
    Elsewhere,
}

impl Variant {
    /// Every placement, in the order §6's table lists them.
    pub const ALL: [Self; 4] = [Self::Overworld, Self::Nether, Self::End, Self::Elsewhere];

    /// Returns the block this placement places.
    ///
    /// The fourth placement reads the overworld's shape document, so it places
    /// the overworld ore under different placement rules rather than a fourth
    /// ore.
    #[must_use]
    pub const fn block(self) -> OreBlock {
        match self {
            Self::Overworld | Self::Elsewhere => OreBlock::Overworld,
            Self::Nether => OreBlock::Nether,
            Self::End => OreBlock::End,
        }
    }

    /// Returns the vein size generation actually uses.
    #[must_use]
    pub const fn vein_size(self) -> u8 {
        match self {
            Self::Overworld | Self::Elsewhere => 5,
            Self::Nether => 8,
            Self::End => 10,
        }
    }

    /// Returns the vein size a shipped document declares for this placement.
    ///
    /// Only the fourth differs. Its own shape document declares ten and no
    /// placement document names it, so the effective figure is the overworld's
    /// five. This is §5's fifth defect read as a number rather than as prose.
    #[must_use]
    pub const fn declared_vein_size(self) -> u8 {
        match self {
            Self::Elsewhere => 10,
            other @ (Self::Overworld | Self::Nether | Self::End) => other.vein_size(),
        }
    }

    /// Returns how many veins one chunk gets.
    #[must_use]
    pub const fn veins_per_chunk(self) -> u8 {
        match self {
            Self::Overworld => 2,
            Self::Nether | Self::End => 8,
            Self::Elsewhere => 6,
        }
    }

    /// Returns the inclusive absolute height range the placement draws from.
    #[must_use]
    pub const fn height_range(self) -> (i32, i32) {
        match self {
            Self::Overworld | Self::Elsewhere => (-15, 40),
            Self::Nether => (2, 40),
            Self::End => (2, 80),
        }
    }

    /// Returns the biome tag the placement selects on.
    #[must_use]
    pub const fn biome_selector(self) -> &'static str {
        match self {
            Self::Overworld | Self::Elsewhere => "is_overworld",
            Self::Nether => "is_nether",
            Self::End => "is_end",
        }
    }

    /// Returns the replaceable tag the shape document targets.
    #[must_use]
    pub const fn replaces(self) -> &'static str {
        match self {
            Self::Overworld | Self::Elsewhere => "stone_ore_replaceables",
            Self::Nether => "netherrack_ore_replaceables",
            Self::End => "end_stone_ore_replaceables",
        }
    }

    /// Returns the placement modifier this variant wraps its biome check in.
    #[must_use]
    pub const fn modifier(self) -> Option<Modifier> {
        match self {
            Self::Overworld => Some(Modifier::OnlyOverworld),
            Self::Elsewhere => Some(Modifier::AnywhereElse),
            Self::Nether | Self::End => None,
        }
    }
}

/// One of the two placement modifiers this entry registers.
///
/// Each wraps the standard biome check with a dimension test. Together they are
/// why the fourth placement exists at all: it is attached to overworld biomes
/// and generates in every dimension using them *except* the overworld, at a
/// richer rate than the overworld gets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Modifier {
    /// Places only when the level is the overworld.
    OnlyOverworld,
    /// Places only when the level is not the overworld.
    AnywhereElse,
}

impl Modifier {
    /// Both modifiers, in the order they are registered.
    pub const ALL: [Self; 2] = [Self::OnlyOverworld, Self::AnywhereElse];

    /// Returns the kind this modifier reports itself as.
    ///
    /// Its own, which is the defect §6 says a counterpart must not copy: the
    /// original reports the standard biome check here, so anything serialising a
    /// placed feature back out loses the dimension test.
    #[must_use]
    pub const fn kind(self) -> &'static str {
        match self {
            Self::OnlyOverworld => "only_overworld",
            Self::AnywhereElse => "anywhere_else",
        }
    }

    /// Answers whether this modifier permits placement in the level given.
    #[must_use]
    pub const fn permits(self, is_overworld: bool) -> bool {
        match self {
            Self::OnlyOverworld => is_overworld,
            Self::AnywhereElse => !is_overworld,
        }
    }
}

/// What a tool that broke an ore block was.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Tool {
    /// Whether the tool preserves the block itself.
    pub silk_touch: bool,
    /// How much the tool raises the drop by.
    pub fortune: u8,
    /// Whether the tool is a pickaxe at iron tier or better.
    pub adequate: bool,
}

impl Default for Tool {
    fn default() -> Self {
        Self {
            silk_touch: false,
            fortune: 0,
            adequate: true,
        }
    }
}

/// What breaking an ore block produced.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Drop {
    /// Nothing, because the tool is not a pickaxe at iron tier or better.
    Nothing,
    /// The ore block itself, because the tool preserves it.
    Block(OreBlock),
    /// A count of raw material.
    Raw {
        /// How many.
        count: u32,
    },
}

/// Returns what breaking `block` with `tool` drops.
///
/// The draws are the caller's, so the rule is testable without a world: `base`
/// is the uniform draw over four to five inclusive, `bonus` is the uniform
/// fortune draw, and `explosion_survives` is what the decay step kept. A silk
/// touch preserves the block and skips all three.
#[must_use]
pub fn drops(block: OreBlock, tool: Tool, base: u8, bonus: u8, explosion_survives: bool) -> Drop {
    if !tool.adequate {
        return Drop::Nothing;
    }
    if tool.silk_touch {
        return Drop::Block(block);
    }
    if !explosion_survives {
        return Drop::Raw { count: 0 };
    }
    let base = u32::from(base.clamp(RAW_MINIMUM, RAW_MAXIMUM));
    let bonus = u32::from(bonus).saturating_mul(u32::from(FORTUNE_MULTIPLIER));
    let bonus = if tool.fortune == 0 { 0 } else { bonus };
    Drop::Raw {
        count: base.saturating_add(bonus),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_ore_blocks_carry_one_behaviour_between_them() {
        assert_eq!(OreBlock::ALL.len(), 3);
        let mut names: Vec<&str> = OreBlock::ALL.iter().map(|block| block.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 3);
        assert!((HARDNESS - 3.0).abs() < f32::EPSILON);
        assert!((BLAST_RESISTANCE - 5.0).abs() < f32::EPSILON);
        assert_eq!(LIGHT_LEVEL, 7);
        assert_eq!(PARTICLES, 10);
    }

    #[test]
    fn the_effective_placement_table_is_the_one_generation_reads() {
        let rows: Vec<(u8, u8, (i32, i32), &str)> = Variant::ALL
            .into_iter()
            .map(|variant| {
                (
                    variant.vein_size(),
                    variant.veins_per_chunk(),
                    variant.height_range(),
                    variant.biome_selector(),
                )
            })
            .collect();

        assert_eq!(
            rows,
            [
                (5, 2, (-15, 40), "is_overworld"),
                (8, 8, (2, 40), "is_nether"),
                (10, 8, (2, 80), "is_end"),
                (5, 6, (-15, 40), "is_overworld"),
            ]
        );
    }

    #[test]
    fn the_fourth_placement_declares_ten_and_generation_reads_five() {
        assert_eq!(Variant::Elsewhere.declared_vein_size(), 10);
        assert_eq!(Variant::Elsewhere.vein_size(), 5);
        for variant in [Variant::Overworld, Variant::Nether, Variant::End] {
            assert_eq!(
                variant.declared_vein_size(),
                variant.vein_size(),
                "only the unreferenced document differs"
            );
        }
    }

    #[test]
    fn the_fourth_placement_is_the_overworld_ore_under_different_rules() {
        assert_eq!(Variant::Elsewhere.block(), OreBlock::Overworld);
        assert_eq!(Variant::Overworld.block(), OreBlock::Overworld);
        assert_eq!(Variant::Elsewhere.replaces(), Variant::Overworld.replaces());
        assert!(Variant::Elsewhere.veins_per_chunk() > Variant::Overworld.veins_per_chunk());
    }

    #[test]
    fn the_two_modifiers_partition_every_level_between_them() {
        for is_overworld in [true, false] {
            let permitting = Modifier::ALL
                .into_iter()
                .filter(|modifier| modifier.permits(is_overworld))
                .count();
            assert_eq!(permitting, 1);
        }
        assert!(Modifier::OnlyOverworld.permits(true));
        assert!(Modifier::AnywhereElse.permits(false));
    }

    #[test]
    fn each_modifier_reports_its_own_kind_rather_than_the_check_it_wraps() {
        assert_eq!(Modifier::OnlyOverworld.kind(), "only_overworld");
        assert_eq!(Modifier::AnywhereElse.kind(), "anywhere_else");
        assert_ne!(
            Modifier::OnlyOverworld.kind(),
            Modifier::AnywhereElse.kind()
        );
    }

    #[test]
    fn the_overworld_and_fourth_placements_wrap_opposite_dimension_tests() {
        assert_eq!(Variant::Overworld.modifier(), Some(Modifier::OnlyOverworld));
        assert_eq!(Variant::Elsewhere.modifier(), Some(Modifier::AnywhereElse));
        assert_eq!(Variant::Nether.modifier(), None);
        assert_eq!(Variant::End.modifier(), None);
    }

    #[test]
    fn a_silk_touch_tool_drops_the_block_and_an_ordinary_one_drops_raw_material() {
        let silk = Tool {
            silk_touch: true,
            ..Tool::default()
        };
        assert_eq!(
            drops(OreBlock::Nether, silk, 4, 0, true),
            Drop::Block(OreBlock::Nether)
        );

        for base in [RAW_MINIMUM, RAW_MAXIMUM] {
            assert_eq!(
                drops(OreBlock::Overworld, Tool::default(), base, 0, true),
                Drop::Raw {
                    count: u32::from(base)
                }
            );
        }
    }

    #[test]
    fn a_draw_outside_four_to_five_is_clamped_into_the_declared_range() {
        assert_eq!(
            drops(OreBlock::End, Tool::default(), 0, 0, true),
            Drop::Raw {
                count: u32::from(RAW_MINIMUM)
            }
        );
        assert_eq!(
            drops(OreBlock::End, Tool::default(), u8::MAX, 0, true),
            Drop::Raw {
                count: u32::from(RAW_MAXIMUM)
            }
        );
    }

    #[test]
    fn the_fortune_bonus_applies_only_with_a_fortune_tool_and_at_multiplier_one() {
        let fortunate = Tool {
            fortune: 3,
            ..Tool::default()
        };
        assert_eq!(
            drops(OreBlock::Overworld, fortunate, 4, 3, true),
            Drop::Raw { count: 7 }
        );
        assert_eq!(
            drops(OreBlock::Overworld, Tool::default(), 4, 3, true),
            Drop::Raw { count: 4 }
        );
    }

    #[test]
    fn an_inadequate_tool_drops_nothing_and_explosion_decay_can_take_everything() {
        let bare = Tool {
            adequate: false,
            ..Tool::default()
        };
        assert_eq!(drops(OreBlock::Overworld, bare, 5, 0, true), Drop::Nothing);
        assert_eq!(
            drops(OreBlock::Overworld, Tool::default(), 5, 0, false),
            Drop::Raw { count: 0 }
        );
    }

    #[test]
    fn a_hostile_fortune_draw_saturates_rather_than_wrapping() {
        let fortunate = Tool {
            fortune: u8::MAX,
            ..Tool::default()
        };
        assert_eq!(
            drops(OreBlock::Overworld, fortunate, 5, u8::MAX, true),
            Drop::Raw {
                count: u32::from(RAW_MAXIMUM) + u32::from(u8::MAX)
            }
        );
    }
}
