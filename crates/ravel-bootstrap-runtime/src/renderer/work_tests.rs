use ravel_renderer::{CELL_COUNT, SectionSnapshot};

use super::{FRAME, Lifecycle, RELOAD, Renderer, work};

fn snapshot(generation: u64) -> SectionSnapshot {
    SectionSnapshot::capture(
        generation,
        &vec![0; CELL_COUNT],
        &vec![0; CELL_COUNT],
        &vec![0; CELL_COUNT],
    )
    .expect("bounded snapshot")
}

fn solid_snapshot(generation: u64) -> SectionSnapshot {
    let mut states = vec![0; CELL_COUNT];
    states[(18 + 1) * 18 + 1] = 16;
    let geometry: Vec<_> = states.iter().map(|state| u8::from(*state == 16)).collect();
    SectionSnapshot::capture(generation, &states, &[0xa3; CELL_COUNT], &geometry)
        .unwrap_or_else(|error| panic!("{error:?}"))
}

fn assert_solid_wire(bytes: &[u8], generation: i64, key: i64) {
    assert_eq!(bytes.len(), 48 + 24 * 5 + 36 * 4 + 12);
    assert_eq!(&bytes[..8], &[0x52, 0x56, 0x4d, 0x35, 1, 0, 0, 0]);
    assert_eq!(&bytes[8..16], &generation.to_le_bytes());
    assert_eq!(&bytes[16..24], &key.to_le_bytes());
    assert_eq!(&bytes[24..28], &0_u32.to_le_bytes());
    assert_eq!(&bytes[28..32], &24_u32.to_le_bytes());
    assert_eq!(&bytes[32..36], &36_u32.to_le_bytes());
    assert_eq!(&bytes[36..40], &1_u32.to_le_bytes());
    assert_eq!(&bytes[40..48], &((1_u64 << 36) - 1).to_le_bytes());
    let mesh = ravel_renderer::mesh::mesh(
        &solid_snapshot(generation.unsigned_abs()),
        key.unsigned_abs(),
        || false,
    )
    .unwrap_or_else(|error| panic!("{error:?}"));
    for (offset, vertex) in mesh.layers()[0].vertices().iter().enumerate() {
        let offset = 48 + offset * 5;
        assert_eq!(&bytes[offset..offset + 3], &vertex.position());
        assert_eq!(bytes[offset + 3], vertex.face().code());
        assert_eq!(bytes[offset + 4], vertex.packed_light());
    }
    for (offset, index) in mesh.layers()[0].indices().iter().enumerate() {
        let offset = 48 + 24 * 5 + offset * 4;
        assert_eq!(&bytes[offset..offset + 4], &index.to_le_bytes());
    }
    assert_eq!(&bytes[312..324], &[16, 0, 0, 0, 0, 0, 0, 0, 36, 0, 0, 0]);
}

fn wait_for_retained_completion(renderer: &Lifecycle) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let ready = work::exchange(renderer, 4, 1, 0).unwrap_or_default();
        if ready.get(2) == Some(&1) {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "completion deadline");
        std::thread::yield_now();
    }
}

#[test]
fn accepted_generation_still_refuses_transfer_when_draining_or_failed() {
    for (draining, worker_failure) in [(true, false), (false, true)] {
        let mut state = Renderer::new();
        assert!(state.submit(solid_snapshot(1), 71).is_ok_and(|value| value));
        let renderer = Lifecycle::new(state);
        wait_for_retained_completion(&renderer);
        {
            let mut locked = renderer
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            locked.draining = draining;
            locked.worker_failure = worker_failure;
        }
        for (operation, key) in [(4, 0), (5, 71)] {
            assert_eq!(
                work::exchange(&renderer, operation, 1, key).unwrap_or_default(),
                Vec::<i64>::new()
            );
        }
        let bytes = work::publish_mesh(&renderer, 1, 71, |bytes| Ok(bytes.to_vec()))
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(bytes, Vec::<u8>::new());
        let mut locked = renderer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        locked.draining = false;
        locked.worker_failure = false;
        drop(locked);
        let bytes = work::publish_mesh(&renderer, 1, 71, |bytes| Ok(bytes.to_vec()))
            .unwrap_or_else(|error| panic!("{error}"));
        assert_solid_wire(&bytes, 1, 71);
        assert_eq!(work::exchange(&renderer, 5, 1, 71).unwrap_or_default(), [1]);
        work::stop(&renderer, 1);
    }
}

#[test]
fn retained_transfer_retries_and_refuses_stale_or_closed_lifecycle() {
    let renderer = Lifecycle::new(Renderer::new());
    {
        let mut locked = renderer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert!(
            locked
                .submit(solid_snapshot(1), 71)
                .is_ok_and(|accepted| accepted)
        );
        drop(locked);
    }
    wait_for_retained_completion(&renderer);
    let failed: Result<(), work::BoundaryError> = work::publish_mesh(&renderer, 1, 71, |_| {
        Err(work::BoundaryError::Worker(
            ravel_renderer::workers::WorkerError::InvalidGeneration,
        ))
    });
    assert!(failed.is_err());
    for _ in 0..2 {
        let bytes = work::publish_mesh(&renderer, 1, 71, |bytes| Ok(bytes.to_vec()))
            .unwrap_or_else(|error| panic!("{error}"));
        assert_solid_wire(&bytes, 1, 71);
    }
    for (generation, key) in [(2, 71), (1, 72), (1, 0), (1, -1)] {
        let bytes = work::publish_mesh(&renderer, generation, key, |bytes| Ok(bytes.to_vec()))
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(bytes, Vec::<u8>::new());
    }
    assert_eq!(
        work::exchange(&renderer, 5, 2, 71).unwrap_or_default(),
        Vec::<i64>::new()
    );
    assert_eq!(work::exchange(&renderer, 5, 1, 72).unwrap_or_default(), [0]);
    assert_eq!(work::exchange(&renderer, 5, 1, 71).unwrap_or_default(), [1]);
    assert_eq!(work::exchange(&renderer, 5, 1, 71).unwrap_or_default(), [0]);
    assert_eq!(
        work::exchange(&renderer, 4, 1, 0).unwrap_or_default(),
        [1, 1, 0, 11]
    );
    work::stop(&renderer, 1);
    assert_eq!(
        work::exchange(&renderer, 5, 1, 71).unwrap_or_default(),
        Vec::<i64>::new()
    );
    let bytes = work::publish_mesh(&renderer, 1, 71, |bytes| Ok(bytes.to_vec()))
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(bytes, Vec::<u8>::new());
}

#[test]
fn work_protocol_rejects_stale_unknown_and_nonzero_reserved_keys() {
    let renderer = Lifecycle::new(Renderer::new());
    for (operation, generation, key) in [(0, 0, 0), (1, 2, 0), (6, 1, 0), (0, 1, 1)] {
        assert_eq!(
            work::exchange(&renderer, operation, generation, key).expect("protocol"),
            [0_i64; 0]
        );
    }
    assert_eq!(
        work::exchange(&renderer, 0, 1, 0).expect("poll"),
        [1, 1, 0, 11]
    );
    let stats = work::exchange(&renderer, 1, 1, 0).expect("stats");
    assert_eq!(stats.len(), 18);
    assert_eq!(&stats[..8], [1, 1, 1, 0, 0, 0, 0, 0]);
    assert_eq!(stats[15], 8);
    assert!(stats[16] > 0);
    assert_eq!(stats[17], 0);
}

#[test]
fn retired_statistics_prove_actual_previous_generation_drain() {
    let renderer = Lifecycle::new(Renderer::new());
    assert_eq!(
        work::exchange(&renderer, 3, 1, 0).expect("no retired pool"),
        [0_i64; 0]
    );
    {
        let mut locked = renderer.lock().expect("renderer");
        locked.observe(FRAME, 1, 10, true);
        assert!(locked.submit(snapshot(1), 49).expect("submit"));
        drop(locked);
    }
    assert_eq!(work::observe(&renderer, RELOAD, 1, 10, true), 2);
    let current = work::exchange(&renderer, 1, 2, 0).expect("current stats");
    assert_eq!(&current[..10], [1, 2, 1, 0, 0, 0, 0, 0, 0, 0]);
    let retired = work::exchange(&renderer, 3, 2, 0).expect("retired stats");
    assert_eq!(retired.len(), 18);
    assert_eq!(&retired[..8], [1, 1, 1, 0, 0, 0, 0, 0]);
    assert_eq!(retired[8], 1);
    assert!(retired[9] > 0);
    assert!(retired[8] <= retired[15]);
    assert!(retired[9] <= retired[16]);
    assert_eq!(retired[14], 0);
    assert_eq!(
        work::exchange(&renderer, 3, 1, 0).expect("stale auth"),
        [0_i64; 0]
    );
    assert_eq!(
        work::exchange(&renderer, 3, 2, 1).expect("reserved key"),
        [0_i64; 0]
    );
}

#[test]
fn retired_statistics_during_join_cannot_claim_final_drain() {
    let renderer = Lifecycle::new(Renderer::new());
    let mut pool = {
        let mut locked = renderer.lock().expect("renderer");
        assert!(locked.submit(snapshot(1), 50).expect("submit"));
        let pool = locked.workers.take().expect("workers");
        locked.retired_stats = Some(pool.stats());
        locked.draining = true;
        locked.generation = 2;
        pool
    };
    let joining = work::exchange(&renderer, 3, 2, 0).expect("joining stats");
    assert_eq!(&joining[..4], [1, 1, 0, 1]);
    assert!(joining[7] > 0);
    assert_eq!(joining[14], 1);
    pool.shutdown().expect("join");
    {
        let mut locked = renderer.lock().expect("renderer");
        locked.retired_stats = Some(pool.stats());
        locked.draining = false;
    }
    let drained = work::exchange(&renderer, 3, 2, 0).expect("final stats");
    assert_eq!(&drained[..8], [1, 1, 1, 0, 0, 0, 0, 0]);
    assert_eq!(drained[8], joining[8]);
    assert_eq!(drained[9], joining[9]);
    assert_eq!(drained[14], 0);
}

#[test]
fn stale_submission_never_creates_workers_and_shutdown_drains_reservations() {
    let mut renderer = Renderer::new();
    assert!(!renderer.submit(snapshot(2), 1).expect("stale"));
    assert!(!renderer.submit(snapshot(1), 0).expect("invalid key"));
    assert!(renderer.workers.is_none());
    assert!(renderer.submit(snapshot(1), 1).expect("submit"));
    assert!(!renderer.submit(snapshot(1), 1).expect("duplicate"));
    let renderer = Lifecycle::new(renderer);
    let stats = work::exchange(&renderer, 2, 1, 0).expect("shutdown");
    assert_eq!(stats.len(), 18);
    assert_eq!(&stats[2..8], [1, 0, 0, 0, 0, 0]);
    assert!(stats[8] >= 1);
    assert!(stats[9] > 0);
    assert!(stats[8] <= stats[15]);
    assert!(stats[9] <= stats[16]);
    assert_eq!(stats[14], 0);
    assert!(
        !renderer
            .lock()
            .expect("renderer")
            .submit(snapshot(1), 2)
            .expect("closed")
    );
    assert_eq!(work::observe(&renderer, RELOAD, 1, 10, true), 2);
    assert!(
        renderer
            .lock()
            .expect("renderer")
            .submit(snapshot(2), 2)
            .expect("restart")
    );
    work::stop(&renderer, 2);
}

#[test]
fn lifecycle_retires_work_and_current_generation_can_restart() {
    let renderer = Lifecycle::new(Renderer::new());
    {
        let mut locked = renderer.lock().expect("renderer");
        locked.observe(FRAME, 1, 10, true);
        assert!(locked.submit(snapshot(1), 1).expect("submit"));
        drop(locked);
    }
    assert_eq!(work::observe(&renderer, RELOAD, 1, 10, true), 2);
    assert_eq!(
        work::exchange(&renderer, 0, 1, 0).expect("stale poll"),
        [0_i64; 0]
    );
    assert_eq!(
        work::exchange(&renderer, 0, 2, 0).expect("poll"),
        [1, 2, 0, 11]
    );
    let retired = work::exchange(&renderer, 1, 2, 0).expect("retired stats");
    assert_eq!(&retired[2..8], [1, 0, 0, 0, 0, 0]);
    assert!(
        renderer
            .lock()
            .expect("renderer")
            .submit(snapshot(2), 2)
            .expect("restart")
    );
    let stats = work::exchange(&renderer, 2, 2, 0).expect("shutdown");
    assert_eq!(&stats[2..8], [1, 0, 0, 0, 0, 0]);
}

#[test]
fn completion_record_has_bounded_fixed_width_and_preserves_identity() {
    let mut pool = ravel_renderer::workers::WorkerPool::new(1).expect("workers");
    pool.try_submit(snapshot(1), 9).expect("submit");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let completion = loop {
        if let Some(completion) = pool.poll() {
            break completion;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "mesh completion deadline"
        );
        std::thread::yield_now();
    };
    let mut values = Vec::new();
    work::append_completion(&mut values, &completion);
    assert_eq!(values.len(), 11);
    assert_eq!(&values[..3], [1, 9, 1]);
    assert_eq!(values[7], 0);
    assert_eq!(values[8], 0);
    assert_eq!(values[9], 0);
    pool.shutdown().expect("shutdown");
}

#[test]
fn poll_batches_and_reservations_obey_the_native_limits() {
    let mut renderer = Renderer::new();
    for key in 1..=8 {
        assert!(renderer.submit(snapshot(1), key).expect("submit"));
    }
    assert!(!renderer.submit(snapshot(1), 9).expect("full"));
    let renderer = Lifecycle::new(renderer);
    let stats = work::exchange(&renderer, 1, 1, 0).expect("stats");
    assert_eq!(stats[3], 8);
    assert_eq!(stats[8], stats[15]);
    assert_eq!(stats[9], stats[16]);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut drained = 0;
    while drained < 8 {
        let values = work::exchange(&renderer, 0, 1, 0).expect("poll");
        assert_eq!(&values[..2], [1, 1]);
        assert_eq!(values[3], 11);
        assert!(values[2] <= 8);
        assert_eq!(
            values.len(),
            4 + usize::try_from(values[2]).expect("count") * 11
        );
        assert!(values.len() <= 92);
        drained += values[2];
        assert!(std::time::Instant::now() < deadline, "batch drain deadline");
        std::thread::yield_now();
    }
    let stats = work::exchange(&renderer, 2, 1, 0).expect("shutdown");
    assert_eq!(&stats[2..8], [1, 0, 0, 0, 0, 0]);
}

#[test]
fn unsupported_and_error_records_are_explicit_without_geometry_publication() {
    let snapshot = SectionSnapshot::capture(
        1,
        &vec![35; CELL_COUNT],
        &vec![0; CELL_COUNT],
        &vec![0; CELL_COUNT],
    )
    .expect("unsupported snapshot");
    let mesh = ravel_renderer::mesh::mesh(&snapshot, 12, || false).expect("mesh");
    assert_eq!(mesh.unsupported_keys(), [35]);
    let completion = ravel_renderer::workers::Completion {
        generation: 1,
        key: 12,
        queue_wait_nanos: 7,
        mesh_nanos: 11,
        output_bytes: mesh.total_bytes(),
        allocations: mesh.allocations(),
        result: Ok(mesh),
    };
    let mut values = Vec::with_capacity(11);
    work::append_completion(&mut values, &completion);
    assert_eq!(values.len(), 11);
    assert_eq!(&values[..5], [1, 12, 2, 7, 11]);
    assert_eq!(&values[7..10], [0, 0, 1]);
    values.clear();
    let completion = ravel_renderer::workers::Completion {
        generation: 1,
        key: 13,
        queue_wait_nanos: 0,
        mesh_nanos: 0,
        output_bytes: 0,
        allocations: 0,
        result: Err(ravel_renderer::mesh::MeshError::Cancelled),
    };
    work::append_completion(&mut values, &completion);
    assert_eq!(values, [1, 13, 3, 0, 0, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn failed_completion_publication_preserves_native_ownership_for_retry() {
    let mut state = Renderer::new();
    assert!(state.submit(snapshot(1), 44).expect("submit"));
    let renderer = Lifecycle::new(state);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let ready = renderer
            .lock()
            .expect("renderer")
            .workers
            .as_ref()
            .expect("workers")
            .stats()
            .completed;
        if ready == 1 {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "completion deadline");
        std::thread::yield_now();
    }
    let failed = work::publish(&renderer, 0, 1, 0, |values| {
        assert_eq!(values[2], 1);
        assert_eq!(values[5], 44);
        Err::<(), _>("JNI materialization")
    });
    assert_eq!(failed, Err("JNI materialization"));
    assert_eq!(work::exchange(&renderer, 1, 1, 0).expect("stats")[3], 1);
    let retried = work::exchange(&renderer, 0, 1, 0).expect("retry");
    assert_eq!(&retried[..6], [1, 1, 1, 11, 1, 44]);
    assert_eq!(work::exchange(&renderer, 1, 1, 0).expect("stats")[3], 0);
    work::stop(&renderer, 1);
}

#[test]
fn failed_receipt_publication_drains_and_closes_unacknowledged_work() {
    let mut state = Renderer::new();
    assert!(state.submit(snapshot(1), 45).expect("submit"));
    let renderer = Lifecycle::new(state);
    let failed =
        super::snapshot::publish(&renderer, 1, [1, 0, 0, 5832, 34992, 80536, 1, 45], |_| {
            Err::<(), _>("JNI receipt write")
        });
    assert_eq!(failed, Err("JNI receipt write"));
    let counters = work::exchange(&renderer, 1, 1, 0).expect("stats");
    assert_eq!(&counters[2..8], [1, 0, 0, 0, 0, 0]);
    assert!(
        !renderer
            .lock()
            .expect("renderer")
            .submit(snapshot(1), 46)
            .expect("closed")
    );
}

#[test]
fn stale_nonadmitted_receipt_is_empty_and_worker_failures_are_explicit() {
    let renderer = Lifecycle::new(Renderer::new());
    renderer.lock().expect("renderer").worker_failure = true;
    assert_eq!(work::observe(&renderer, RELOAD, 1, 10, true), 2);
    let published = super::snapshot::publish(&renderer, 1, [1, 0, 0, 0, 0, 0, 0, 45], |values| {
        assert_eq!(values, [0_i64; 0]);
        Ok::<_, ()>(())
    });
    assert_eq!(published, Ok(()));
    let stats = work::exchange(&renderer, 2, 2, 0).expect("drain despite failure");
    assert_eq!(stats[17], 1);
    assert_eq!(&stats[2..8], [1, 0, 0, 0, 0, 0]);
}

#[test]
fn admitted_receipt_survives_close_and_reload_without_publishing_stale_results() {
    for reload in [false, true] {
        let mut state = Renderer::new();
        assert!(state.submit(snapshot(1), 45).expect("admitted"));
        let renderer = Lifecycle::new(state);
        if reload {
            assert_eq!(work::observe(&renderer, RELOAD, 1, 10, true), 2);
        } else {
            work::stop(&renderer, 1);
        }
        let receipt = [1, 0, 0, 5832, 34992, 80536, 1, 45];
        assert_eq!(
            super::snapshot::publish(&renderer, 1, receipt, |values| {
                assert_eq!(values, receipt);
                Ok::<_, ()>(())
            }),
            Ok(())
        );
        let polled = work::exchange(&renderer, 0, 1, 0).expect("poll");
        if reload {
            assert!(polled.is_empty(), "retired generation must be rejected");
        } else {
            assert_eq!(polled, [1, 1, 0, 11], "closed pool has no results");
        }
        let generation = if reload { 2 } else { 1 };
        let retired = work::exchange(&renderer, 3, generation, 0).expect("retired");
        assert_eq!(retired[1], 1);
        assert_eq!(&retired[2..8], [1, 0, 0, 0, 0, 0]);
    }
}

#[test]
fn failed_stale_admitted_receipt_does_not_stop_the_replacement_pool() {
    let mut state = Renderer::new();
    assert!(state.submit(snapshot(1), 45).expect("admitted"));
    let renderer = Lifecycle::new(state);
    assert_eq!(work::observe(&renderer, RELOAD, 1, 10, true), 2);
    assert!(
        renderer
            .lock()
            .expect("renderer")
            .submit(snapshot(2), 46)
            .expect("replacement")
    );
    let receipt = [1, 0, 0, 5832, 34992, 80536, 1, 45];
    let failed = super::snapshot::publish(&renderer, 1, receipt, |values| {
        assert_eq!(values, receipt);
        Err::<(), _>("JNI receipt write")
    });
    assert_eq!(failed, Err("JNI receipt write"));
    assert!(renderer.lock().expect("renderer").can_capture(2));
    assert_eq!(work::exchange(&renderer, 1, 2, 0).expect("stats")[3], 1);
    assert_eq!(
        work::exchange(&renderer, 0, 1, 0).expect("stale poll"),
        Vec::<i64>::new()
    );
    work::stop(&renderer, 2);
}

#[test]
fn lifecycle_advance_waits_for_an_in_progress_shutdown_join() {
    let renderer = std::sync::Arc::new(Lifecycle::new(Renderer::new()));
    let mut pool = {
        let mut locked = renderer.lock().expect("renderer");
        locked.observe(FRAME, 1, 10, true);
        assert!(locked.submit(snapshot(1), 47).expect("submit"));
        locked.closed = true;
        locked.draining = true;
        locked.workers.take().expect("retiring workers")
    };
    let (entered, started) = std::sync::mpsc::channel();
    let (sent, received) = std::sync::mpsc::channel();
    let other = std::sync::Arc::clone(&renderer);
    let observer = std::thread::spawn(move || {
        entered.send(()).expect("entered");
        sent.send(work::observe(&other, RELOAD, 1, 10, true))
            .expect("observed");
    });
    started.recv().expect("started");
    assert!(
        received
            .recv_timeout(std::time::Duration::from_millis(20))
            .is_err()
    );
    pool.shutdown().expect("join");
    {
        let mut locked = renderer.lock().expect("renderer");
        locked.retired_stats = Some(pool.stats());
        locked.draining = false;
        drop(locked);
        renderer.drained.notify_all();
    }
    assert_eq!(
        received
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("joined before advance"),
        2
    );
    observer.join().expect("observer");
    let stats = work::exchange(&renderer, 1, 2, 0).expect("retired stats");
    assert_eq!(&stats[2..8], [1, 0, 0, 0, 0, 0]);
    assert!(
        renderer
            .lock()
            .expect("renderer")
            .submit(snapshot(2), 48)
            .expect("restart")
    );
    work::stop(&renderer, 2);
}

#[test]
fn failed_nonadmitted_receipt_does_not_close_other_work() {
    let renderer = Lifecycle::new(Renderer::new());
    let failed = super::snapshot::publish(&renderer, 1, [-1, 5, 35, 0, 0, 0, 0, 48], |_| {
        Err::<(), _>("diagnostic write failure")
    });
    assert_eq!(failed, Err("diagnostic write failure"));
    assert!(renderer.lock().expect("renderer").can_capture(1));
    assert!(
        renderer
            .lock()
            .expect("renderer")
            .submit(snapshot(1), 49)
            .expect("still open")
    );
    work::stop(&renderer, 1);
}
