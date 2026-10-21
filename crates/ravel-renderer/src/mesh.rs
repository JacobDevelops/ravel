//! CPU geometric cube primitives from captured full-opaque predicates, not baked models.
//!
//! No textures, UVs, ambient occlusion or faithful game lighting are inferred from keys.
//! Unknown states emit no primitive, never occlude, and make the result incomplete.
//! Unsupported keys include the entire snapshot border, even diagonal neighbours:
//! such a key conservatively marks the result incomplete without hiding geometry.

use crate::{CELL_COUNT, Cell, SectionSnapshot};
use std::mem::{size_of, size_of_val};
use std::ops::Range;

const CORE_CELLS: usize = 4096;
/// Maximum emitted faces, before neighbour culling, across all core cells.
pub const MAX_FACES: usize = CORE_CELLS * 6;
/// Maximum retained object plus heap payload; allocator bookkeeping is excluded.
pub const MAX_MESH_BYTES: usize = size_of::<MeshResult>()
    + MAX_FACES
        * (4 * size_of::<Vertex>()
            + 6 * size_of::<u32>()
            + size_of::<FaceRange>()
            + size_of::<MaterialRange>())
    + CELL_COUNT * size_of::<u32>();
/// Four solid-layer vectors and one unsupported-key vector; empty vectors do not allocate.
pub const MAX_MESH_ALLOCATIONS: usize = 5;
/// Flood queue and visited fixed-array payload; excludes call frames.
pub const MAX_MESH_STACK_SCRATCH_BYTES: usize = CORE_CELLS * 3;

/// Stable visibility order: west, east, down, up, north, south.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum Face {
    /// Negative X boundary.
    West,
    /// Positive X boundary.
    East,
    /// Negative Y boundary.
    Down,
    /// Positive Y boundary.
    Up,
    /// Negative Z boundary.
    North,
    /// Positive Z boundary.
    South,
}

impl Face {
    /// Stable wire/visibility index, in declaration order.
    #[must_use]
    pub fn code(self) -> u8 {
        match self {
            Self::West => 0,
            Self::East => 1,
            Self::Down => 2,
            Self::Up => 3,
            Self::North => 4,
            Self::South => 5,
        }
    }
}

const FACES: [Face; 6] = [
    Face::West,
    Face::East,
    Face::Down,
    Face::Up,
    Face::North,
    Face::South,
];
const CORNERS: [[[u8; 3]; 4]; 6] = [
    [[0, 0, 0], [0, 0, 1], [0, 1, 1], [0, 1, 0]],
    [[1, 0, 1], [1, 0, 0], [1, 1, 0], [1, 1, 1]],
    [[0, 0, 1], [0, 0, 0], [1, 0, 0], [1, 0, 1]],
    [[0, 1, 0], [0, 1, 1], [1, 1, 1], [1, 1, 0]],
    [[1, 0, 0], [0, 0, 0], [0, 1, 0], [1, 1, 0]],
    [[0, 0, 1], [1, 0, 1], [1, 1, 1], [0, 1, 1]],
];

/// One local integer cube corner; all vertices of a face share the adjacent cell's raw light.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Vertex {
    position: [u8; 3],
    face: Face,
    packed_light: u8,
}
impl Vertex {
    /// Section-local integer position, including corners on coordinate 16.
    #[must_use]
    pub fn position(self) -> [u8; 3] {
        self.position
    }
    /// Outward face normal and winding identity.
    #[must_use]
    pub fn face(self) -> Face {
        self.face
    }
    /// Uninterpolated sky-high/block-low nibbles, not a baked lightmap coordinate or AO.
    #[must_use]
    pub fn packed_light(self) -> u8 {
        self.packed_light
    }
}

/// Contiguous index range for one captured state identity; no texture handle is implied.
#[derive(Debug, Eq, PartialEq)]
pub struct MaterialRange {
    state_key: u32,
    indices: Range<u32>,
}
impl MaterialRange {
    /// Opaque adapter state identity for this material range.
    #[must_use]
    pub fn state_key(&self) -> u32 {
        self.state_key
    }
    /// Half-open range in the layer's index buffer.
    #[must_use]
    pub fn indices(&self) -> Range<u32> {
        self.indices.clone()
    }
}

/// Individually sortable quad range, in the same index buffer as its material.
#[derive(Debug, Eq, PartialEq)]
pub struct FaceRange {
    state_key: u32,
    origin: [u8; 3],
    face: Face,
    first_index: u32,
}
impl FaceRange {
    /// Opaque adapter state identity for this quad.
    #[must_use]
    pub fn state_key(&self) -> u32 {
        self.state_key
    }
    /// Outward normal for this quad.
    #[must_use]
    pub fn face(&self) -> Face {
        self.face
    }
    /// Six contiguous triangle indices for this quad.
    #[must_use]
    pub fn indices(&self) -> Range<u32> {
        self.first_index..self.first_index + 6
    }
    /// Doubled section-local centre, avoiding floating-point sorting ambiguity.
    #[must_use]
    pub fn centre_twice(&self) -> [u8; 3] {
        let corners = CORNERS[usize::from(self.face.code())];
        std::array::from_fn(|axis| self.origin[axis] * 2 + corners[0][axis] + corners[2][axis])
    }
}

/// Vanilla layer order. Only solid is populated by the full-opaque geometric contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderLayer {
    /// Depth-writing opaque layer.
    Solid,
    /// Alpha-tested layer using atlas mipmaps.
    CutoutMipped,
    /// Alpha-tested layer without atlas mipmaps.
    Cutout,
    /// Blended layer requiring camera-dependent ordering.
    Translucent,
}

/// Owned full-opaque geometric buffers; not a textured baked-model representation.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct LayerBuffers {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    materials: Vec<MaterialRange>,
    faces: Vec<FaceRange>,
}
impl LayerBuffers {
    /// Cube corners in emission order.
    #[must_use]
    pub fn vertices(&self) -> &[Vertex] {
        &self.vertices
    }
    /// Triangle indices into `vertices`.
    #[must_use]
    pub fn indices(&self) -> &[u32] {
        &self.indices
    }
    /// Contiguous material ranges covering the index buffer.
    #[must_use]
    pub fn materials(&self) -> &[MaterialRange] {
        &self.materials
    }
    /// Quad boundaries retained for face-level comparisons.
    #[must_use]
    pub fn faces(&self) -> &[FaceRange] {
        &self.faces
    }
    fn heap_bytes(&self) -> usize {
        size_of_val(self.vertices.as_slice())
            + size_of_val(self.indices.as_slice())
            + size_of_val(self.materials.as_slice())
            + size_of_val(self.faces.as_slice())
    }
    fn allocations(&self) -> usize {
        [
            !self.vertices.is_empty(),
            !self.indices.is_empty(),
            !self.materials.is_empty(),
            !self.faces.is_empty(),
        ]
        .into_iter()
        .filter(|value| *value)
        .count()
    }
}

/// Symmetric 6x6 matrix, bit `from * 6 + to`; diagonals mean a face touches open space.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Visibility(u64);
impl Visibility {
    /// Raw row-major 6-by-6 visibility matrix.
    #[must_use]
    pub fn bits(self) -> u64 {
        self.0
    }
    /// Whether open space connects the supplied section boundaries.
    #[must_use]
    pub fn connected(self, from: Face, to: Face) -> bool {
        self.0 & (1 << (u32::from(from.code()) * 6 + u32::from(to.code()))) != 0
    }
    fn connect(&mut self, faces: u8) {
        for from in 0..6 {
            for to in 0..6 {
                if faces & (1 << from) != 0 && faces & (1 << to) != 0 {
                    self.0 |= 1 << (from * 6 + to);
                }
            }
        }
    }
}

/// Immutable owned worker output; layer array follows `RenderLayer` declaration order.
#[derive(Debug, Eq, PartialEq)]
pub struct MeshResult {
    generation: u64,
    job_id: u64,
    layers: [LayerBuffers; 4],
    unsupported: Vec<u32>,
    visibility: Visibility,
}
impl MeshResult {
    /// Resource/world generation captured by this mesh's snapshot.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }
    /// Caller-supplied job identity, not an adapter state identity.
    #[must_use]
    pub fn job_id(&self) -> u64 {
        self.job_id
    }
    /// Layer buffers in vanilla declaration order.
    #[must_use]
    pub fn layers(&self) -> &[LayerBuffers; 4] {
        &self.layers
    }
    /// Distinct nonzero states whose captured geometry is unsupported.
    #[must_use]
    pub fn unsupported_keys(&self) -> &[u32] {
        &self.unsupported
    }
    /// Whether every captured state fits the full-opaque geometric contract.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.unsupported.is_empty()
    }
    /// Open-space reachability derived from the captured opaque predicates.
    #[must_use]
    pub fn visibility(&self) -> Visibility {
        self.visibility
    }
    /// Retained vector payload bytes, excluding this result's object.
    pub fn heap_bytes(&self) -> usize {
        self.layers
            .iter()
            .map(LayerBuffers::heap_bytes)
            .sum::<usize>()
            + size_of_val(self.unsupported.as_slice())
    }
    /// Result object and retained vector payload bytes.
    #[must_use]
    pub fn total_bytes(&self) -> usize {
        size_of::<Self>() + self.heap_bytes()
    }
    /// Number of nonempty retained vector allocations.
    pub fn allocations(&self) -> usize {
        self.layers
            .iter()
            .map(LayerBuffers::allocations)
            .sum::<usize>()
            + usize::from(!self.unsupported.is_empty())
    }
}

/// Explicit failures; no failed mesh may be drawn.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeshError {
    /// Generation invalidation or caller cancellation.
    Cancelled,
    /// A bounded allocation could not be reserved.
    Allocation,
    /// Geometry exceeds the fixed output representation.
    OutputLimit,
    /// A requested cell lies outside the captured border.
    InvalidCoordinates,
}

impl std::fmt::Display for MeshError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Cancelled => "mesh generation was cancelled",
            Self::Allocation => "mesh buffer allocation failed",
            Self::OutputLimit => "mesh output budget exceeded",
            Self::InvalidCoordinates => "mesh coordinates are outside the captured border",
        })
    }
}

impl std::error::Error for MeshError {}

/// Counts before allocating. Cell loops check cancellation every 64 visits.
/// Bounded sorts, material counting and each allocation have checks on both sides.
///
/// # Errors
///
/// Returns `Cancelled` when cancellation is observed, `Allocation` when a bounded
/// buffer cannot be reserved, `OutputLimit` when output exceeds its representation,
/// or `InvalidCoordinates` if an internal position is outside the captured border.
pub fn mesh(
    snapshot: &SectionSnapshot,
    job_id: u64,
    cancel: impl Fn() -> bool,
) -> Result<MeshResult, MeshError> {
    let (face_count, unsupported) = count(snapshot, &cancel)?;
    check_cancel(&cancel)?;
    let mut solid = LayerBuffers {
        faces: reserve(face_count)?,
        ..LayerBuffers::default()
    };
    check_cancel(&cancel)?;
    visit_faces(snapshot, &cancel, |origin, face, state_key| {
        solid.faces.push(FaceRange {
            state_key,
            origin,
            face,
            first_index: 0,
        });
    })?;
    check_cancel(&cancel)?;
    solid
        .faces
        .sort_unstable_by_key(|face| (face.state_key, face.origin, face.face));
    check_cancel(&cancel)?;
    let material_count = solid
        .faces
        .iter()
        .enumerate()
        .filter(|(index, face)| *index == 0 || solid.faces[*index - 1].state_key != face.state_key)
        .count();
    check_cancel(&cancel)?;
    solid.vertices = reserve(face_count * 4)?;
    check_cancel(&cancel)?;
    solid.indices = reserve(face_count * 6)?;
    check_cancel(&cancel)?;
    solid.materials = reserve(material_count)?;
    check_cancel(&cancel)?;
    emit(snapshot, &mut solid, &cancel)?;
    let visibility = visibility(snapshot, &cancel)?;
    check_cancel(&cancel)?;
    Ok(MeshResult {
        generation: snapshot.generation(),
        job_id,
        layers: [
            solid,
            LayerBuffers::default(),
            LayerBuffers::default(),
            LayerBuffers::default(),
        ],
        unsupported,
        visibility,
    })
}

fn reserve<T>(count: usize) -> Result<Vec<T>, MeshError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| MeshError::Allocation)?;
    Ok(values)
}
fn check_cancel(cancel: &impl Fn() -> bool) -> Result<(), MeshError> {
    if cancel() {
        Err(MeshError::Cancelled)
    } else {
        Ok(())
    }
}
fn opaque(cell: Cell) -> bool {
    cell.state_key() != 0 && cell.is_full_opaque_cube()
}
fn cell(snapshot: &SectionSnapshot, p: [u8; 3]) -> Result<Cell, MeshError> {
    snapshot
        .cell_at(usize::from(p[0]), usize::from(p[1]), usize::from(p[2]))
        .ok_or(MeshError::InvalidCoordinates)
}
fn neighbour(mut p: [u8; 3], face: Face) -> [u8; 3] {
    match face {
        Face::West => p[0] -= 1,
        Face::East => p[0] += 1,
        Face::Down => p[1] -= 1,
        Face::Up => p[1] += 1,
        Face::North => p[2] -= 1,
        Face::South => p[2] += 1,
    }
    p
}
fn core_position(index: usize) -> [u8; 3] {
    [
        index.to_le_bytes()[0] & 15,
        (index >> 8).to_le_bytes()[0],
        (index >> 4).to_le_bytes()[0] & 15,
    ]
}
fn bordered(origin: [u8; 3]) -> [u8; 3] {
    origin.map(|axis| axis + 1)
}

fn visit_faces(
    snapshot: &SectionSnapshot,
    cancel: &impl Fn() -> bool,
    mut visit: impl FnMut([u8; 3], Face, u32),
) -> Result<(), MeshError> {
    for index in 0..CORE_CELLS {
        if index % 64 == 0 {
            check_cancel(cancel)?;
        }
        let origin = core_position(index);
        let position = bordered(origin);
        let current = cell(snapshot, position)?;
        if opaque(current) {
            for face in FACES {
                if !opaque(cell(snapshot, neighbour(position, face))?) {
                    visit(origin, face, current.state_key());
                }
            }
        }
    }
    Ok(())
}

fn count(
    snapshot: &SectionSnapshot,
    cancel: &impl Fn() -> bool,
) -> Result<(usize, Vec<u32>), MeshError> {
    let mut distinct = 0;
    for (index, (&state, &geometry)) in snapshot
        .palette
        .iter()
        .zip(snapshot.geometry.iter())
        .enumerate()
    {
        if index % 64 == 0 {
            check_cancel(cancel)?;
        }
        if state != 0 && geometry == 0 {
            distinct += 1;
        }
    }
    let mut faces = 0;
    visit_faces(snapshot, cancel, |_, _, _| faces += 1)?;
    let bytes = size_of::<MeshResult>()
        + faces
            * (4 * size_of::<Vertex>()
                + 6 * size_of::<u32>()
                + size_of::<FaceRange>()
                + size_of::<MaterialRange>())
        + distinct * size_of::<u32>();
    if faces > MAX_FACES || bytes > MAX_MESH_BYTES {
        return Err(MeshError::OutputLimit);
    }
    check_cancel(cancel)?;
    let mut unsupported = reserve(distinct)?;
    check_cancel(cancel)?;
    for (index, (&state, &geometry)) in snapshot
        .palette
        .iter()
        .zip(snapshot.geometry.iter())
        .enumerate()
    {
        if index % 64 == 0 {
            check_cancel(cancel)?;
        }
        if state != 0 && geometry == 0 {
            unsupported.push(state);
        }
    }
    check_cancel(cancel)?;
    unsupported.sort_unstable();
    check_cancel(cancel)?;
    Ok((faces, unsupported))
}

fn emit(
    snapshot: &SectionSnapshot,
    solid: &mut LayerBuffers,
    cancel: &impl Fn() -> bool,
) -> Result<(), MeshError> {
    for (index, face) in solid.faces.iter_mut().enumerate() {
        if index % 64 == 0 {
            check_cancel(cancel)?;
        }
        let first = u32::try_from(index * 6).map_err(|_| MeshError::OutputLimit)?;
        let base = u32::try_from(index * 4).map_err(|_| MeshError::OutputLimit)?;
        face.first_index = first;
        let light = cell(snapshot, neighbour(bordered(face.origin), face.face))?.packed_light();
        for corner in CORNERS[usize::from(face.face.code())] {
            solid.vertices.push(Vertex {
                position: std::array::from_fn(|axis| face.origin[axis] + corner[axis]),
                face: face.face,
                packed_light: light,
            });
        }
        solid
            .indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        match solid.materials.last_mut() {
            Some(material) if material.state_key == face.state_key => {
                material.indices.end = first + 6;
            }
            _ => solid.materials.push(MaterialRange {
                state_key: face.state_key,
                indices: first..first + 6,
            }),
        }
    }
    Ok(())
}

fn visibility(
    snapshot: &SectionSnapshot,
    cancel: &impl Fn() -> bool,
) -> Result<Visibility, MeshError> {
    let mut visited = [false; CORE_CELLS];
    let mut queue = [0_u16; CORE_CELLS];
    let mut result = Visibility::default();
    for start in 0..CORE_CELLS {
        if start % 64 == 0 {
            check_cancel(cancel)?;
        }
        if visited[start] || opaque(cell(snapshot, bordered(core_position(start)))?) {
            continue;
        }
        visited[start] = true;
        queue[0] = u16::try_from(start).map_err(|_| MeshError::OutputLimit)?;
        let (mut read, mut write, mut touched) = (0, 1, 0);
        while read < write {
            if read % 64 == 0 {
                check_cancel(cancel)?;
            }
            let p = core_position(usize::from(queue[read]));
            read += 1;
            for face in FACES {
                if on_face(p, face) {
                    touched |= 1 << face.code();
                    continue;
                }
                let next = neighbour(p, face);
                let index =
                    usize::from(next[1]) * 256 + usize::from(next[2]) * 16 + usize::from(next[0]);
                if !visited[index] && !opaque(cell(snapshot, bordered(next))?) {
                    visited[index] = true;
                    queue[write] = u16::try_from(index).map_err(|_| MeshError::OutputLimit)?;
                    write += 1;
                }
            }
        }
        result.connect(touched);
    }
    Ok(result)
}
fn on_face(p: [u8; 3], face: Face) -> bool {
    match face {
        Face::West => p[0] == 0,
        Face::East => p[0] == 15,
        Face::Down => p[1] == 0,
        Face::Up => p[1] == 15,
        Face::North => p[2] == 0,
        Face::South => p[2] == 15,
    }
}

#[cfg(test)]
mod tests;
