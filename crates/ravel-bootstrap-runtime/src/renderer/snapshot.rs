//! One bounded JNI capture at a time, with owned snapshots transferred to workers.
//! Metrics count payload bytes, not allocator metadata or JVM array headers.
//! Scratch counts fixed heap and stack arrays, excluding scalar locals and frame overhead.

use std::sync::{Mutex, PoisonError, TryLockError};

use jni::Env;
use jni::objects::{JByteArray, JIntArray};
use jni::sys::jlong;
use ravel_renderer::{CELL_COUNT, MAX_CAPTURE_SCRATCH_BYTES, SectionSnapshot, SnapshotError};

use super::Lifecycle;
use super::work::BoundaryError;

static CAPTURE: Mutex<()> = Mutex::new(());

type Receipt = [jlong; 8];

pub(super) struct Captured {
    receipt: [jlong; 6],
    snapshot: Option<SectionSnapshot>,
}

/// The permit bounds concurrent JNI buffers and palette scratch to one batch.
/// Both lifecycle checks use the invalidation mutex; the final check is the
/// admission linearization point. Its receipt acknowledges ownership, not a mesh result.
pub(super) fn admit(
    renderer: &Lifecycle,
    generation: jlong,
    key: jlong,
    capture: impl FnOnce() -> jni::errors::Result<Option<Captured>>,
) -> Result<Option<Receipt>, BoundaryError> {
    let _permit = match CAPTURE.try_lock() {
        Ok(permit) => permit,
        Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
        Err(TryLockError::WouldBlock) => return Ok(None),
    };
    if key <= 0
        || !renderer
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .can_capture(generation)
    {
        return Ok(None);
    }
    let Some(captured) = capture()? else {
        return Ok(None);
    };
    let mut locked = renderer.lock().unwrap_or_else(PoisonError::into_inner);
    if !locked.can_capture(generation) {
        return Ok(None);
    }
    let admitted = match captured.snapshot {
        Some(snapshot) if captured.receipt[2] == 0 => locked.submit(snapshot, key)?,
        _ => false,
    };
    drop(locked);
    let mut receipt = [0; 8];
    receipt[..6].copy_from_slice(&captured.receipt);
    receipt[6] = jlong::from(admitted);
    receipt[7] = key;
    Ok(Some(receipt))
}

pub(super) fn copy_and_compare(
    env: &Env<'_>,
    generation: jlong,
    states: &JIntArray<'_>,
    light: &JByteArray<'_>,
    geometry: &JByteArray<'_>,
) -> jni::errors::Result<Option<Captured>> {
    let nulls = u8::from(states.is_null())
        | (u8::from(light.is_null()) << 1)
        | (u8::from(geometry.is_null()) << 2);
    if nulls != 0 {
        return Ok(Some(failure(7, [jlong::from(nulls), 0, 0])));
    }
    let lengths = (states.len(env)?, light.len(env)?, geometry.len(env)?);
    if !valid_lengths(lengths.0, lengths.1, lengths.2) {
        return Ok(Some(capture_error(SnapshotError::InvalidLengths {
            states: lengths.0,
            light: lengths.1,
            geometry: lengths.2,
        })));
    }
    // Validate all lengths before allocating bounded copies. Region APIs do
    // not pin or expose Java memory, and no object escapes this call.
    let mut copied_states = vec![0; CELL_COUNT];
    states.get_region(env, 0, &mut copied_states)?;
    let copied_light = env.convert_byte_array(light)?;
    let copied_geometry = env.convert_byte_array(geometry)?;
    Ok(compare(
        generation,
        &copied_states,
        &copied_light,
        &copied_geometry,
    ))
}

fn valid_lengths(states: usize, light: usize, geometry: usize) -> bool {
    states == CELL_COUNT && light == CELL_COUNT && geometry == CELL_COUNT
}

fn compare(generation: jlong, states: &[i32], light: &[u8], geometry: &[u8]) -> Option<Captured> {
    let generation = u64::try_from(generation).unwrap_or(0);
    let snapshot = match SectionSnapshot::capture(generation, states, light, geometry) {
        Ok(snapshot) => snapshot,
        Err(error) => return Some(capture_error(error)),
    };
    let mismatches = mismatched_cells(&snapshot, states, light, geometry);
    let copy_bytes = CELL_COUNT * (size_of::<i32>() + 2 * size_of::<u8>());
    let receipt = [
        1,
        jlong::try_from(snapshot.total_bytes()).ok()?,
        jlong::try_from(mismatches).ok()?,
        jlong::try_from(CELL_COUNT).ok()?,
        jlong::try_from(copy_bytes).ok()?,
        jlong::try_from(MAX_CAPTURE_SCRATCH_BYTES + copy_bytes).ok()?,
    ];
    Some(Captured {
        receipt,
        snapshot: Some(snapshot),
    })
}

fn capture_error(error: SnapshotError) -> Captured {
    match error {
        SnapshotError::InvalidGeneration => failure(1, [0; 3]),
        SnapshotError::InvalidLengths {
            states,
            light,
            geometry,
        } => failure(
            2,
            [size_wire(states), size_wire(light), size_wire(geometry)],
        ),
        SnapshotError::InvalidState { index, state } => {
            failure(3, [size_wire(index), jlong::from(state), 0])
        }
        SnapshotError::InvalidGeometry { index, flag } => {
            failure(4, [size_wire(index), jlong::from(flag), 0])
        }
        SnapshotError::InconsistentGeometry { state } => failure(5, [jlong::from(state), 0, 0]),
        SnapshotError::PaletteLimit => failure(6, [0; 3]),
    }
}

fn size_wire(value: usize) -> jlong {
    jlong::try_from(value).unwrap_or(jlong::MAX)
}

fn failure(code: jlong, details: [jlong; 3]) -> Captured {
    Captured {
        receipt: [-1, code, details[0], details[1], details[2], 0],
        snapshot: None,
    }
}

fn mismatched_cells(
    snapshot: &SectionSnapshot,
    states: &[i32],
    light: &[u8],
    geometry: &[u8],
) -> usize {
    states
        .iter()
        .zip(light)
        .zip(geometry)
        .enumerate()
        .filter(|(index, ((state, light), geometry))| {
            snapshot.cell(*index).is_none_or(|cell| {
                i32::try_from(cell.state_key()).ok() != Some(**state)
                    || cell.packed_light() != **light
                    || u8::from(cell.is_full_opaque_cube()) != **geometry
            })
        })
        .count()
}

pub(super) fn publish<T, E>(
    renderer: &Lifecycle,
    generation: jlong,
    receipt: Receipt,
    publish: impl FnOnce(&[jlong]) -> Result<T, E>,
) -> Result<T, E> {
    let locked = renderer.lock().unwrap_or_else(PoisonError::into_inner);
    let admitted = receipt[6] == 1;
    // Cancellation can discard the mesh, but cannot revoke admission Java must settle.
    let values = if admitted || locked.can_capture(generation) {
        &receipt[..]
    } else {
        &[]
    };
    let result = publish(values);
    drop(locked);
    if result.is_err() && admitted {
        // A failed receipt cannot leave Java-unacknowledged native ownership.
        super::work::stop(renderer, generation);
    }
    result
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::sync::{Mutex, PoisonError};

    use super::{CELL_COUNT, SectionSnapshot, admit, compare, mismatched_cells, valid_lengths};
    use crate::renderer::{FRAME, Lifecycle, RELOAD, Renderer, UNLOAD};

    static CAPTURE_TESTS: Mutex<()> = Mutex::new(());

    fn captured() -> super::Captured {
        compare(
            1,
            &vec![0; CELL_COUNT],
            &vec![0; CELL_COUNT],
            &vec![0; CELL_COUNT],
        )
        .expect("snapshot")
    }

    fn diagnostic(captured: Option<super::Captured>) -> [jni::sys::jlong; 6] {
        let captured = captured.expect("diagnostic receipt");
        assert!(captured.snapshot.is_none());
        assert_eq!(captured.receipt[0], -1);
        captured.receipt
    }

    #[test]
    fn format_requires_exact_bordered_lengths_and_valid_state_keys() {
        for length in [0, CELL_COUNT - 1, CELL_COUNT + 1, usize::MAX] {
            assert!(!valid_lengths(length, CELL_COUNT, CELL_COUNT));
            assert!(!valid_lengths(CELL_COUNT, length, CELL_COUNT));
            assert!(!valid_lengths(CELL_COUNT, CELL_COUNT, length));
        }
        assert!(valid_lengths(CELL_COUNT, CELL_COUNT, CELL_COUNT));
        let mut states = vec![0; CELL_COUNT];
        let light = vec![255; CELL_COUNT];
        let geometry = vec![0; CELL_COUNT];
        for invalid in [-1, 0x0100_0000, i32::MAX] {
            states[CELL_COUNT - 1] = invalid;
            assert_eq!(
                diagnostic(compare(1, &states, &light, &geometry)),
                [-1, 3, 5831, i64::from(invalid), 0, 0]
            );
        }
        states[CELL_COUNT - 1] = 0x00ff_ffff;
        assert_eq!(
            compare(1, &states, &light, &geometry)
                .expect("capture")
                .receipt[0],
            1
        );
        for generation in [-1, 0] {
            assert_eq!(
                diagnostic(compare(generation, &states, &light, &geometry)),
                [-1, 1, 0, 0, 0, 0]
            );
        }
        assert_eq!(
            diagnostic(compare(1, &states[..CELL_COUNT - 1], &light, &geometry)),
            [-1, 2, 5831, 5832, 5832, 0]
        );
        assert_eq!(
            diagnostic(compare(1, &states, &light[..CELL_COUNT - 1], &geometry)),
            [-1, 2, 5832, 5831, 5832, 0]
        );
        assert_eq!(
            diagnostic(compare(1, &states, &light, &geometry[..CELL_COUNT - 1])),
            [-1, 2, 5832, 5832, 5831, 0]
        );
        let mut malformed_geometry = geometry;
        malformed_geometry[CELL_COUNT - 1] = 2;
        assert_eq!(
            diagnostic(compare(1, &states, &light, &malformed_geometry)),
            [-1, 4, 5831, 2, 0, 0]
        );
        malformed_geometry[CELL_COUNT - 1] = 0;
        malformed_geometry[0] = 1;
        assert_eq!(
            diagnostic(compare(1, &states, &light, &malformed_geometry)),
            [-1, 5, 0, 0, 0, 0]
        );
        states.fill(35);
        assert_eq!(
            diagnostic(compare(1, &states, &light, &malformed_geometry)),
            [-1, 5, 35, 0, 0, 0]
        );
    }

    #[test]
    fn decoded_receipt_compares_every_state_light_and_geometry_cell() {
        let states: Vec<_> = (0..CELL_COUNT)
            .map(|index| i32::try_from(index).expect("bounded"))
            .collect();
        let light: Vec<_> = (0..CELL_COUNT)
            .map(|index| u8::try_from(index % 256).expect("byte"))
            .collect();
        let geometry = vec![0; CELL_COUNT];
        let receipt = compare(1, &states, &light, &geometry)
            .expect("valid snapshot")
            .receipt;
        assert_eq!(receipt[0], 1);
        assert_eq!(
            receipt[1],
            i64::try_from(ravel_renderer::MAX_TOTAL_BYTES).expect("bounded snapshot")
        );
        assert_eq!(receipt[2], 0);
        assert_eq!(receipt[3], 5832);
        assert_eq!(receipt[4], 34992);
        assert_eq!(
            receipt[5],
            i64::try_from(ravel_renderer::MAX_CAPTURE_SCRATCH_BYTES + 34992)
                .expect("bounded scratch")
        );
    }

    #[test]
    fn comparison_detects_state_light_and_geometry_errors_including_last_cell() {
        let mut states = vec![0; CELL_COUNT];
        let mut light = vec![0; CELL_COUNT];
        let mut geometry = vec![0; CELL_COUNT];
        let snapshot = SectionSnapshot::capture(1, &states, &light, &geometry).expect("snapshot");
        states[0] = 1;
        light[CELL_COUNT - 1] = 1;
        assert_eq!(mismatched_cells(&snapshot, &states, &light, &geometry), 2);
        states[CELL_COUNT - 1] = 2;
        assert_eq!(mismatched_cells(&snapshot, &states, &light, &geometry), 2);
        light[CELL_COUNT >> 1] = 255;
        assert_eq!(mismatched_cells(&snapshot, &states, &light, &geometry), 3);
        geometry[1] = 1;
        assert_eq!(mismatched_cells(&snapshot, &states, &light, &geometry), 4);
    }

    #[test]
    fn admission_rejects_stale_before_capture_and_invalidation_during_capture() {
        let _serial = CAPTURE_TESTS.lock().expect("capture test serialization");
        let renderer = Lifecycle::new(Renderer::new());
        let called = Cell::new(false);
        for generation in [-1, 0, 2] {
            assert!(
                admit(&renderer, generation, 1, || {
                    called.set(true);
                    Ok(Some(captured()))
                })
                .expect("admission")
                .is_none()
            );
        }
        assert!(!called.get());
        for key in [-1, 0] {
            assert!(
                admit(&renderer, 1, key, || panic!("invalid key capture"))
                    .expect("invalid key")
                    .is_none()
            );
        }
        renderer
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .observe(FRAME, 1, 10, true);
        for (event, generation) in [(RELOAD, 1), (UNLOAD, 2)] {
            let receipt = admit(&renderer, generation, 1, || {
                renderer
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .observe(event, generation, 10, true);
                Ok(Some(captured()))
            })
            .expect("admission");
            assert!(receipt.is_none());
            assert!(
                admit(&renderer, generation, 1, || panic!("stale capture"))
                    .expect("admission")
                    .is_none()
            );
        }
    }

    #[test]
    fn admission_reports_receipts_and_closes_after_shutdown() {
        let _serial = CAPTURE_TESTS.lock().expect("capture test serialization");
        let renderer = Lifecycle::new(Renderer::new());
        renderer
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .observe(FRAME, 1, 10, true);
        for generation in 1..3 {
            renderer
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .observe(RELOAD, generation, 10, true);
        }
        let receipt = admit(&renderer, 3, 7, || {
            assert!(
                admit(&renderer, 3, 8, || panic!("busy capture"))
                    .expect("busy rejection")
                    .is_none()
            );
            Ok(compare(
                3,
                &vec![0; CELL_COUNT],
                &vec![0; CELL_COUNT],
                &vec![0; CELL_COUNT],
            ))
        })
        .expect("current admission")
        .expect("receipt");
        assert_eq!(&receipt[6..], [1, 7]);
        let mismatch = admit(&renderer, 3, 8, || {
            let mut captured = captured();
            captured.receipt[2] = 1;
            Ok(Some(captured))
        })
        .expect("mismatch")
        .expect("receipt");
        assert_eq!(mismatch[6], 0);
        let diagnostic = admit(&renderer, 3, 9, || {
            Ok(Some(super::capture_error(
                ravel_renderer::SnapshotError::InconsistentGeometry { state: 35 },
            )))
        })
        .expect("capture failure")
        .expect("diagnostic");
        assert_eq!(diagnostic, [-1, 5, 35, 0, 0, 0, 0, 9]);
        super::super::work::exchange(&renderer, 2, 3, 0).expect("shutdown");
        assert!(
            admit(&renderer, 3, 10, || panic!("closed capture"))
                .expect("closed generation")
                .is_none()
        );
    }
}
