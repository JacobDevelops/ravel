use super::*;

fn input() -> Vec<u8> {
    let mut bytes = vec![0; 16 + RECORD];
    bytes[..4].copy_from_slice(&1_u32.to_le_bytes());
    for offset in [168, 172, 176, 180, 184, 188, 192, 196, 200, 204, 208, 212] {
        bytes[16 + offset..20 + offset].copy_from_slice(&1_f32.to_bits().to_le_bytes());
    }
    bytes
}

#[test]
fn packet_decodes_owned_source_and_emits_without_final_buffer_input() {
    let bytes = input();
    let (snapshot, camera) = decode(1, &bytes).unwrap();
    let mesh = snapshot.mesh(1, camera, || false).unwrap();
    assert_eq!(mesh.layers()[0].len(), 1);
    assert_eq!(mesh.payload_bytes(), 112);
    assert_eq!(mesh.layers()[0][0].vertices, [[0; 7]; 4]);
}

#[test]
fn malformed_or_nonfinite_packets_never_admit() {
    for bytes in [vec![], vec![0; 15], vec![255; 16], vec![0; 17]] {
        if bytes.len() == 17 { assert!(decode(0, &bytes).is_none()); }
        else { assert!(decode(1, &bytes).is_none()); }
    }
    let mut bytes = input();
    bytes[16 + 8..16 + 12].copy_from_slice(&4_u32.to_le_bytes());
    assert!(decode(1, &bytes).is_none());
    bytes[16 + 8..16 + 12].copy_from_slice(&0_u32.to_le_bytes());
    bytes[16 + 40..16 + 44].copy_from_slice(&f32::NAN.to_bits().to_le_bytes());
    assert!(decode(1, &bytes).is_none());
}

#[test]
fn final_buffer_is_only_an_oracle_and_mismatches_remain_observable() {
    let source = input();
    let mut oracle = vec![0; 128];
    oracle[..4].copy_from_slice(&1_u32.to_le_bytes());
    let result = compare(1, &source, &oracle);
    assert_eq!(&result[..3], &[1, 0, -1]);
    oracle[16 + 16..16 + 20].copy_from_slice(&1_u32.to_le_bytes());
    let result = compare(1, &source, &oracle);
    assert_eq!(&result[..3], &[1, 1, 4]);
    oracle[..4].copy_from_slice(&2_u32.to_le_bytes());
    assert_eq!(compare(1, &source, &oracle)[..2], [-1, 5]);
}
