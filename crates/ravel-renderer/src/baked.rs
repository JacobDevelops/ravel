//! Owned vanilla baked-model inputs and Rust-emitted shaded quads.

use crate::{MAX_GENERATION, MAX_STATE_KEY, mesh::RenderLayer};
use num_traits::ToPrimitive;
use std::mem::{size_of, size_of_val};

/// Sections exceeding this fixed input budget are unsupported, not truncated.
pub const MAX_QUADS: usize = 65_536;
/// Maximum retained input vector payload, including spare capacity.
pub const MAX_INPUT_BYTES: usize = MAX_QUADS * size_of::<QuadInput>();
/// Maximum retained emitted quad payload across all layers.
pub const MAX_OUTPUT_BYTES: usize = MAX_QUADS * size_of::<ShadedQuad>();

/// Exact model input words use the reviewed BLOCK format: XYZ, color, UV, light.
///
/// Color is a native little-endian `IntBuffer` word (numeric `0xAABBGGRR`).
/// Floats are retained as bits and validated before admission. Inputs are the
/// actual face-culled quads, not the unculled model's complete quad list.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuadInput {
    /// Opaque adapter state identity, not a model registry id.
    pub state_key: u32,
    /// Atlas sprite identity scoped to this resource generation.
    pub sprite: u32,
    /// Vanilla destination layer for the face-culled model quad.
    pub layer: RenderLayer,
    /// Four exact model vertices, each seven native little-endian BLOCK words.
    pub vertices: [[u32; 7]; 4],
    /// Effective vanilla postPosition offset, including the builder's section translation.
    /// Each f64 sum is rounded to f32 before addition to the model coordinate.
    pub offset: [u64; 3],
    /// Independently captured block-color provider result, before AO multiplication.
    pub tint: u32,
    /// Independently captured per-corner AO brightness, as f32 bits.
    pub ambient_occlusion: [u32; 4],
    /// Exact per-vertex RGB multipliers supplied to vanilla's `BufferBuilder`.
    pub multipliers: [[u32; 3]; 4],
    /// Exact per-vertex packed lightmap coordinates supplied to vanilla.
    pub combined_light: [u32; 4],
}

/// Explicit admission/emission failures; none authorizes drawing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BakedError {
    /// Snapshot, atlas or active resource generations do not agree.
    Generation,
    /// Retained input exceeds its explicit memory bound.
    Limit,
    /// Adapter state identity is outside the neutral representation.
    State,
    /// Model, shading, offset, camera or transformed position is nonfinite.
    Nonfinite,
    /// Bounded output storage could not be reserved.
    Allocation,
    /// Cancellation was observed before output publication.
    Cancelled,
}

impl std::fmt::Display for BakedError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Generation => "baked model and atlas generations do not match",
            Self::Limit => "baked geometry exceeds the retained memory budget",
            Self::State => "baked geometry contains an invalid adapter state identity",
            Self::Nonfinite => "baked geometry contains a nonfinite coordinate or shading value",
            Self::Allocation => "cannot allocate baked geometry storage",
            Self::Cancelled => "baked geometry work was cancelled",
        })
    }
}

impl std::error::Error for BakedError {}

/// A model and its sprite identities belong to the same resource generation.
#[derive(Debug, Eq, PartialEq)]
pub struct BakedSnapshot {
    generation: u64,
    quads: Vec<QuadInput>,
}

/// Rust-emitted BLOCK words plus preserved, independently captured provenance.
#[derive(Debug, Eq, PartialEq)]
pub struct ShadedQuad {
    /// Original adapter state identity.
    pub state_key: u32,
    /// Original generation-scoped atlas sprite identity.
    pub sprite: u32,
    /// Rust-emitted BLOCK words, preserving raw model UVs.
    pub vertices: [[u32; 7]; 4],
    /// Original independent tint provenance.
    pub tint: u32,
    /// Original independent per-corner AO provenance.
    pub ambient_occlusion: [u32; 4],
    ordinal: u32,
    distance: u32,
}

/// Retained Rust-emitted model quads, separated into vanilla's four render layers.
#[derive(Debug, Eq, PartialEq)]
pub struct BakedMesh {
    generation: u64,
    layers: [Vec<ShadedQuad>; 4],
}

impl BakedSnapshot {
    /// Accepts only owned data; no game/model/atlas object can escape its owner.
    ///
    /// # Errors
    /// The retained Vec capacity, not only its length, must fit `MAX_QUADS`.
    /// Rejects mixed generations, excessive input, invalid states or nonfinite values.
    pub fn new(
        generation: u64,
        atlas_generation: u64,
        quads: Vec<QuadInput>,
    ) -> Result<Self, BakedError> {
        if generation == 0 || generation > MAX_GENERATION || generation != atlas_generation {
            return Err(BakedError::Generation);
        }
        if quads.len() > MAX_QUADS || quads.capacity() > MAX_QUADS {
            return Err(BakedError::Limit);
        }
        for quad in &quads {
            validate(quad)?;
        }
        Ok(Self { generation, quads })
    }

    /// Resource generation shared by all model and atlas inputs.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Retained input vector payload, including spare capacity.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.quads.capacity() * size_of::<QuadInput>()
    }

    /// Emits the supplied model geometry, never a cube approximation.
    ///
    /// # Errors
    /// Camera coordinates must be section-relative, computed exactly as vanilla:
    /// `(camera_world_f32 as f64 + builder_offset_f64) as f32`.
    /// Fluid geometry is not admitted by this baked-model-only contract.
    /// Rejects stale generations, nonfinite cameras/output, cancellation or allocation failure.
    pub fn mesh(
        &self,
        generation: u64,
        camera: [f32; 3],
        cancelled: impl Fn() -> bool,
    ) -> Result<BakedMesh, BakedError> {
        if generation != self.generation {
            return Err(BakedError::Generation);
        }
        if !camera.into_iter().all(f32::is_finite) {
            return Err(BakedError::Nonfinite);
        }
        check_cancel(&cancelled)?;
        let mut layers: [Vec<ShadedQuad>; 4] = std::array::from_fn(|_| Vec::new());
        let mut counts = [0; 4];
        for quad in &self.quads {
            counts[layer_index(quad.layer)] += 1;
        }
        for (layer, count) in layers.iter_mut().zip(counts) {
            check_cancel(&cancelled)?;
            layer
                .try_reserve_exact(count)
                .map_err(|_| BakedError::Allocation)?;
        }
        for (ordinal, quad) in self.quads.iter().enumerate() {
            check_cancel(&cancelled)?;
            layers[layer_index(quad.layer)].push(emit(quad, ordinal)?);
        }
        sort_translucent(&mut layers[3], camera, &cancelled)?;
        check_cancel(&cancelled)?;
        Ok(BakedMesh { generation, layers })
    }
}

impl BakedMesh {
    /// Reorders only retained translucent quads, without allocations or re-emission.
    ///
    /// # Errors
    /// Rejects stale generations, nonfinite camera/distance values and cancellation.
    /// A failed or cancelled resort must not be drawn.
    pub fn resort(
        &mut self,
        generation: u64,
        camera: [f32; 3],
        cancelled: impl Fn() -> bool,
    ) -> Result<(), BakedError> {
        if generation != self.generation {
            return Err(BakedError::Generation);
        }
        if !camera.into_iter().all(f32::is_finite) {
            return Err(BakedError::Nonfinite);
        }
        sort_translucent(&mut self.layers[3], camera, &cancelled)
    }

    /// Resource generation required before any submission or resort.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Vanilla layer order: solid, cutout-mipped, cutout, translucent.
    #[must_use]
    pub fn layers(&self) -> &[Vec<ShadedQuad>; 4] {
        &self.layers
    }

    /// Retained quad vector payload, including spare capacity.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.layers
            .iter()
            .map(|layer| layer.capacity() * size_of::<ShadedQuad>())
            .sum()
    }

    /// Emitted BLOCK vertex bytes, excluding provenance and spare capacity.
    #[must_use]
    pub fn payload_bytes(&self) -> usize {
        self.layers
            .iter()
            .map(|layer| layer.len() * size_of_val(&[[0_u32; 7]; 4]))
            .sum()
    }
}

fn layer_index(layer: RenderLayer) -> usize {
    match layer {
        RenderLayer::Solid => 0,
        RenderLayer::CutoutMipped => 1,
        RenderLayer::Cutout => 2,
        RenderLayer::Translucent => 3,
    }
}

fn validate(quad: &QuadInput) -> Result<(), BakedError> {
    if quad.state_key > MAX_STATE_KEY {
        return Err(BakedError::State);
    }
    let vertices = quad
        .vertices
        .iter()
        .flat_map(|vertex| [vertex[0], vertex[1], vertex[2], vertex[4], vertex[5]]);
    let shading = quad
        .ambient_occlusion
        .into_iter()
        .chain(quad.multipliers.into_iter().flatten());
    if !vertices
        .chain(shading)
        .all(|bits| f32::from_bits(bits).is_finite())
        || !quad
            .offset
            .into_iter()
            .all(|bits| f64::from_bits(bits).is_finite())
    {
        return Err(BakedError::Nonfinite);
    }
    for vertex in quad.vertices {
        for (coordinate, offset) in vertex[..3].iter().zip(quad.offset) {
            let translation = f64::from_bits(offset)
                .to_f32()
                .ok_or(BakedError::Nonfinite)?;
            if !(f32::from_bits(*coordinate) + translation).is_finite() {
                return Err(BakedError::Nonfinite);
            }
        }
    }
    Ok(())
}

fn emit(quad: &QuadInput, ordinal: usize) -> Result<ShadedQuad, BakedError> {
    let mut vertices = quad.vertices;
    for (index, vertex) in vertices.iter_mut().enumerate() {
        for (coordinate, offset) in vertex[..3].iter_mut().zip(quad.offset) {
            let translation = f64::from_bits(offset)
                .to_f32()
                .ok_or(BakedError::Nonfinite)?;
            let transformed = f32::from_bits(*coordinate) + translation;
            if !transformed.is_finite() {
                return Err(BakedError::Nonfinite);
            }
            *coordinate = transformed.to_bits();
        }
        vertex[3] = multiply_color(vertex[3], quad.multipliers[index]);
        vertex[6] = quad.combined_light[index];
    }
    Ok(ShadedQuad {
        state_key: quad.state_key,
        sprite: quad.sprite,
        vertices,
        tint: quad.tint,
        ambient_occlusion: quad.ambient_occlusion,
        ordinal: u32::try_from(ordinal).map_err(|_| BakedError::Limit)?,
        distance: 0,
    })
}

fn multiply_color(color: u32, multipliers: [u32; 3]) -> u32 {
    multipliers
        .into_iter()
        .zip([0, 8, 16])
        .zip(color.to_le_bytes())
        .fold(color & 0xff00_0000, |value, ((bits, shift), component)| {
            let component = f32::from(component);
            let product = component * f32::from_bits(bits);
            let multiplied = product.to_i32().unwrap_or_else(|| {
                if product.is_sign_negative() {
                    i32::MIN
                } else {
                    i32::MAX
                }
            });
            value | multiplied.cast_unsigned().wrapping_shl(shift)
        })
}

fn sort_translucent(
    quads: &mut [ShadedQuad],
    camera: [f32; 3],
    cancelled: &impl Fn() -> bool,
) -> Result<(), BakedError> {
    for quad in quads.iter_mut() {
        check_cancel(cancelled)?;
        let value = distance(quad, camera);
        if !value.is_finite() {
            return Err(BakedError::Nonfinite);
        }
        quad.distance = value.to_bits();
    }
    for root in (0..quads.len().div_euclid(2)).rev() {
        sift(quads, root, quads.len(), cancelled)?;
    }
    for end in (1..quads.len()).rev() {
        check_cancel(cancelled)?;
        quads.swap(0, end);
        sift(quads, 0, end, cancelled)?;
    }
    check_cancel(cancelled)
}

fn sift(
    quads: &mut [ShadedQuad],
    mut root: usize,
    end: usize,
    cancelled: &impl Fn() -> bool,
) -> Result<(), BakedError> {
    while root * 2 + 1 < end {
        check_cancel(cancelled)?;
        let mut child = root * 2 + 1;
        if child + 1 < end && order(&quads[child], &quads[child + 1]).is_lt() {
            child += 1;
        }
        if !order(&quads[root], &quads[child]).is_lt() {
            break;
        }
        quads.swap(root, child);
        root = child;
    }
    Ok(())
}

fn order(a: &ShadedQuad, b: &ShadedQuad) -> std::cmp::Ordering {
    b.distance.cmp(&a.distance).then(a.ordinal.cmp(&b.ordinal))
}

fn distance(quad: &ShadedQuad, camera: [f32; 3]) -> f32 {
    let mut delta = [0_f32; 3];
    for axis in 0..3 {
        let v = quad.vertices.map(|vertex| f32::from_bits(vertex[axis]));
        let center = (v[0] + v[1] + v[2] + v[3]) * 0.25;
        delta[axis] = center - camera[axis];
    }
    delta.map(|value| value * value).into_iter().sum()
}

fn check_cancel(cancelled: &impl Fn() -> bool) -> Result<(), BakedError> {
    if cancelled() {
        Err(BakedError::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
