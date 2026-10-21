//! Retained CPU mesh transfer: fixed little-endian header, solid geometry only.
//!
//! Version 1 is 48 + vertices*5 + indices*4 + materials*12 bytes. Status is
//! 0 complete, 1 incomplete, 2 error; only complete payloads are drawable.
//! No ownership is transferred by encoding. The caller holds the completion borrow
//! through the Java-owned copy and acknowledges separately after accepting it.

use crate::mesh::{MAX_FACES, MeshError};
use crate::workers::Completion;

/// Magic bytes `RVM5`, interpreted as a little-endian u32.
pub const MAGIC: u32 = 0x354d_5652;
/// Stable transfer version.
pub const VERSION: u32 = 1;
/// Fixed magic/version, identity, status/counts and visibility prefix.
pub const HEADER_BYTES: usize = 48;
/// Maximum transient encoded payload, derived from the CPU mesh face ceiling.
pub const MAX_WIRE_BYTES: usize = HEADER_BYTES + MAX_FACES * (4 * 5 + 6 * 4 + 12);

fn encoded_len(vertices: usize, indices: usize, materials: usize) -> Result<usize, MeshError> {
    if vertices > MAX_FACES * 4 || indices > MAX_FACES * 6 || materials > MAX_FACES {
        return Err(MeshError::OutputLimit);
    }
    Ok(HEADER_BYTES + vertices * 5 + indices * 4 + materials * 12)
}

/// Encodes a bounded owned copy. Allocation/limit failure leaves its source untouched.
///
/// # Errors
///
/// Returns `Allocation` if the encoded buffer cannot be reserved, or `OutputLimit`
/// if a mesh count exceeds the bounded wire representation.
pub fn encode(completion: &Completion) -> Result<Vec<u8>, MeshError> {
    let mesh = completion.result.as_ref().ok();
    let solid = mesh.map(|mesh| &mesh.layers()[0]);
    let (vertices, indices, materials) = solid.map_or((0, 0, 0), |solid| {
        (
            solid.vertices().len(),
            solid.indices().len(),
            solid.materials().len(),
        )
    });
    let length = encoded_len(vertices, indices, materials)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| MeshError::Allocation)?;
    bytes.extend_from_slice(&MAGIC.to_le_bytes());
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    bytes.extend_from_slice(&completion.generation.to_le_bytes());
    bytes.extend_from_slice(&completion.job_id.to_le_bytes());
    let status = completion.outcome() as u32;
    for value in [status, count(vertices)?, count(indices)?, count(materials)?] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(
        &mesh
            .map_or(0, |mesh| mesh.visibility().bits())
            .to_le_bytes(),
    );
    if let Some(solid) = solid {
        for vertex in solid.vertices() {
            bytes.extend_from_slice(&vertex.position());
            bytes.push(vertex.face().code());
            bytes.push(vertex.packed_light());
        }
        for index in solid.indices() {
            bytes.extend_from_slice(&index.to_le_bytes());
        }
        for material in solid.materials() {
            let range = material.indices();
            for value in [material.state_key(), range.start, range.end - range.start] {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
    }
    Ok(bytes)
}

fn count(value: usize) -> Result<u32, MeshError> {
    u32::try_from(value).map_err(|_| MeshError::OutputLimit)
}

#[cfg(test)]
mod tests;
