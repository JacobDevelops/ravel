use super::*;
use crate::mesh::mesh;
use crate::{CELL_COUNT, SectionSnapshot};

fn completion(state: i32) -> Completion {
    let mut states = vec![0; CELL_COUNT];
    states[(18 + 1) * 18 + 1] = state;
    let geometry: Vec<_> = states.iter().map(|value| u8::from(*value == 16)).collect();
    let snapshot = SectionSnapshot::capture(7, &states, &[0xa3; CELL_COUNT], &geometry)
        .unwrap_or_else(|error| panic!("snapshot: {error:?}"));
    Completion {
        generation: 7,
        job_id: 0x0102_0304_0506_0708,
        result: mesh(&snapshot, 0x0102_0304_0506_0708, || false),
        queue_wait_nanos: 0,
        mesh_nanos: 0,
        output_bytes: 0,
        allocations: 0,
    }
}

#[test]
fn solid_wire_has_exact_lengths_endian_geometry_and_materials() {
    let bytes = encode(&completion(16)).unwrap_or_else(|error| panic!("wire: {error:?}"));
    assert_eq!(bytes.len(), 48 + 24 * 5 + 36 * 4 + 12);
    assert_eq!(&bytes[..8], &[0x52, 0x56, 0x4d, 0x35, 1, 0, 0, 0]);
    assert_eq!(&bytes[8..16], &7_i64.to_le_bytes());
    assert_eq!(&bytes[16..24], &0x0102_0304_0506_0708_i64.to_le_bytes());
    assert_eq!(
        &bytes[24..40],
        &[0, 0, 0, 0, 24, 0, 0, 0, 36, 0, 0, 0, 1, 0, 0, 0]
    );
    assert_eq!(&bytes[40..48], &((1_u64 << 36) - 1).to_le_bytes());
    assert_eq!(&bytes[48..53], &[0, 0, 0, 0, 0xa3]);
    let indices = 48 + 24 * 5;
    for (index, value) in [0_u32, 1, 2, 0, 2, 3].into_iter().enumerate() {
        assert_eq!(
            &bytes[indices + index * 4..indices + index * 4 + 4],
            &value.to_le_bytes()
        );
    }
    assert_eq!(
        &bytes[bytes.len() - 12..],
        &[16, 0, 0, 0, 0, 0, 0, 0, 36, 0, 0, 0]
    );
}

#[test]
fn empty_incomplete_and_error_are_explicit_and_bounded() {
    for (state, status) in [(0, 0_u32), (17, 1)] {
        let bytes = encode(&completion(state)).unwrap_or_else(|error| panic!("wire: {error:?}"));
        assert_eq!(bytes.len(), HEADER_BYTES);
        assert_eq!(&bytes[24..28], &status.to_le_bytes());
        assert_eq!(&bytes[28..40], &[0; 12]);
    }
    let mut failed = completion(0);
    failed.result = Err(MeshError::OutputLimit);
    let bytes = encode(&failed).unwrap_or_else(|error| panic!("wire: {error:?}"));
    assert_eq!(bytes.len(), HEADER_BYTES);
    assert_eq!(&bytes[24..28], &2_u32.to_le_bytes());
    assert_eq!(
        encoded_len(MAX_FACES * 4, MAX_FACES * 6, MAX_FACES),
        Ok(MAX_WIRE_BYTES)
    );
    assert_eq!(MAX_WIRE_BYTES, 1_376_304);
    assert_eq!(
        encoded_len(MAX_FACES * 4 + 1, 0, 0),
        Err(MeshError::OutputLimit)
    );
    assert_eq!(
        encoded_len(0, MAX_FACES * 6 + 1, 0),
        Err(MeshError::OutputLimit)
    );
    assert_eq!(
        encoded_len(0, 0, MAX_FACES + 1),
        Err(MeshError::OutputLimit)
    );
    assert_eq!(encoded_len(usize::MAX, 0, 0), Err(MeshError::OutputLimit));
}
