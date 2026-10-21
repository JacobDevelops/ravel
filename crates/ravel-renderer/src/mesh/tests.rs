use super::*;
use std::cell::Cell;

fn snapshot(states: &[i32]) -> SectionSnapshot {
    let geometry: Vec<_> = states.iter().map(|state| u8::from(*state == 16)).collect();
    SectionSnapshot::capture(7, states, &[0xa3; CELL_COUNT], &geometry).expect("fixture")
}

fn index(x: usize, y: usize, z: usize) -> usize {
    (y * 18 + z) * 18 + x
}

#[test]
fn air_has_all_visibility_pairs_and_no_allocations() {
    let result = mesh(&snapshot(&vec![0; CELL_COUNT]), 42, || false).expect("mesh");
    assert_eq!((result.generation(), result.key()), (7, 42));
    assert_eq!(result.visibility().bits(), (1_u64 << 36) - 1);
    assert_eq!(result.allocations(), 0);
    assert!(result.is_complete());
    assert_eq!(result.total_bytes(), size_of::<MeshResult>());
    assert!(
        result
            .layers()
            .iter()
            .all(|layer| layer.vertices().is_empty())
    );
}

#[test]
fn stone_primitive_has_exact_winding_light_and_ranges() {
    let mut states = vec![0; CELL_COUNT];
    states[index(1, 1, 1)] = 16;
    let result = mesh(&snapshot(&states), 9, || false).expect("mesh");
    let solid = &result.layers()[0];
    assert_eq!(solid.vertices().len(), 24);
    assert_eq!(solid.indices().len(), 36);
    assert_eq!(solid.materials()[0].state_key(), 16);
    assert_eq!(solid.materials()[0].indices(), 0..36);
    assert_eq!(solid.faces().len(), 6);
    assert_eq!(solid.vertices()[0].position(), [0, 0, 0]);
    assert!(solid.vertices().iter().all(|v| v.packed_light() == 0xa3));
    assert_eq!(&solid.indices()[..6], &[0, 1, 2, 0, 2, 3]);
    assert_eq!(result.allocations(), 4);
    assert_eq!(geometry_hash(solid), 0x4fbc_2b7e_8a50_c27d);
}

#[test]
fn shared_and_border_faces_are_culled() {
    let mut states = vec![0; CELL_COUNT];
    states[index(1, 1, 1)] = 16;
    states[index(2, 1, 1)] = 16;
    states[index(0, 1, 1)] = 16;
    let result = mesh(&snapshot(&states), 0, || false).expect("mesh");
    assert_eq!(result.layers()[0].faces().len(), 9);
    // Independent topology: first cube exposes D/U/N/S; second E/D/U/N/S.
    assert_eq!(geometry_hash(&result.layers()[0]), 0xbb7a_e302_9f02_ece3);
    let solid = mesh(&snapshot(&vec![16; CELL_COUNT]), 0, || false).expect("solid");
    assert_eq!(solid.layers()[0].vertices(), []);
    assert_eq!(solid.visibility().bits(), 0);
}

#[test]
fn unsupported_keys_are_sorted_deduplicated_and_never_occlude() {
    let mut states = vec![0; CELL_COUNT];
    states[index(1, 1, 1)] = 16;
    states[index(0, 1, 1)] = 17;
    states[index(2, 1, 1)] = 32;
    states[CELL_COUNT - 1] = 17;
    let result = mesh(&snapshot(&states), 0, || false).expect("partial");
    assert_eq!(result.unsupported_keys(), &[17, 32]);
    assert!(!result.is_complete());
    assert_eq!(result.layers()[0].faces().len(), 6);
    assert_eq!(result.visibility().bits(), (1_u64 << 36) - 1);
    assert_eq!(result.allocations(), 5);
}

#[test]
fn sealed_wall_disconnects_opposite_faces_but_unknown_wall_does_not() {
    let mut states = vec![0; CELL_COUNT];
    for y in 1..=16 {
        for z in 1..=16 {
            states[index(8, y, z)] = 16;
        }
    }
    let result = mesh(&snapshot(&states), 0, || false).expect("wall");
    assert!(!result.visibility().connected(Face::West, Face::East));
    assert!(result.visibility().connected(Face::North, Face::South));
    for value in &mut states {
        if *value == 16 {
            *value = 17;
        }
    }
    let unknown = mesh(&snapshot(&states), 0, || false).expect("unknown wall");
    assert!(unknown.visibility().connected(Face::West, Face::East));
}

#[test]
fn cancellation_covers_count_emission_and_flood_fill() {
    let mut states = vec![0; CELL_COUNT];
    states[index(1, 1, 1)] = 16;
    let input = snapshot(&states);
    let total = Cell::new(0);
    mesh(&input, 0, || {
        total.set(total.get() + 1);
        false
    })
    .expect("complete reference");
    for stop in 0..total.get() {
        let calls = Cell::new(0);
        let result = mesh(&input, 0, || {
            let count = calls.get();
            calls.set(count + 1);
            count >= stop
        });
        assert_eq!(result, Err(MeshError::Cancelled), "stop {stop}");
        assert_eq!(
            calls.get(),
            stop + 1,
            "no work after cancellation at {stop}"
        );
    }
}

#[test]
fn worst_case_checkerboard_stays_inside_output_and_allocation_caps() {
    let mut states = vec![0; CELL_COUNT];
    for y in 1..=16 {
        for z in 1..=16 {
            for x in 1..=16 {
                if (x + y + z) % 2 == 0 {
                    states[index(x, y, z)] = 16;
                }
            }
        }
    }
    let result = mesh(&snapshot(&states), u64::MAX, || false).expect("bounded");
    assert_eq!(result.layers()[0].faces().len(), 12_288);
    assert!(result.total_bytes() <= MAX_MESH_BYTES);
    assert!(result.allocations() <= MAX_MESH_ALLOCATIONS);
    assert_eq!(result.layers()[0].vertices().len(), 49_152);
}

fn geometry_hash(layer: &LayerBuffers) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for vertex in layer.vertices() {
        for byte in vertex
            .position()
            .into_iter()
            .chain([vertex.face().code(), vertex.packed_light()])
        {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
        }
    }
    for index in layer.indices() {
        for byte in index.to_le_bytes() {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
        }
    }
    hash
}

#[test]
fn captured_high_key_cube_support_is_not_an_id_whitelist() {
    let key = i32::try_from(crate::MAX_STATE_KEY).expect("key");
    let mut states = vec![0; CELL_COUNT];
    let mut geometry = [0; CELL_COUNT];
    states[index(1, 1, 1)] = key;
    geometry[index(1, 1, 1)] = 1;
    states[index(0, 1, 1)] = key;
    geometry[index(0, 1, 1)] = 1;
    let captured =
        SectionSnapshot::capture(1, &states, &[0; CELL_COUNT], &geometry).expect("cubes");
    let result = mesh(&captured, 0, || false).expect("supported");
    assert!(result.is_complete());
    assert_eq!(result.layers()[0].faces().len(), 5);
    assert_eq!(
        result.layers()[0].materials()[0].state_key(),
        crate::MAX_STATE_KEY
    );
    geometry.fill(0);
    let unknown =
        SectionSnapshot::capture(1, &states, &[0; CELL_COUNT], &geometry).expect("unknown");
    let result = mesh(&unknown, 0, || false).expect("unsupported");
    assert_eq!(result.unsupported_keys(), &[crate::MAX_STATE_KEY]);
    assert_eq!(result.layers()[0].faces(), []);
}

#[test]
fn distinct_cube_keys_have_contiguous_materials_and_adjacent_raw_light() {
    let mut states = vec![0; CELL_COUNT];
    let mut geometry = [0; CELL_COUNT];
    let mut light = [0; CELL_COUNT];
    for (x, key) in [(1, 32), (3, 16)] {
        states[index(x, 1, 1)] = key;
        geometry[index(x, 1, 1)] = 1;
    }
    light[index(2, 1, 1)] = 0xf2;
    let snapshot = SectionSnapshot::capture(1, &states, &light, &geometry).expect("materials");
    let result = mesh(&snapshot, 0, || false).expect("mesh");
    let solid = &result.layers()[0];
    assert_eq!(solid.materials().len(), 2);
    assert_eq!(solid.materials()[0].state_key(), 16);
    assert_eq!(solid.materials()[0].indices(), 0..36);
    assert_eq!(solid.materials()[1].state_key(), 32);
    assert_eq!(solid.materials()[1].indices(), 36..72);
    assert_eq!(solid.vertices()[0].packed_light(), 0xf2);
    assert_eq!(solid.faces()[0].centre_twice(), [4, 1, 1]);
    for quad in solid.vertices().as_chunks::<4>().0 {
        let first_position = quad[0].position().map(i32::from);
        let second_position = quad[1].position().map(i32::from);
        let third_position = quad[2].position().map(i32::from);
        let first_edge: [i32; 3] =
            std::array::from_fn(|axis| second_position[axis] - first_position[axis]);
        let second_edge: [i32; 3] =
            std::array::from_fn(|axis| third_position[axis] - first_position[axis]);
        let cross = [
            first_edge[1] * second_edge[2] - first_edge[2] * second_edge[1],
            first_edge[2] * second_edge[0] - first_edge[0] * second_edge[2],
            first_edge[0] * second_edge[1] - first_edge[1] * second_edge[0],
        ];
        let normal = match quad[0].face() {
            Face::West => [-1, 0, 0],
            Face::East => [1, 0, 0],
            Face::Down => [0, -1, 0],
            Face::Up => [0, 1, 0],
            Face::North => [0, 0, -1],
            Face::South => [0, 0, 1],
        };
        assert_eq!(cross, normal);
    }
}

#[test]
fn unsupported_only_and_air_have_exact_empty_geometry_hash() {
    let mut states = vec![0; CELL_COUNT];
    for key in [0, 17] {
        states.fill(key);
        let result = mesh(&snapshot(&states), 0, || false).expect("empty geometry");
        assert_eq!(geometry_hash(&result.layers()[0]), 0xcbf2_9ce4_8422_2325);
    }
}

#[test]
fn every_face_uses_its_own_adjacent_light_without_nibble_loss() {
    let mut states = vec![0; CELL_COUNT];
    let mut geometry = [0; CELL_COUNT];
    let mut light = [0; CELL_COUNT];
    states[index(8, 8, 8)] = 32;
    geometry[index(8, 8, 8)] = 1;
    let adjacent = [
        (7, 8, 8),
        (9, 8, 8),
        (8, 7, 8),
        (8, 9, 8),
        (8, 8, 7),
        (8, 8, 9),
    ];
    let values = [0x01, 0x20, 0x3f, 0xf4, 0xa5, 0x6b];
    for ((x, y, z), value) in adjacent.into_iter().zip(values) {
        light[index(x, y, z)] = value;
    }
    let snapshot = SectionSnapshot::capture(1, &states, &light, &geometry).expect("light fixture");
    let result = mesh(&snapshot, 0, || false).expect("mesh");
    for (quad, value) in result.layers()[0]
        .vertices()
        .as_chunks::<4>()
        .0
        .iter()
        .zip(values)
    {
        assert!(quad.iter().all(|vertex| vertex.packed_light() == value));
    }
}

#[test]
fn sealed_pocket_has_no_visibility_and_partial_opening_only_touches_one_face() {
    let mut states = vec![16; CELL_COUNT];
    states[index(8, 8, 8)] = 0;
    let sealed = mesh(&snapshot(&states), 0, || false).expect("pocket");
    assert_eq!(sealed.visibility().bits(), 0);
    assert_eq!(sealed.layers()[0].faces().len(), 6);
    for x in 1..=8 {
        states[index(x, 8, 8)] = 0;
    }
    let open = mesh(&snapshot(&states), 0, || false).expect("open pocket");
    assert_eq!(open.visibility().bits(), 1);
    assert!(open.visibility().connected(Face::West, Face::West));
    assert!(!open.visibility().connected(Face::West, Face::East));
}

#[test]
fn air_is_never_an_opaque_cube_even_with_captured_flag_one() {
    let states = vec![0; CELL_COUNT];
    let snapshot =
        SectionSnapshot::capture(1, &states, &[0; CELL_COUNT], &[1; CELL_COUNT]).expect("air");
    assert!(!snapshot.cell(0).expect("air cell").is_full_opaque_cube());
    let result = mesh(&snapshot, 0, || false).expect("air mesh");
    assert_eq!(result.layers()[0].vertices(), []);
    assert!(result.is_complete());
    assert_eq!(result.visibility().bits(), (1_u64 << 36) - 1);
}

#[test]
fn mesh_cell_lookup_refuses_coordinates_outside_the_capture() {
    let mut states = vec![0; CELL_COUNT];
    states[index(1, 2, 3)] = 16;
    let snapshot = snapshot(&states);
    for position in [[18, 0, 0], [0, 18, 0], [0, 0, 18], [255; 3]] {
        assert_eq!(
            cell(&snapshot, position),
            Err(MeshError::InvalidCoordinates)
        );
    }
    for (position, state) in [([0; 3], 0), ([17; 3], 0), ([1, 2, 3], 16)] {
        assert_eq!(
            cell(&snapshot, position).map(crate::Cell::state_key),
            Ok(state)
        );
    }
}
