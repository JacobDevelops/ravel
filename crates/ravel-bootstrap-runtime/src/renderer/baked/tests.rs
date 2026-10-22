use super::super::{Lifecycle, RELOAD, Renderer, work};
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
    let result = emit_and_compare(&source, &oracle, &mut output, || false);
    assert_eq!(output.len(), 32);
    assert_eq!(&output[..4], &[1, 0, 0, 0]);
    assert_eq!(&result[..3], &[1, 0, -1]);
    oracle[16 + 16..16 + 20].copy_from_slice(&1_u32.to_le_bytes());
    let result = emit_and_compare(&source, &oracle, &mut Vec::new(), || false);
    assert_eq!(&result[..3], &[1, 1, 4]);
    oracle[..4].copy_from_slice(&2_u32.to_le_bytes());
    assert_eq!(
        emit_and_compare(&source, &oracle, &mut Vec::new(), || false)[..2],
        [-1, 5]
    );
}

#[test]
fn independently_observed_atlas_generation_must_match_completed_resources() {
    let mut bytes = input();
    bytes[24..32].copy_from_slice(&2_u64.to_le_bytes());
    assert!(decode(&bytes).is_none());
    bytes[24..32].copy_from_slice(&0_u64.to_le_bytes());
    assert!(decode(&bytes).is_none());
}

fn retained(generation: jlong, resource: jlong) -> residency::Retained {
    let mut source = input();
    source[16..24].copy_from_slice(&resource.to_le_bytes());
    source[24..32].copy_from_slice(&resource.to_le_bytes());
    residency::Retained {
        identity: [generation, 0, resource, resource, -16, 32, 48],
        mesh: decode(&source)
            .expect("snapshot")
            .0
            .mesh(u64::try_from(resource).expect("resource"), [0.; 3], || {
                false
            })
            .expect("mesh"),
    }
}

#[test]
fn shaded_replacement_requires_exact_acknowledgement_and_keeps_owned_mesh() {
    let mut leases = Residency::new();
    let first = leases.publish(retained(1, 2)).expect("first pending");
    assert!(leases.busy());
    assert!(leases.publish(retained(1, 2)).is_none());
    assert!(!leases.settle(2, first, true));
    assert!(!leases.settle(1, first + 1, true));
    assert!(leases.settle(1, first, true));
    assert!(!leases.settle(1, first, true));
    let old = leases.metrics();
    assert!(old[0] > 0);
    assert_eq!(old[1], 0);
    assert_eq!(&old[5..], &[1, first, 2, 2, -16, 32, 48]);
    let second = leases.publish(retained(1, 3)).expect("replacement pending");
    let pending = leases.metrics();
    assert_eq!(pending[0], old[0]);
    assert_eq!(pending[1], old[0]);
    assert_eq!(pending[2], old[0] * 2);
    assert_eq!(&pending[5..], &old[5..]);
    assert!(leases.settle(1, second, true));
    assert_eq!(leases.metrics()[3..5], [2, 1]);
    assert_eq!(leases.metrics()[7..9], [3, 3]);
}

#[test]
fn translucent_resort_requires_the_exact_live_lease_before_and_after_acknowledgement() {
    let mut leases = Residency::new();
    let key = leases.publish(retained(7, 2)).expect("pending");
    assert!(leases.resort(7, key, [0.; 3], 4).is_some());
    assert!(leases.settle(7, key, true));
    let before = leases.metrics();
    assert!(leases.resort(8, key, [0.; 3], 4).is_none());
    assert!(leases.resort(7, key + 1, [0.; 3], 4).is_none());
    assert!(leases.resort(7, key, [f32::NAN; 3], 4).is_none());
    assert!(leases.resort(7, key, [4.; 3], 5).is_none());
    assert!(leases.resort(7, key, [4.; 3], 4).is_some());
    assert_eq!(leases.metrics(), before);
    leases.retire();
    assert!(leases.resort(7, key, [0.; 3], 4).is_none());
}

#[test]
fn shaded_refusal_reload_and_shutdown_release_both_slots_without_reusing_keys() {
    let mut leases = Residency::new();
    let first = leases.publish(retained(1, 1)).expect("first pending");
    assert!(leases.settle(1, first, true));
    let refused = leases.publish(retained(1, 1)).expect("refused pending");
    assert!(leases.settle(1, refused, false));
    assert_eq!(leases.metrics()[6], first);
    let pending = leases.publish(retained(1, 1)).expect("reload pending");
    leases.retire();
    assert_eq!(&leases.metrics()[..2], &[0, 0]);
    assert_eq!(&leases.metrics()[5..], &[0; 7]);
    assert!(!leases.settle(1, pending, true));
    let reopened = leases.publish(retained(2, 2)).expect("reopened pending");
    assert!(reopened > pending);
    assert!(leases.settle(2, reopened, true));
    leases.retire();
    leases.retire();
    assert_eq!(leases.metrics()[4], 4);
    assert_eq!(&leases.metrics()[..2], &[0, 0]);
}

#[test]
fn shaded_reservations_and_provenance_refuse_outside_the_fixed_envelope() {
    let mut leases = Residency::new();
    let mut oversized = retained(1, 1);
    let quad = QuadInput {
        state_key: 0,
        sprite: 0,
        layer: RenderLayer::Solid,
        vertices: [[0; 7]; 4],
        offset: [0; 3],
        tint: 0,
        ambient_occlusion: [0; 4],
        multipliers: [[0; 3]; 4],
        combined_light: [0; 4],
    };
    oversized.mesh = BakedSnapshot::new(1, 1, vec![quad; MAX_QUADS + 1])
        .expect("core permits larger than bridge")
        .mesh(1, [0.; 3], || false)
        .expect("mesh");
    assert!(leases.publish(oversized).is_none());
    for identity in [
        [0, 0, 1, 1, 0, 0, 0],
        [1, 0, 0, 0, 0, 0, 0],
        [1, 0, 1, 2, 0, 0, 0],
        [1, 0, 1, 1, 1, 0, 0],
    ] {
        let mut invalid = retained(1, 1);
        invalid.identity = identity;
        assert!(leases.publish(invalid).is_none());
    }
    assert_eq!(leases.metrics(), [0; 12]);
}

#[test]
fn shaded_close_and_reload_drain_pending_and_resident_before_acceptance() {
    for reload in [false, true] {
        let mut state = Renderer::new();
        let first = state.shaded.publish(retained(1, 1)).expect("resident");
        assert!(state.shaded.settle(1, first, true));
        let pending = state.shaded.publish(retained(1, 1)).expect("pending");
        let renderer = Lifecycle::new(state);
        if reload {
            assert_eq!(work::observe(&renderer, RELOAD, 1, 10, true), 2);
        } else {
            work::stop(&renderer, 1);
        }
        let mut locked = renderer.lock().expect("renderer");
        assert!(!locked.can_capture(1));
        assert!(!locked.shaded.settle(1, pending, true));
        assert_eq!(&locked.shaded.metrics()[..2], &[0, 0]);
        assert_eq!(&locked.shaded.metrics()[5..], &[0; 7]);
        assert_eq!(locked.shaded.metrics()[4], 2);
        assert_eq!(locked.can_capture(2), reload);
    }
}

#[test]
fn shaded_acceptance_refuses_same_generation_during_close_drain_or_failure() {
    let mut state = Renderer::new();
    assert!(state.can_capture(1));
    state.closed = true;
    assert!(state.accepts(1));
    assert!(!state.can_capture(1));
    state.closed = false;
    state.draining = true;
    assert!(state.accepts(1));
    assert!(!state.can_capture(1));
    state.draining = false;
    state.worker_failure = true;
    assert!(state.accepts(1));
    assert!(!state.can_capture(1));
}

#[test]
fn emission_oracle_and_reservation_refusals_are_distinct() {
    let source = input();
    let mut oracle = vec![0; 128];
    oracle[..4].copy_from_slice(&1_u32.to_le_bytes());
    assert_eq!(
        emit_and_compare(&source, &oracle, &mut Vec::new(), || true),
        BakedFailure::Emission.receipt()
    );
    assert_eq!(
        emit_and_compare(&source, &[], &mut Vec::new(), || false),
        BakedFailure::OracleHeader.receipt()
    );
    assert_eq!(
        emit_and_compare(&source, &oracle[..16], &mut Vec::new(), || false),
        BakedFailure::OraclePayload.receipt()
    );
    let failed = emit_and_compare_with(
        &source,
        &oracle,
        &mut Vec::new(),
        || false,
        |_, _| Vec::<u8>::new().try_reserve_exact(usize::MAX),
        |_| {},
    );
    assert_eq!(failed, BakedFailure::Allocation.receipt());
}

#[test]
fn reload_during_comparison_or_before_commit_cannot_publish() {
    let renderer = Lifecycle::new(Renderer::new());
    let comparing = std::cell::Cell::new(false);
    let mut output = Vec::new();
    let result = emit_and_compare_with(
        &input(),
        &[0; 128],
        &mut output,
        || {
            if comparing.replace(false) {
                work::observe(&renderer, RELOAD, 1, 10, true);
            }
            !renderer.lock().expect("renderer").can_capture(1)
        },
        |words, size| {
            comparing.set(true);
            words.try_reserve_exact(size)
        },
        |_| {},
    );
    assert_eq!(result, BakedFailure::Emission.receipt());
    assert_eq!(output, [0; 4]);
    let prepared = Prepared {
        source: input(),
        emitted: vec![1; 32],
        result: [1, 0, -1, 0, 0, 0],
        mesh: None,
    };
    let receipt = commit(&renderer, 1, [0; 3], prepared, |_| -> Result<(), ()> {
        panic!("stale publication must not write Java output");
    })
    .expect("stale refusal");
    assert_eq!(receipt, Vec::<i64>::new());
    assert_eq!(renderer.lock().expect("renderer").shaded.metrics(), [0; 12]);
}

fn nonzero_input() -> Vec<u8> {
    let mut bytes = input();
    let mut set =
        |offset: usize, value: u32| bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    set(4, 2_f32.to_bits());
    set(8, 3_f32.to_bits());
    set(12, 4_f32.to_bits());
    set(32, 37);
    set(36, 41);
    set(40, 2);
    set(44, 0x00ab_cdef);
    for axis in 0..3 {
        let bits = f64::from(u32::try_from(axis + 1).expect("axis")).to_bits();
        set(
            48 + axis * 8,
            u32::try_from(bits & 0xffff_ffff).expect("low word"),
        );
        set(52 + axis * 8, u32::try_from(bits >> 32).expect("high word"));
    }
    for vertex in 0..4 {
        for item in 0..7 {
            let value = if item == 3 {
                0xff12_3456
            } else {
                f32::from(u16::try_from(vertex * 7 + item + 1).expect("field")).to_bits()
            };
            set(72 + (vertex * 7 + item) * 4, value);
        }
        for (axis, scale) in [0.125, 0.25, 0.5].into_iter().enumerate() {
            set(
                200 + (vertex * 3 + axis) * 4,
                (f32::from(u16::try_from(vertex + 1).expect("vertex")) * scale).to_bits(),
            );
        }
        set(
            184 + vertex * 4,
            f32::from(u16::try_from(vertex + 1).expect("vertex")).to_bits(),
        );
        set(248 + vertex * 4, u32::try_from(vertex + 91).expect("light"));
    }
    bytes
}

#[test]
fn copied_nonzero_fields_survive_decode_and_emission() {
    let bytes = nonzero_input();
    let (snapshot, camera) = decode(&bytes).expect("nonzero packet");
    assert_eq!(camera, [2., 3., 4.]);
    let mesh = snapshot.mesh(1, camera, || false).expect("emission");
    let quad = &mesh.layers()[2][0];
    assert_eq!(
        (quad.state_key, quad.sprite, quad.tint),
        (37, 41, 0x00ab_cdef)
    );
    for vertex in 0..4 {
        for (axis, coordinate) in quad.vertices[vertex].iter().take(3).enumerate() {
            assert_eq!(
                f32::from_bits(*coordinate),
                f32::from(u16::try_from(vertex * 7 + axis + axis + 2).expect("coordinate"))
            );
        }
        assert_eq!(
            quad.vertices[vertex][3],
            [0xff09_0d0a, 0xff12_1a15, 0xff1b_2720, 0xff24_342b][vertex]
        );
        assert_eq!(
            quad.vertices[vertex][4],
            f32::from(u16::try_from(vertex * 7 + 5).expect("u")).to_bits()
        );
        assert_eq!(
            quad.vertices[vertex][5],
            f32::from(u16::try_from(vertex * 7 + 6).expect("v")).to_bits()
        );
        assert_eq!(
            quad.vertices[vertex][6],
            u32::try_from(vertex + 91).expect("light")
        );
        assert_eq!(
            quad.ambient_occlusion[vertex],
            f32::from(u16::try_from(vertex + 1).expect("ao")).to_bits()
        );
    }
}

#[test]
fn java_python_and_rust_crossing_contracts_match() {
    let java = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../jvm/legacy-platform/src/agent/java/dev/ravel/jvm/LegacyRendererContract.java"
    ));
    let python = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../jvm/legacy-platform/tools/renderer_proof.py"
    ));
    let benchmark = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../registry/sevtech/bench/run.py"
    ));
    let geometry = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../jvm/legacy-platform/src/main/java/dev/ravel/legacy/opengl/GeometryProof.java"
    ));
    let automation = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../jvm/legacy-platform/src/main/java/dev/ravel/legacy/launch/ClientAutomation.java"
    ));
    assert!(geometry.contains(&format!("MAX_BAKED_QUADS = {MAX_QUADS};")));
    assert!(automation.contains("GeometryProof.MAX_BAKED_QUADS"));
    for (name, index) in [
        (
            "SNAPSHOT_MISMATCHES",
            super::super::status::SNAPSHOT_MISMATCHES,
        ),
        ("SNAPSHOT_ADMITTED", super::super::status::SNAPSHOT_ADMITTED),
    ] {
        assert!(java.contains(&format!("{name} = {index};")));
    }
    assert!(java.contains(&format!("BAKED_RECORD_BYTES = {RECORD};")));
    assert!(java.contains(&format!("BAKED_MAX_QUADS = {MAX_QUADS};")));
    for source in [python, benchmark] {
        assert!(source.contains(&format!("BAKED_RECORD_BYTES = {RECORD}\n")));
        assert!(source.contains(&format!("BAKED_MAX_QUADS_PER_SECTION = {MAX_QUADS}\n")));
        assert!(source.contains(
            "BAKED_GL_RESERVED_BYTES = 3 * BAKED_MAX_QUADS_PER_SECTION * BAKED_QUAD_BYTES\n"
        ));
        assert!(source.contains("PROOF_IMAGE_EDGE = 64\n"));
        assert!(source.contains("PROOF_IMAGE_PIXELS = PROOF_IMAGE_EDGE ** 2\n"));
    }
}

#[test]
fn java_and_rust_status_contracts_match() {
    use super::super::status::SnapshotFailure;
    let java = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../jvm/legacy-platform/src/agent/java/dev/ravel/jvm/LegacyRendererContract.java"
    ));
    for (name, failure) in [
        ("BOUNDS", BakedFailure::Bounds),
        ("DECODE", BakedFailure::Decode),
        ("EMISSION", BakedFailure::Emission),
        ("ORACLE_HEADER", BakedFailure::OracleHeader),
        ("LAYER_COUNT", BakedFailure::LayerCount),
        ("ORACLE_PAYLOAD", BakedFailure::OraclePayload),
        ("ALLOCATION", BakedFailure::Allocation),
    ] {
        assert!(java.contains(&format!("{name}({})", failure.receipt()[1])));
    }
    for (name, failure) in [
        ("GENERATION", SnapshotFailure::Generation),
        ("LENGTHS", SnapshotFailure::Lengths),
        ("STATE", SnapshotFailure::State),
        ("GEOMETRY", SnapshotFailure::Geometry),
        (
            "INCONSISTENT_GEOMETRY",
            SnapshotFailure::InconsistentGeometry,
        ),
        ("PALETTE", SnapshotFailure::Palette),
        ("NULL_INPUT", SnapshotFailure::NullInput),
    ] {
        assert!(java.contains(&format!("{name}({})", i64::from(failure))));
    }
}

#[test]
fn injected_capture_admission_refuses_busy_stale_and_overlapping_calls() {
    let renderer = Lifecycle::new(Renderer::new());
    assert_eq!(admit(&renderer, 1, || Ok::<_, ()>(7)), Ok(Some(7)));
    assert_eq!(admit(&renderer, 0, || Ok::<_, ()>(7)), Ok(None));
    let permit = renderer.capture.lock().expect("permit");
    assert_eq!(admit(&renderer, 1, || Ok::<_, ()>(7)), Ok(None));
    drop(permit);
    renderer
        .lock()
        .expect("renderer")
        .shaded
        .publish(retained(1, 1))
        .expect("pending");
    assert_eq!(admit(&renderer, 1, || Ok::<_, ()>(7)), Ok(None));
    let output = 4 + MAX_QUADS * 28;
    assert!(valid_bounds(INPUT_BOUND, ORACLE_BOUND, output));
    assert!(!valid_bounds(INPUT_BOUND + 1, ORACLE_BOUND, output));
    assert!(!valid_bounds(INPUT_BOUND, ORACLE_BOUND + 1, output));
    assert!(!valid_bounds(INPUT_BOUND, ORACLE_BOUND, output - 1));
}
