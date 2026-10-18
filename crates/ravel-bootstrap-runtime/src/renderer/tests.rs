use std::sync::{Barrier, Mutex, PoisonError};
use std::thread;

use super::{
    COMPILE, CURRENT, FRAME, LAYER, RELOAD, Renderer, TASK, TERRAIN, UNLOAD, UPDATE, UPLOAD,
};

#[test]
fn generations_are_positive_and_reload_and_unload_invalidate_previous_work() {
    let mut renderer = Renderer::new();
    assert_eq!(renderer.observe(CURRENT, 0, 0, false), 1);
    assert_eq!(renderer.observe(CURRENT, 1, 20, true), 1);
    assert_eq!(renderer.owner_thread, 0);
    assert!(!renderer.admits(COMPILE, 1, 20, false));
    assert!(!renderer.accepts(0));
    assert!(!renderer.accepts(-1));
    assert!(renderer.accepts(1));
    assert_eq!(renderer.observe(RELOAD, 1, 10, true), 2);
    assert!(!renderer.accepts(1));
    assert!(renderer.accepts(2));
    assert_eq!(renderer.observe(UNLOAD, 2, 10, true), 3);
    assert!(!renderer.accepts(2));
    assert!(renderer.accepts(3));
}

#[test]
fn owner_observations_claim_one_thread_and_workers_cannot_claim_it() {
    let mut renderer = Renderer::new();
    for event in [UNLOAD, TERRAIN, LAYER, UPDATE, COMPILE, TASK, UPLOAD] {
        assert!(!renderer.admits(event, 1, 20, true));
        renderer.observe(event, 1, 20, true);
    }
    assert!(!renderer.admits(TASK, 1, 20, false));
    renderer.observe(TASK, 1, 20, false);
    for event in [RELOAD, UNLOAD, FRAME, TERRAIN, LAYER, UPDATE, UPLOAD] {
        assert!(!renderer.admits(event, 1, 10, false));
        assert!(!renderer.admits(event, 1, 0, true));
        renderer.observe(event, 1, 10, false);
        renderer.observe(event, 1, 0, true);
    }
    assert_eq!(renderer.owner_thread, 0);
    renderer.observe(FRAME, 1, 10, true);
    for event in [RELOAD, UNLOAD, FRAME, TERRAIN, LAYER, UPDATE, UPLOAD] {
        assert!(!renderer.admits(event, 1, 20, true));
        assert!(!renderer.admits(event, 1, 10, false));
        renderer.observe(event, 1, 20, true);
        renderer.observe(event, 1, 10, false);
    }
    assert_eq!(renderer.generation, 1);
    assert_eq!(renderer.owner_thread, 10);
    for event in [FRAME, TERRAIN, LAYER, UPDATE, UPLOAD] {
        assert!(renderer.admits(event, 1, 10, true));
    }
    for event in [COMPILE, TASK] {
        assert!(renderer.admits(event, 1, 20, false));
        assert!(renderer.admits(event, 1, 10, true));
        assert!(!renderer.admits(event, 1, 20, true));
    }
}

#[test]
fn stale_and_unknown_events_do_not_mutate_generation_or_ownership() {
    let mut renderer = Renderer::new();
    assert!(!renderer.admits(FRAME, 0, 20, true));
    assert!(!renderer.admits(99, 1, 20, true));
    renderer.observe(FRAME, 0, 20, true);
    renderer.observe(99, 1, 20, true);
    assert_eq!(renderer.owner_thread, 0);
    renderer.observe(RELOAD, 1, 10, true);
    for event in [
        RELOAD, UNLOAD, FRAME, TERRAIN, LAYER, UPDATE, COMPILE, TASK, UPLOAD,
    ] {
        assert!(!renderer.admits(event, 1, 10, true));
        assert!(!renderer.admits(event, 1, 20, false));
        assert_eq!(renderer.observe(event, 1, 10, true), 2);
        assert_eq!(renderer.observe(event, 1, 20, false), 2);
    }
    assert_eq!(renderer.owner_thread, 10);
    assert!(!renderer.accepts(1));
}

#[test]
fn worker_completion_rechecks_after_invalidation_between_separate_native_calls() {
    for invalidation in [RELOAD, UNLOAD] {
        let mut renderer = Renderer::new();
        renderer.observe(FRAME, 1, 10, true);
        let renderer = Mutex::new(renderer);
        let checked = Barrier::new(2);
        let invalidated = Barrier::new(2);
        thread::scope(|scope| {
            let worker = scope.spawn(|| {
                // Model distinct accept/event calls, not a lease across calls.
                let accepted = renderer
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .accepts(1);
                checked.wait();
                invalidated.wait();
                let mut locked = renderer.lock().unwrap_or_else(PoisonError::into_inner);
                let admitted = locked.admits(COMPILE, 1, 20, false);
                let current = locked.observe(COMPILE, 1, 20, false);
                drop(locked);
                (accepted, admitted, current)
            });
            checked.wait();
            let generation = {
                let mut locked = renderer.lock().unwrap_or_else(PoisonError::into_inner);
                locked.observe(invalidation, 1, 10, true)
            };
            invalidated.wait();
            let (accepted, admitted, current) = worker.join().expect("worker completes");
            assert!(accepted);
            assert!(!admitted);
            assert_eq!(current, 2);
            assert_eq!(generation, 2);
            let locked = renderer.lock().unwrap_or_else(PoisonError::into_inner);
            assert!(!locked.accepts(1));
            assert!(!locked.admits(TASK, 1, 20, false));
            assert_eq!(locked.owner_thread, 10);
        });
    }
}

#[test]
fn generation_exhaustion_fails_closed_without_reusing_a_token() {
    let mut renderer = Renderer::new();
    renderer.generation = i64::MAX;
    assert!(renderer.accepts(i64::MAX));
    assert_eq!(renderer.observe(RELOAD, i64::MAX, 10, true), 0);
    assert!(!renderer.accepts(0));
    assert!(!renderer.accepts(i64::MAX));
    assert_eq!(renderer.observe(RELOAD, 0, 10, true), 0);
    assert_eq!(renderer.observe(FRAME, 1, 10, true), 0);
}

#[test]
fn invalid_worker_scalars_and_future_generations_are_refused() {
    let mut renderer = Renderer::new();
    renderer.observe(FRAME, 1, 10, true);
    for event in [COMPILE, TASK] {
        for thread_id in [0, -1] {
            assert!(!renderer.admits(event, 1, thread_id, false));
            assert_eq!(renderer.observe(event, 1, thread_id, false), 1);
        }
        assert!(!renderer.admits(event, 2, 20, false));
        assert_eq!(renderer.observe(event, 2, 20, false), 1);
    }
    assert!(!renderer.accepts(2));
    assert_eq!(renderer.owner_thread, 10);
}

#[test]
fn worker_admission_closes_after_each_invalidation_and_reopens_for_the_new_token() {
    let mut renderer = Renderer::new();
    renderer.observe(FRAME, 1, 10, true);
    for (event, generation) in [(RELOAD, 1), (UNLOAD, 2)] {
        assert!(renderer.admits(COMPILE, generation, 20, false));
        assert!(renderer.admits(TASK, generation, 20, false));
        assert_eq!(
            renderer.observe(event, generation, 10, true),
            generation + 1
        );
        assert!(!renderer.admits(COMPILE, generation, 20, false));
        assert!(!renderer.admits(TASK, generation, 20, false));
        assert!(renderer.admits(COMPILE, generation + 1, 20, false));
        assert!(renderer.admits(TASK, generation + 1, 20, false));
    }
}
