#![forbid(unsafe_code)]

//! Owned section snapshots and bounded CPU geometric primitives, not a world mirror.
//!
//! The 16-cube core includes a full one-cell border. Coordinates `0..18` denote
//! section origin minus one through origin plus sixteen, in `((y * 18) + z) * 18 + x`
//! order. State keys are opaque numeric identities supplied by the adapter.
//! The geometric primitive captures only the full-opaque-cube predicate; baked
//! model inputs have a separate owned contract. Callers gate copies by generation.

use std::mem::{size_of, size_of_val};

pub mod baked;
pub mod mesh;
pub mod wire;
pub mod workers;

/// Core section edge, excluding neighbours.
pub const SECTION_EDGE: usize = 16;
/// Edge including both neighbour cells.
pub const SNAPSHOT_EDGE: usize = SECTION_EDGE + 2;
/// Exact required length of each capture input.
pub const CELL_COUNT: usize = SNAPSHOT_EDGE * SNAPSHOT_EDGE * SNAPSHOT_EDGE;
/// Largest accepted opaque adapter state identity.
pub const MAX_STATE_KEY: u32 = 0x00ff_ffff;
/// Generation must fit a positive Java long.
pub const MAX_GENERATION: u64 = 0x7fff_ffff_ffff_ffff;
/// Every cell may have its own palette entry.
pub const MAX_PALETTE_ENTRIES: usize = CELL_COUNT;
/// Worst-case retained heap payload; allocator metadata is excluded.
pub const MAX_HEAP_BYTES: usize = CELL_COUNT * (size_of::<u32>() + size_of::<u16>() + 2);
/// Retained object plus its maximum heap payload, excluding allocator metadata.
pub const MAX_TOTAL_BYTES: usize = size_of::<SectionSnapshot>() + MAX_HEAP_BYTES;

const LOOKUP_SLOTS: usize = 8192;
const EMPTY_SLOT: u16 = u16::MAX;

const _: () = {
    assert!(LOOKUP_SLOTS.is_power_of_two());
    assert!(LOOKUP_SLOTS > CELL_COUNT);
    assert!(CELL_COUNT < 65_535);
};

/// Fixed-array stack scratch payload during capture, excluding scalar locals and stack frames.
pub const MAX_CAPTURE_STACK_SCRATCH_BYTES: usize = LOOKUP_SLOTS * size_of::<u16>() + CELL_COUNT;
/// Fixed-size heap palette scratch payload during capture.
pub const MAX_CAPTURE_HEAP_SCRATCH_BYTES: usize = CELL_COUNT * size_of::<u32>();
/// Total bounded scratch payload during capture, excluding scalar locals and stack frames.
pub const MAX_CAPTURE_SCRATCH_BYTES: usize =
    MAX_CAPTURE_STACK_SCRATCH_BYTES + MAX_CAPTURE_HEAP_SCRATCH_BYTES;

/// An invalid snapshot input; rejection never retains the supplied arrays.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnapshotError {
    /// Generation is zero or cannot be represented as a positive Java long.
    InvalidGeneration,
    /// All three inputs must describe the complete core and border.
    InvalidLengths {
        /// Supplied state count.
        states: usize,
        /// Supplied packed-light count.
        light: usize,
        /// Supplied geometry-flag count.
        geometry: usize,
    },
    /// A state is negative or exceeds the adapter identity budget.
    InvalidState {
        /// Index in capture order.
        index: usize,
        /// Rejected state identity.
        state: i32,
    },
    /// Geometry must be zero (unknown) or one (captured full opaque cube).
    InvalidGeometry {
        /// Capture-order cell index.
        index: usize,
        /// Invalid flag.
        flag: u8,
    },
    /// One state key cannot have contradictory captured geometry.
    InconsistentGeometry {
        /// State with conflicting flags.
        state: u32,
    },
    /// The bounded palette cannot represent another entry.
    PaletteLimit,
}

impl std::fmt::Display for SnapshotError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidGeneration => formatter.write_str("invalid snapshot generation"),
            Self::InvalidLengths {
                states,
                light,
                geometry,
            } => write!(
                formatter,
                "snapshot lengths must be {CELL_COUNT}: states={states}, light={light}, geometry={geometry}"
            ),
            Self::InvalidState { index, state } => {
                write!(formatter, "invalid state identity {state} at cell {index}")
            }
            Self::InvalidGeometry { index, flag } => {
                write!(formatter, "invalid geometry flag {flag} at cell {index}")
            }
            Self::InconsistentGeometry { state } => {
                write!(formatter, "inconsistent geometry for state {state}")
            }
            Self::PaletteLimit => formatter.write_str("snapshot palette limit exceeded"),
        }
    }
}

impl std::error::Error for SnapshotError {}

/// Decoded value copied from a snapshot; it cannot mutate the snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Cell {
    state: u32,
    light: u8,
    full_opaque_cube: bool,
}

impl Cell {
    /// Opaque adapter state identity, without truncation or model interpretation.
    #[must_use]
    pub fn state_key(self) -> u32 {
        self.state
    }

    /// Captured full-opaque predicate, with air key zero unconditionally empty.
    #[must_use]
    pub fn is_full_opaque_cube(self) -> bool {
        self.state != 0 && self.full_opaque_cube
    }

    /// Sky light in the upper nibble, block light in the lower nibble.
    #[must_use]
    pub fn packed_light(self) -> u8 {
        self.light
    }

    /// Sky-light level from zero to fifteen.
    #[must_use]
    pub fn sky_light(self) -> u8 {
        self.light >> 4
    }

    /// Block-light level from zero to fifteen.
    #[must_use]
    pub fn block_light(self) -> u8 {
        self.light & 15
    }
}

/// Immutable local palette, compact cell indices and packed light, with no game references.
#[derive(Debug, Eq, PartialEq)]
pub struct SectionSnapshot {
    generation: u64,
    palette: Box<[u32]>,
    indices: Box<[u16]>,
    light: Box<[u8]>,
    geometry: Box<[u8]>,
}

impl SectionSnapshot {
    /// Copies a complete bordered section. Geometry flags are captured game
    /// `isOpaqueCube && isFullCube` predicates (zero/one), consistent per state.
    /// Air key zero is always treated as empty by the mesh primitive.
    ///
    /// # Errors
    ///
    /// Rejects invalid generations, incomplete arrays, out-of-range states, invalid
    /// or contradictory geometry flags, and palettes exceeding the bounded capacity.
    pub fn capture(
        generation: u64,
        states: &[i32],
        light: &[u8],
        geometry: &[u8],
    ) -> Result<Self, SnapshotError> {
        if generation == 0 || generation > MAX_GENERATION {
            return Err(SnapshotError::InvalidGeneration);
        }
        if states.len() != CELL_COUNT || light.len() != CELL_COUNT || geometry.len() != CELL_COUNT {
            return Err(SnapshotError::InvalidLengths {
                states: states.len(),
                light: light.len(),
                geometry: geometry.len(),
            });
        }
        let mut palette = vec![0; MAX_PALETTE_ENTRIES].into_boxed_slice();
        let mut indices = vec![0; CELL_COUNT].into_boxed_slice();
        let mut slots = [EMPTY_SLOT; LOOKUP_SLOTS];
        let mut geometry_palette = [2; CELL_COUNT];
        let mut palette_len = 0;
        for (index, &state) in states.iter().enumerate() {
            let key = u32::try_from(state)
                .ok()
                .filter(|&key| key <= MAX_STATE_KEY)
                .ok_or(SnapshotError::InvalidState { index, state })?;
            indices[index] = intern(key, &mut palette, &mut palette_len, &mut slots)?;
            let flag = geometry[index];
            if flag > 1 {
                return Err(SnapshotError::InvalidGeometry { index, flag });
            }
            let entry = &mut geometry_palette[usize::from(indices[index])];
            if *entry != 2 && *entry != flag {
                return Err(SnapshotError::InconsistentGeometry { state: key });
            }
            *entry = flag;
        }
        Ok(Self {
            generation,
            palette: Box::from(&palette[..palette_len]),
            indices,
            light: Box::from(light),
            geometry: Box::from(&geometry_palette[..palette_len]),
        })
    }

    /// Resource/world generation supplied at capture.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Number of deduplicated state/model identities.
    #[must_use]
    pub fn palette_len(&self) -> usize {
        self.palette.len()
    }

    /// Decodes a linear cell index; out-of-range indices return `None`.
    #[must_use]
    pub fn cell(&self, index: usize) -> Option<Cell> {
        let &palette_index = self.indices.get(index)?;
        Some(Cell {
            state: self.palette[usize::from(palette_index)],
            light: self.light[index],
            full_opaque_cube: self.geometry[usize::from(palette_index)] == 1,
        })
    }

    /// Decodes bordered coordinates; all faces, edges and corners are preserved.
    #[must_use]
    pub fn cell_at(&self, x: usize, y: usize, z: usize) -> Option<Cell> {
        if x >= SNAPSHOT_EDGE || y >= SNAPSHOT_EDGE || z >= SNAPSHOT_EDGE {
            return None;
        }
        self.cell(((y * SNAPSHOT_EDGE) + z) * SNAPSHOT_EDGE + x)
    }

    /// Exact retained heap payload, excluding allocator metadata.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        size_of_val(self.palette.as_ref())
            + size_of_val(self.indices.as_ref())
            + size_of_val(self.light.as_ref())
            + size_of_val(self.geometry.as_ref())
    }

    /// Object size plus retained heap payload, excluding allocator metadata.
    #[must_use]
    pub fn total_bytes(&self) -> usize {
        size_of::<Self>() + self.heap_bytes()
    }
}

fn intern(
    key: u32,
    palette: &mut [u32],
    palette_len: &mut usize,
    slots: &mut [u16; LOOKUP_SLOTS],
) -> Result<u16, SnapshotError> {
    let bytes = key.wrapping_mul(0x9e37_79b9).to_le_bytes();
    let mut slot = usize::from(u16::from_le_bytes([bytes[2], bytes[3]]) >> 3);
    for _ in 0..CELL_COUNT {
        let entry = slots[slot];
        if entry == EMPTY_SLOT {
            let index = u16::try_from(*palette_len).map_err(|_| SnapshotError::PaletteLimit)?;
            let target = palette
                .get_mut(*palette_len)
                .ok_or(SnapshotError::PaletteLimit)?;
            *target = key;
            *palette_len += 1;
            slots[slot] = index;
            return Ok(index);
        }
        if palette[usize::from(entry)] == key {
            return Ok(entry);
        }
        slot = (slot + 1) & (LOOKUP_SLOTS - 1);
    }
    Err(SnapshotError::PaletteLimit)
}

#[cfg(test)]
mod tests;
