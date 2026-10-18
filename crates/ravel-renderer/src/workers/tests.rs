use super::*;
use crate::CELL_COUNT;
use std::sync::Barrier;

fn snapshot(generation: u64) -> SectionSnapshot {
    let states = vec![0; CELL_COUNT];
    SectionSnapshot::capture(generation, &states, &[0; CELL_COUNT], &[0; CELL_COUNT])
        .expect("valid air snapshot")
}

fn parked_pool() -> WorkerPool {
    WorkerPool {
        shared: Arc::new(Shared::new(1)),
        threads: std::array::from_fn(|_| None),
    }
}

#[test]
fn queued_and_completed_jobs_share_count_and_byte_ceiling() {
    let pool = parked_pool();
    for key in 0..MAX_OUTSTANDING_JOBS {
        pool.try_submit(snapshot(1), u64::try_from(key).expect("key"))
            .expect("within ceiling");
    }
    assert_eq!(pool.try_submit(snapshot(1), 99), Err(SubmitError::Full));
    let (slot, job) = take_job(&pool.shared).expect("queued job");
    finish_job(&pool.shared, slot, execute(&pool.shared, slot, job));
    let stats = pool.stats();
    assert_eq!(stats.queued, MAX_OUTSTANDING_JOBS - 1);
    assert_eq!(stats.completed, 1);
    assert_eq!(stats.outstanding, MAX_OUTSTANDING_JOBS);
    assert_eq!(stats.reserved_bytes, MAX_RESERVED_BYTES);
    assert_eq!(pool.try_submit(snapshot(1), 99), Err(SubmitError::Full));
    assert!(pool.poll().is_some());
    pool.try_submit(snapshot(1), 99)
        .expect("released reservation");
    assert_eq!(pool.stats().max_outstanding, MAX_OUTSTANDING_JOBS);
    assert_eq!(pool.stats().max_reserved_bytes, MAX_RESERVED_BYTES);
    assert_eq!(pool.stats().reserved_bytes, MAX_RESERVED_BYTES);
}

#[test]
fn stale_and_duplicate_submissions_never_enter_the_pool() {
    let pool = parked_pool();
    assert_eq!(pool.try_submit(snapshot(2), 1), Err(SubmitError::Stale));
    pool.try_submit(snapshot(1), 1).expect("first owner");
    assert_eq!(
        pool.try_submit(snapshot(1), 1),
        Err(SubmitError::DuplicateKey)
    );
    let (slot, job) = take_job(&pool.shared).expect("job");
    finish_job(&pool.shared, slot, execute(&pool.shared, slot, job));
    assert_eq!(
        pool.try_submit(snapshot(1), 1),
        Err(SubmitError::DuplicateKey)
    );
    assert_eq!(pool.poll().expect("single completion").key, 1);
    assert!(pool.poll().is_none());
    pool.try_submit(snapshot(1), 1)
        .expect("key can be reused after drain");
}

#[test]
fn invalidation_between_mesh_and_publication_cancels_running_owner() {
    let pool = parked_pool();
    pool.try_submit(snapshot(1), 7).expect("job");
    let shared = Arc::clone(&pool.shared);
    let barrier = Arc::new(Barrier::new(2));
    let worker_barrier = Arc::clone(&barrier);
    let worker = thread::spawn(move || {
        let (slot, job) = take_job(&shared).expect("job");
        let completion = execute(&shared, slot, job);
        worker_barrier.wait();
        worker_barrier.wait();
        assert!(shared.cancelled[slot].load(Ordering::Acquire));
        finish_job(&shared, slot, completion);
    });
    barrier.wait();
    pool.invalidate(2).expect("new generation");
    assert_eq!(pool.stats().outstanding, 1);
    assert_eq!(pool.stats().reserved_bytes, JOB_RESERVED_BYTES);
    barrier.wait();
    worker.join().expect("publication worker");
    assert!(pool.poll().is_none());
    assert_eq!(pool.stats().reserved_bytes, 0);
    pool.try_submit(snapshot(2), 7)
        .expect("new generation owner");
}

#[test]
fn same_generation_invalidation_clears_completions_and_queued_jobs() {
    let pool = parked_pool();
    pool.try_submit(snapshot(1), 1).expect("first");
    pool.try_submit(snapshot(1), 2).expect("second");
    let (slot, job) = take_job(&pool.shared).expect("job");
    finish_job(&pool.shared, slot, execute(&pool.shared, slot, job));
    pool.invalidate(1)
        .expect("invalidate even unchanged generation");
    assert!(pool.poll().is_none());
    assert_eq!(pool.stats().outstanding, 0);
    assert_eq!(pool.stats().reserved_bytes, 0);
    pool.try_submit(snapshot(1), 1).expect("pool reusable");
}

#[test]
fn shutdown_joins_and_drains_every_reservation_and_is_terminal() {
    let mut pool = WorkerPool::new(1).expect("worker startup");
    for key in 0..MAX_OUTSTANDING_JOBS {
        pool.try_submit(snapshot(1), u64::try_from(key).expect("key"))
            .expect("bounded admission");
    }
    pool.shutdown().expect("joined workers");
    let stats = pool.stats();
    assert!(stats.stopped);
    assert_eq!(stats.outstanding, 0);
    assert_eq!(stats.reserved_bytes, 0);
    assert!(pool.poll().is_none());
    assert_eq!(pool.try_submit(snapshot(1), 0), Err(SubmitError::Stopped));
    assert!(pool.invalidate(2).is_ok());
    assert_eq!(pool.try_submit(snapshot(2), 0), Err(SubmitError::Stopped));
    pool.shutdown().expect("idempotent shutdown");
    WorkerPool::new(2)
        .expect("explicit restart")
        .shutdown()
        .expect("restart cleanup");
}

#[test]
fn cancelled_running_mesh_releases_its_reservation_without_publication() {
    let pool = parked_pool();
    pool.try_submit(snapshot(1), 5).expect("admission");
    let (slot, job) = take_job(&pool.shared).expect("running job");
    pool.invalidate(2).expect("invalidate running mesh");
    let completion = execute(&pool.shared, slot, job);
    assert!(matches!(completion.result, Err(MeshError::Cancelled)));
    finish_job(&pool.shared, slot, completion);
    assert!(pool.poll().is_none());
    assert_eq!(pool.stats().reserved_bytes, 0);
    pool.try_submit(snapshot(2), 5).expect("new owner");
    let (slot, job) = take_job(&pool.shared).expect("fresh job");
    finish_job(&pool.shared, slot, execute(&pool.shared, slot, job));
    let completion = pool.poll().expect("current generation");
    assert_eq!(completion.generation, 2);
    assert_eq!(completion.result.expect("air mesh").generation(), 2);
    assert_eq!(pool.stats().outstanding, 0);
}

#[test]
fn invalid_generation_does_not_change_pool_generation() {
    assert!(matches!(
        WorkerPool::new(0),
        Err(WorkerError::InvalidGeneration)
    ));
    let pool = parked_pool();
    assert!(matches!(
        pool.invalidate(0),
        Err(WorkerError::InvalidGeneration)
    ));
    assert_eq!(pool.stats().generation, 1);
}

fn publication_after_invalidations(generations: &[u64]) {
    let pool = parked_pool();
    pool.try_submit(snapshot(1), 17).expect("original owner");
    let shared = Arc::clone(&pool.shared);
    let barrier = Arc::new(Barrier::new(2));
    let worker_barrier = Arc::clone(&barrier);
    let handle = thread::spawn(move || {
        let (slot, job) = take_job(&shared).expect("running original owner");
        let completion = execute(&shared, slot, job);
        worker_barrier.wait();
        worker_barrier.wait();
        finish_job(&shared, slot, completion);
    });
    barrier.wait();
    for &generation in generations {
        pool.invalidate(generation).expect("generation transition");
    }
    assert_eq!(
        pool.try_submit(snapshot(1), 17),
        Err(SubmitError::DuplicateKey)
    );
    barrier.wait();
    handle.join().expect("late publication");
    assert!(pool.poll().is_none());
    assert_eq!(pool.stats().reserved_bytes, 0);
}

#[test]
fn same_generation_running_invalidation_rejects_late_publication() {
    publication_after_invalidations(&[1]);
}

#[test]
fn generation_aba_rejects_late_publication() {
    publication_after_invalidations(&[2, 1]);
}

#[test]
fn real_worker_wakes_and_publishes_owned_completion() {
    let mut pool = WorkerPool::new(1).expect("worker threads");
    pool.try_submit(snapshot(1), 41)
        .expect("admission wakes worker");
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    let completion = loop {
        if let Some(completion) = pool.poll() {
            break completion;
        }
        assert!(Instant::now() < deadline, "worker failed to publish");
        thread::yield_now();
    };
    assert_eq!(completion.key, 41);
    assert_eq!(completion.result.expect("mesh").generation(), 1);
    assert_eq!(pool.stats().reserved_bytes, 0);
    pool.shutdown().expect("worker cleanup");
}

#[test]
fn panicking_owner_stops_pool_and_releases_every_reservation() {
    let mut pool = parked_pool();
    for key in 1..=3 {
        pool.try_submit(snapshot(1), key).expect("admitted owner");
    }
    let (failed_slot, failed_job) = take_job(&pool.shared).expect("failed owner");
    let (other_slot, other_job) = take_job(&pool.shared).expect("other running owner");
    let shared = Arc::clone(&pool.shared);
    pool.threads[0] = Some(thread::spawn(move || {
        let _guard = RunningOwner::new(&shared, failed_slot);
        drop(failed_job);
        panic!("injected mesh worker failure");
    }));
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    while !pool.stats().stopped {
        assert!(Instant::now() < deadline, "panic guard failed to stop pool");
        thread::yield_now();
    }
    assert_eq!(pool.stats().outstanding, 1);
    assert_eq!(pool.try_submit(snapshot(1), 99), Err(SubmitError::Stopped));
    let completion = execute(&pool.shared, other_slot, other_job);
    finish_job(&pool.shared, other_slot, completion);
    assert_eq!(pool.stats().outstanding, 0);
    assert_eq!(pool.stats().reserved_bytes, 0);
    assert!(pool.poll().is_none());
    assert!(matches!(pool.shutdown(), Err(WorkerError::ThreadPanicked)));
    assert_eq!(pool.stats().reserved_bytes, 0);
}

#[test]
fn transactional_completion_failure_retains_owners_until_success() {
    let pool = parked_pool();
    for key in 1..=3 {
        pool.try_submit(snapshot(1), key).expect("admitted owner");
        let (slot, job) = take_job(&pool.shared).expect("queued owner");
        finish_job(&pool.shared, slot, execute(&pool.shared, slot, job));
    }
    let reserved = pool.stats().reserved_bytes;
    let failure = pool.consume_ready(|ready| {
        assert_eq!(ready.iter().flatten().count(), 3);
        Err::<(), _>("publication allocation failed")
    });
    assert_eq!(failure, Err("publication allocation failed"));
    assert_eq!(pool.stats().completed, 3);
    assert_eq!(pool.stats().reserved_bytes, reserved);
    let keys = pool
        .consume_ready(|ready| {
            let keys: Vec<_> = ready
                .iter()
                .flatten()
                .map(|completion| completion.key)
                .collect();
            Ok::<_, ()>(keys)
        })
        .expect("successful publication");
    assert_eq!(keys, [1, 2, 3]);
    assert_eq!(pool.stats().completed, 0);
    assert_eq!(pool.stats().reserved_bytes, 0);
    assert!(pool.poll().is_none());
}

#[test]
fn transactional_publication_invokes_empty_callback_once_after_invalidation() {
    let pool = parked_pool();
    pool.try_submit(snapshot(1), 3).expect("owner");
    let (slot, job) = take_job(&pool.shared).expect("queued owner");
    finish_job(&pool.shared, slot, execute(&pool.shared, slot, job));
    pool.invalidate(2).expect("invalidate completion");
    let mut calls = 0;
    let published = pool
        .consume_ready(|ready| {
            calls += 1;
            assert!(ready.iter().all(Option::is_none));
            Ok::<_, ()>(42)
        })
        .expect("empty publication");
    assert_eq!(published, 42);
    assert_eq!(calls, 1);
    assert_eq!(pool.stats().reserved_bytes, 0);
}
