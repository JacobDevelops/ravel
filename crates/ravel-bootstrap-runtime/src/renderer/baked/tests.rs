use super::*;

fn input() -> Vec<u8> {
    let mut bytes = vec![0; 32 + RECORD];
    bytes[..4].copy_from_slice(&1_u32.to_le_bytes());
    bytes[16..24].copy_from_slice(&1_u64.to_le_bytes());
    bytes[24..32].copy_from_slice(&1_u64.to_le_bytes());
    for offset in [168, 172, 176, 180, 184, 188, 192, 196, 200, 204, 208, 212] {
        bytes[32 + offset..36 + offset].copy_from_slice(&1_f32.to_bits().to_le_bytes());
    }
    bytes
}

#[test]
fn packet_decodes_owned_source_and_emits_without_final_buffer_input() {
    let bytes = input();
    let (snapshot, camera) = decode(&bytes).expect("valid baked packet");
    let mesh = snapshot
        .mesh(1, camera, || false)
        .expect("valid baked mesh");
    assert_eq!(mesh.layers()[0].len(), 1);
    assert_eq!(mesh.payload_bytes(), 112);
    assert_eq!(mesh.layers()[0][0].vertices, [[0; 7]; 4]);
}

#[test]
fn malformed_or_nonfinite_packets_never_admit() {
    for bytes in [vec![], vec![0; 15], vec![255; 16], vec![0; 17]] {
        assert!(decode(&bytes).is_none());
    }
    let mut bytes = input();
    bytes[32 + 8..32 + 12].copy_from_slice(&4_u32.to_le_bytes());
    assert!(decode(&bytes).is_none());
    bytes[32 + 8..32 + 12].copy_from_slice(&0_u32.to_le_bytes());
    bytes[32 + 40..32 + 44].copy_from_slice(&f32::NAN.to_bits().to_le_bytes());
    assert!(decode(&bytes).is_none());
}

#[test]
fn final_buffer_is_only_an_oracle_and_mismatches_remain_observable() {
    let source = input();
    let mut oracle = vec![0; 128];
    oracle[..4].copy_from_slice(&1_u32.to_le_bytes());
    let mut output = Vec::new();
    let result = compare(1, &source, &oracle, &mut output);
    assert_eq!(output.len(), 32);
    assert_eq!(&output[..4], &[1, 0, 0, 0]);
    assert_eq!(&result[..3], &[1, 0, -1]);
    oracle[16 + 16..16 + 20].copy_from_slice(&1_u32.to_le_bytes());
    let result = compare(1, &source, &oracle, &mut Vec::new());
    assert_eq!(&result[..3], &[1, 1, 4]);
    oracle[..4].copy_from_slice(&2_u32.to_le_bytes());
    assert_eq!(compare(1, &source, &oracle, &mut Vec::new())[..2], [-1, 5]);
}

#[test]
fn independently_observed_atlas_generation_must_match_completed_resources() {
    let mut bytes = input();
    bytes[24..32].copy_from_slice(&2_u64.to_le_bytes());
    assert!(decode(&bytes).is_none());
    bytes[24..32].copy_from_slice(&0_u64.to_le_bytes());
    assert!(decode(&bytes).is_none());
}
