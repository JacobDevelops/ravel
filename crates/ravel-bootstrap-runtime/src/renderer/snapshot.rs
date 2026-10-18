//! One bounded observational batch at a time; nothing is retained after comparison.
//! Metrics count payload bytes, not allocator metadata or JVM array headers.
//! Scratch counts fixed heap and stack arrays, excluding scalar locals and frame overhead.

use std::sync::{Mutex, PoisonError, TryLockError};

use jni::Env;
use jni::objects::{JByteArray, JIntArray};
use jni::sys::jlong;
use ravel_renderer::{CELL_COUNT, MAX_CAPTURE_SCRATCH_BYTES, SectionSnapshot};

use super::Renderer;

static CAPTURE: Mutex<()> = Mutex::new(());

type Receipt = [jlong; 6];

/// The permit bounds concurrent JNI buffers and palette scratch to one batch.
/// Both lifecycle checks use the invalidation mutex; the final check is the
/// admission linearization point, not a lease authorizing later publication.
pub(super) fn admit(
    renderer: &Mutex<Renderer>,
    generation: jlong,
    capture: impl FnOnce() -> jni::errors::Result<Option<Receipt>>,
) -> jni::errors::Result<Option<Receipt>> {
    let _permit = match CAPTURE.try_lock() {
        Ok(permit) => permit,
        Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
        Err(TryLockError::WouldBlock) => return Ok(None),
    };
    if !renderer
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .accepts(generation)
    {
        return Ok(None);
    }
    let receipt = capture()?;
    let locked = renderer.lock().unwrap_or_else(PoisonError::into_inner);
    Ok(receipt.filter(|_| locked.accepts(generation)))
}

pub(super) fn copy_and_compare(
    env: &Env<'_>,
    generation: jlong,
    states: &JIntArray<'_>,
    light: &JByteArray<'_>,
) -> jni::errors::Result<Option<Receipt>> {
    if states.is_null() || light.is_null() || !valid_lengths(states.len(env)?, light.len(env)?) {
        return Ok(None);
    }
    // Check both lengths before allocating either bounded copy. Region APIs do
    // not pin or expose Java memory, and no object escapes this call.
    let mut copied_states = vec![0; CELL_COUNT];
    states.get_region(env, 0, &mut copied_states)?;
    let copied_light = env.convert_byte_array(light)?;
    Ok(compare(generation, &copied_states, &copied_light))
}

fn valid_lengths(states: usize, light: usize) -> bool {
    states == CELL_COUNT && light == CELL_COUNT
}

fn compare(generation: jlong, states: &[i32], light: &[u8]) -> Option<Receipt> {
    let generation = u64::try_from(generation).ok()?;
    let snapshot = SectionSnapshot::capture(generation, states, light).ok()?;
    let mismatches = mismatched_cells(&snapshot, states, light);
    Some([
        1,
        jlong::try_from(snapshot.total_bytes()).ok()?,
        jlong::try_from(mismatches).ok()?,
        jlong::try_from(CELL_COUNT).ok()?,
        jlong::try_from(CELL_COUNT * (size_of::<i32>() + size_of::<u8>())).ok()?,
        jlong::try_from(MAX_CAPTURE_SCRATCH_BYTES).ok()?,
    ])
}

fn mismatched_cells(snapshot: &SectionSnapshot, states: &[i32], light: &[u8]) -> usize {
    states
        .iter()
        .zip(light)
        .enumerate()
        .filter(|(index, (state, light))| {
            snapshot.cell(*index).is_none_or(|cell| {
                i32::try_from(cell.state_key()).ok() != Some(**state)
                    || cell.packed_light() != **light
            })
        })
        .count()
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use std::sync::{Mutex, PoisonError};

    use super::{CELL_COUNT, SectionSnapshot, admit, compare, mismatched_cells, valid_lengths};
    use crate::renderer::{FRAME, RELOAD, Renderer, UNLOAD};

    #[test]
    fn format_requires_exact_bordered_lengths_and_valid_state_keys() {
        for length in [0, CELL_COUNT - 1, CELL_COUNT + 1, usize::MAX] {
            assert!(!valid_lengths(length, CELL_COUNT));
            assert!(!valid_lengths(CELL_COUNT, length));
        }
        assert!(valid_lengths(CELL_COUNT, CELL_COUNT));
        let mut states = vec![0; CELL_COUNT];
        let light = vec![255; CELL_COUNT];
        for invalid in [-1, 0x0100_0000, i32::MAX] {
            states[CELL_COUNT - 1] = invalid;
            assert!(compare(1, &states, &light).is_none());
        }
        states[CELL_COUNT - 1] = 0x00ff_ffff;
        assert!(compare(1, &states, &light).is_some());
        for generation in [-1, 0] {
            assert!(compare(generation, &states, &light).is_none());
        }
        assert!(compare(1, &states[..CELL_COUNT - 1], &light).is_none());
        assert!(compare(1, &states, &light[..CELL_COUNT - 1]).is_none());
    }

    #[test]
    fn decoded_receipt_compares_every_state_and_light_cell() {
        let states: Vec<_> = (0..CELL_COUNT)
            .map(|index| i32::try_from(index).expect("bounded"))
            .collect();
        let light: Vec<_> = (0..CELL_COUNT)
            .map(|index| u8::try_from(index % 256).expect("byte"))
            .collect();
        let receipt = compare(1, &states, &light).expect("valid snapshot");
        assert_eq!(receipt[0], 1);
        assert_eq!(
            receipt[1],
            i64::try_from(ravel_renderer::MAX_TOTAL_BYTES).expect("bounded snapshot")
        );
        assert_eq!(receipt[2], 0);
        assert_eq!(receipt[3], 5832);
        assert_eq!(receipt[4], 29160);
        assert_eq!(
            receipt[5],
            i64::try_from(ravel_renderer::MAX_CAPTURE_SCRATCH_BYTES).expect("bounded scratch")
        );
    }

    #[test]
    fn comparison_detects_state_and_light_errors_including_last_cell() {
        let mut states = vec![0; CELL_COUNT];
        let mut light = vec![0; CELL_COUNT];
        let snapshot = SectionSnapshot::capture(1, &states, &light).expect("snapshot");
        states[0] = 1;
        light[CELL_COUNT - 1] = 1;
        assert_eq!(mismatched_cells(&snapshot, &states, &light), 2);
        states[CELL_COUNT - 1] = 2;
        assert_eq!(mismatched_cells(&snapshot, &states, &light), 2);
        light[CELL_COUNT >> 1] = 255;
        assert_eq!(mismatched_cells(&snapshot, &states, &light), 3);
    }

    #[test]
    fn admission_rejects_stale_before_capture_and_invalidation_during_capture() {
        let renderer = Mutex::new(Renderer::new());
        let called = Cell::new(false);
        for generation in [-1, 0, 2] {
            assert!(
                admit(&renderer, generation, || {
                    called.set(true);
                    Ok(Some([1, 10, 0, 5832, 29160, 51376]))
                })
                .expect("admission")
                .is_none()
            );
        }
        assert!(!called.get());
        renderer
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .observe(FRAME, 1, 10, true);
        for (event, generation) in [(RELOAD, 1), (UNLOAD, 2)] {
            let receipt = admit(&renderer, generation, || {
                renderer
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .observe(event, generation, 10, true);
                Ok(Some([1, 10, 0, 5832, 29160, 51376]))
            })
            .expect("admission");
            assert!(receipt.is_none());
            assert!(
                admit(&renderer, generation, || panic!(
                    "stale capture must not run"
                ))
                .expect("admission")
                .is_none()
            );
        }
        assert_eq!(
            admit(&renderer, 3, || {
                // A reentrant/concurrent call cannot allocate another batch.
                assert!(
                    admit(&renderer, 3, || panic!("busy capture must not run"))
                        .expect("busy rejection")
                        .is_none()
                );
                Ok(Some([1, 10, 0, 5832, 29160, 51376]))
            })
            .expect("current admission"),
            Some([1, 10, 0, 5832, 29160, 51376])
        );
    }
}
