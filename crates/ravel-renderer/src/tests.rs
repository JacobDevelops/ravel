use super::{
    CELL_COUNT, MAX_BLOCK_ID, MAX_CAPTURE_ALLOCATIONS, MAX_CAPTURE_CELL_VISITS,
    MAX_CAPTURE_HEAP_SCRATCH_BYTES, MAX_CAPTURE_PALETTE_PROBES, MAX_CAPTURE_SCRATCH_BYTES,
    MAX_CAPTURE_STACK_SCRATCH_BYTES, MAX_GENERATION, MAX_HEAP_BYTES, MAX_PALETTE_ENTRIES,
    MAX_STATE_KEY, MAX_TOTAL_BYTES, SNAPSHOT_EDGE, SectionSnapshot, SnapshotError,
};

#[test]
fn air_deduplicates_state_and_model_identity() {
    let states = vec![0; CELL_COUNT];
    let snapshot = SectionSnapshot::capture(1, &states, &[0; CELL_COUNT], &[0; CELL_COUNT])
        .expect("air snapshot");
    assert_eq!(snapshot.generation(), 1);
    assert_eq!(snapshot.palette_len(), 1);
    assert_eq!(snapshot.heap_bytes(), 5 + CELL_COUNT * 3);
    for index in 0..CELL_COUNT {
        let cell = snapshot.cell(index).expect("air cell");
        assert_eq!(cell.state_key(), 0);
        assert_eq!(cell.model_key(), 0);
        assert_eq!(cell.packed_light(), 0);
    }
}

#[test]
fn sparse_states_are_owned_and_first_occurrence_deduplicated() {
    let mut states = vec![0; CELL_COUNT];
    let mut light = [0xf0; CELL_COUNT];
    states[5] = 17;
    states[20] = 17;
    states[CELL_COUNT - 1] = 32;
    let snapshot =
        SectionSnapshot::capture(9, &states, &light, &[0; CELL_COUNT]).expect("sparse snapshot");
    states.fill(99);
    light.fill(0);
    assert_eq!(snapshot.palette.as_ref(), &[0, 17, 32]);
    assert_eq!(snapshot.indices[5], snapshot.indices[20]);
    assert_eq!(snapshot.cell(5).expect("state").state_key(), 17);
    assert_eq!(snapshot.cell(5).expect("light").packed_light(), 0xf0);
    assert_eq!(snapshot.cell(CELL_COUNT - 1).expect("last").state_key(), 32);
}

#[test]
fn high_block_ids_preserve_every_metadata_value() {
    let mut states = vec![0; CELL_COUNT];
    for (metadata, state) in states[..16].iter_mut().enumerate() {
        *state = i32::try_from((MAX_BLOCK_ID << 4) | u32::try_from(metadata).expect("metadata"))
            .expect("packed state");
    }
    let snapshot =
        SectionSnapshot::capture(MAX_GENERATION, &states, &[0; CELL_COUNT], &[0; CELL_COUNT])
            .expect("high ids");
    assert_eq!(snapshot.generation(), MAX_GENERATION);
    for (metadata, &state) in states[..16].iter().enumerate() {
        let cell = snapshot.cell(metadata).expect("high id cell");
        assert_eq!(cell.block_id(), MAX_BLOCK_ID);
        assert_eq!(cell.metadata(), u8::try_from(metadata).expect("metadata"));
        assert_eq!(cell.state_key(), u32::try_from(state).expect("state"));
        assert_eq!(cell.model_key(), cell.state_key());
    }
}

#[test]
fn all_packed_light_values_roundtrip_without_state_duplication() {
    let mut light = [0; CELL_COUNT];
    for (index, value) in light.iter_mut().enumerate() {
        *value = index.to_le_bytes()[0];
    }
    let states = vec![0; CELL_COUNT];
    let snapshot = SectionSnapshot::capture(1, &states, &light, &[0; CELL_COUNT]).expect("light");
    assert_eq!(snapshot.palette_len(), 1);
    for (index, expected) in light.iter().copied().enumerate() {
        let cell = snapshot.cell(index).expect("light cell");
        assert_eq!(cell.packed_light(), expected);
        assert_eq!(cell.sky_light(), expected >> 4);
        assert_eq!(cell.block_light(), expected & 15);
    }
}

#[test]
fn every_core_face_edge_and_corner_uses_y_z_x_order() {
    let states: Vec<_> = (0..CELL_COUNT)
        .map(|index| i32::try_from(index).expect("cell index"))
        .collect();
    let light = std::array::from_fn::<_, CELL_COUNT, _>(|index| index.to_le_bytes()[0]);
    let snapshot =
        SectionSnapshot::capture(1, &states, &light, &[0; CELL_COUNT]).expect("bordered snapshot");
    for y in 0..SNAPSHOT_EDGE {
        for z in 0..SNAPSHOT_EDGE {
            for x in 0..SNAPSHOT_EDGE {
                let index = ((y * SNAPSHOT_EDGE) + z) * SNAPSHOT_EDGE + x;
                let cell = snapshot.cell_at(x, y, z).expect("border and core cell");
                assert_eq!(cell.state_key(), u32::try_from(index).expect("cell index"));
                assert_eq!(cell.packed_light(), light[index]);
            }
        }
    }
}

#[test]
fn all_distinct_palette_reaches_exact_storage_bound() {
    let states: Vec<_> = (0..CELL_COUNT)
        .map(|index| i32::try_from(index).expect("cell index"))
        .collect();
    let snapshot = SectionSnapshot::capture(1, &states, &[255; CELL_COUNT], &[0; CELL_COUNT])
        .expect("all distinct snapshot");
    assert_eq!(snapshot.palette_len(), MAX_PALETTE_ENTRIES);
    assert_eq!(snapshot.heap_bytes(), MAX_HEAP_BYTES);
    assert_eq!(snapshot.total_bytes(), MAX_TOTAL_BYTES);
    assert_eq!(MAX_HEAP_BYTES, 46_656);
    assert_eq!(MAX_CAPTURE_ALLOCATIONS, 5);
    assert_eq!(MAX_CAPTURE_CELL_VISITS, CELL_COUNT);
    assert_eq!(MAX_CAPTURE_PALETTE_PROBES, CELL_COUNT * CELL_COUNT);
    assert_eq!(MAX_CAPTURE_HEAP_SCRATCH_BYTES, 23_328);
    assert_eq!(MAX_CAPTURE_STACK_SCRATCH_BYTES, 22_216);
    assert_eq!(MAX_CAPTURE_SCRATCH_BYTES, 45_544);
    for (index, state) in states.iter().copied().enumerate() {
        assert_eq!(
            snapshot.cell(index).expect("distinct cell").state_key(),
            u32::try_from(state).expect("state")
        );
    }
}

#[test]
fn colliding_keys_roundtrip_deterministically() {
    let colliding_states: Vec<_> = (0..=MAX_STATE_KEY)
        .filter(|key| key.wrapping_mul(0x9e37_79b9) >> 19 == 0)
        .take(64)
        .map(|key| i32::try_from(key).expect("colliding state"))
        .collect();
    assert_eq!(colliding_states.len(), 64);
    let states: Vec<_> = colliding_states
        .iter()
        .copied()
        .cycle()
        .take(CELL_COUNT)
        .collect();
    let light = std::array::from_fn::<_, CELL_COUNT, _>(|index| index.to_le_bytes()[0]);
    let first =
        SectionSnapshot::capture(7, &states, &light, &[0; CELL_COUNT]).expect("first snapshot");
    let second =
        SectionSnapshot::capture(7, &states, &light, &[0; CELL_COUNT]).expect("second snapshot");
    assert_eq!(first, second);
    assert_eq!(first.palette_len(), 64);
    assert!(first.heap_bytes() <= MAX_HEAP_BYTES);
    for (index, state) in states.iter().copied().enumerate() {
        let cell = first.cell(index).expect("colliding cell");
        assert_eq!(cell.state_key(), u32::try_from(state).expect("state"));
        assert_eq!(cell.packed_light(), light[index]);
    }
}

#[test]
fn decode_rejects_out_of_bounds_without_arithmetic_overflow() {
    let states = vec![0; CELL_COUNT];
    let snapshot = SectionSnapshot::capture(1, &states, &[0; CELL_COUNT], &[0; CELL_COUNT])
        .expect("air snapshot");
    assert_eq!(snapshot.cell(CELL_COUNT), None);
    assert_eq!(snapshot.cell(usize::MAX), None);
    for invalid in [SNAPSHOT_EDGE, usize::MAX] {
        assert_eq!(snapshot.cell_at(invalid, 0, 0), None);
        assert_eq!(snapshot.cell_at(0, invalid, 0), None);
        assert_eq!(snapshot.cell_at(0, 0, invalid), None);
    }
}

#[test]
fn capture_rejects_invalid_lengths_on_either_input() {
    let states = vec![0; CELL_COUNT + 1];
    let light = [0; CELL_COUNT + 1];
    for (state_len, light_len) in [
        (0, 0),
        (CELL_COUNT - 1, CELL_COUNT),
        (CELL_COUNT + 1, CELL_COUNT),
        (CELL_COUNT, CELL_COUNT - 1),
        (CELL_COUNT, CELL_COUNT + 1),
    ] {
        assert_eq!(
            SectionSnapshot::capture(
                1,
                &states[..state_len],
                &light[..light_len],
                &[0; CELL_COUNT]
            ),
            Err(SnapshotError::InvalidLengths {
                states: state_len,
                light: light_len,
                geometry: CELL_COUNT,
            })
        );
    }
}

#[test]
fn capture_rejects_invalid_generation_and_state_ranges() {
    let mut states = vec![0; CELL_COUNT];
    for generation in [0, MAX_GENERATION + 1, u64::MAX] {
        assert_eq!(
            SectionSnapshot::capture(generation, &states, &[0; CELL_COUNT], &[0; CELL_COUNT]),
            Err(SnapshotError::InvalidGeneration)
        );
    }
    for invalid in [
        i32::MIN,
        -1,
        i32::try_from(MAX_STATE_KEY + 1).expect("too large"),
        i32::MAX,
    ] {
        states[CELL_COUNT - 1] = invalid;
        assert_eq!(
            SectionSnapshot::capture(1, &states, &[0; CELL_COUNT], &[0; CELL_COUNT]),
            Err(SnapshotError::InvalidState {
                index: CELL_COUNT - 1,
                state: invalid
            })
        );
    }
}

#[test]
fn geometry_flags_are_validated_and_consistent_per_state_key() {
    let states = vec![16; CELL_COUNT];
    let light = [0; CELL_COUNT];
    let mut geometry = [1; CELL_COUNT];
    let snapshot = SectionSnapshot::capture(1, &states, &light, &geometry).expect("cube geometry");
    assert!(snapshot.cell(0).expect("cell").is_full_opaque_cube());
    geometry[1] = 0;
    assert_eq!(
        SectionSnapshot::capture(1, &states, &light, &geometry),
        Err(SnapshotError::InconsistentGeometry { state: 16 })
    );
    geometry[1] = 2;
    assert_eq!(
        SectionSnapshot::capture(1, &states, &light, &geometry),
        Err(SnapshotError::InvalidGeometry { index: 1, flag: 2 })
    );
    assert_eq!(
        SectionSnapshot::capture(1, &states, &light, &geometry[..CELL_COUNT - 1]),
        Err(SnapshotError::InvalidLengths {
            states: CELL_COUNT,
            light: CELL_COUNT,
            geometry: CELL_COUNT - 1,
        })
    );
}
