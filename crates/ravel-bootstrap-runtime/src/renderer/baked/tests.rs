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

fn retained(generation: jlong, resource: jlong) -> residency::Retained {
    residency::Retained {
        identity: [generation, 0, resource, resource, -16, 32, 48],
        source: input(),
        emitted: vec![1, 0, 0, 0, 42],
    }
}

#[test]
fn shaded_replacement_requires_exact_acknowledgement_and_keeps_owned_source() {
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
    oversized.source = vec![0; INPUT_BOUND + 1];
    assert!(leases.publish(oversized).is_none());
    let mut oversized = retained(1, 1);
    oversized.emitted = vec![0; 5 + MAX_QUADS * 28];
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
