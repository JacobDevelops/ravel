//! Fixed-capacity mesh ownership from admission through completion publication.

use crate::mesh::{MAX_MESH_BYTES, MAX_MESH_STACK_SCRATCH_BYTES, MeshError, MeshResult, mesh};
use crate::{MAX_GENERATION, MAX_TOTAL_BYTES, SectionSnapshot};
use std::fmt;
use std::mem::size_of;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::Instant;

/// Maximum concurrently executing mesh calls.
pub const MAX_WORKERS: usize = 2;
/// Combined queued, running and completed job ceiling.
pub const MAX_OUTSTANDING_JOBS: usize = 8;
/// Conservative payload reservation held until a job is drained or discarded.
///
/// Includes snapshot, maximum mesh, bounded stack scratch and job metadata;
/// allocator bookkeeping and platform thread stacks are excluded.
pub const JOB_RESERVED_BYTES: usize = MAX_TOTAL_BYTES
    + MAX_MESH_BYTES
    + MAX_MESH_STACK_SCRATCH_BYTES
    + size_of::<Job>()
    + size_of::<Completion>();
/// Maximum admitted payload reservation, including completed outputs.
pub const MAX_RESERVED_BYTES: usize = MAX_OUTSTANDING_JOBS * JOB_RESERVED_BYTES;

/// Pool startup, generation validation or explicit shutdown failure.
#[derive(Debug)]
pub enum WorkerError {
    /// Generations must be positive Java longs.
    InvalidGeneration,
    /// The operating system rejected thread creation.
    Spawn(std::io::Error),
    /// At least one worker panicked; all workers were still joined and drained.
    ThreadPanicked,
}

impl fmt::Display for WorkerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidGeneration => formatter.write_str("invalid worker generation"),
            Self::Spawn(error) => write!(formatter, "worker startup failed: {error}"),
            Self::ThreadPanicked => formatter.write_str("mesh worker panicked"),
        }
    }
}

impl std::error::Error for WorkerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Spawn(error) => Some(error),
            Self::InvalidGeneration | Self::ThreadPanicked => None,
        }
    }
}

/// Admission rejection; the transferred snapshot is immediately dropped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubmitError {
    /// Shutdown is terminal; construct a new pool to restart.
    Stopped,
    /// Snapshot generation does not match the pool generation.
    Stale,
    /// Combined work count or byte reservation ceiling was reached.
    Full,
    /// Another queued, running or completed job already owns this key.
    DuplicateJobId,
}

impl fmt::Display for SubmitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Stopped => "mesh worker pool is stopped",
            Self::Stale => "snapshot generation is stale",
            Self::Full => "mesh worker reservation is full",
            Self::DuplicateJobId => "mesh job identity is already outstanding",
        })
    }
}

impl std::error::Error for SubmitError {}

/// Shared metadata and geometry-wire outcome; only `Complete` is drawable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Outcome {
    /// Every captured state is supported.
    Complete = 0,
    /// Explicitly unsupported captured content.
    Incomplete = 1,
    /// Meshing failed.
    Failed = 2,
}

/// Sole owned publication for one admitted key.
#[derive(Debug)]
pub struct Completion {
    /// Captured generation, checked again while polling under the pool lock.
    pub generation: u64,
    /// Caller-supplied identity, unique among outstanding jobs.
    pub job_id: u64,
    /// Owned mesh or explicit meshing failure.
    pub result: Result<MeshResult, MeshError>,
    /// Admission-to-execution delay, saturating nanoseconds.
    pub queue_wait_nanos: u64,
    /// Meshing duration, saturating nanoseconds.
    pub mesh_nanos: u64,
    /// Retained successful mesh payload including its object.
    pub output_bytes: usize,
    /// Successful mesh allocation count; failures report zero.
    pub allocations: usize,
}
impl Completion {
    /// Returns the same outcome encoding for metadata and geometry transfer.
    #[must_use]
    pub fn outcome(&self) -> Outcome {
        match &self.result {
            Ok(mesh) if mesh.is_complete() => Outcome::Complete,
            Ok(_) => Outcome::Incomplete,
            Err(_) => Outcome::Failed,
        }
    }
}


/// Bounded scalar counters; duration/output totals saturate rather than grow storage.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WorkerStats {
    /// Current accepted snapshot generation.
    pub generation: u64,
    /// Terminal shutdown state.
    pub stopped: bool,
    /// All reserved owners, including completed outputs.
    pub outstanding: usize,
    /// Owners awaiting a worker.
    pub queued: usize,
    /// Owners currently held by workers, including cancellation in progress.
    pub running: usize,
    /// Owners awaiting a caller poll.
    pub completed: usize,
    /// Current conservative payload reservation.
    pub reserved_bytes: usize,
    /// Lifetime admitted-owner high-water mark.
    pub max_outstanding: usize,
    /// Lifetime reservation high-water mark.
    pub max_reserved_bytes: usize,
    /// Saturating total queue delay for executed jobs, including discarded work.
    pub queue_wait_nanos: u64,
    /// Saturating total execution time, including discarded work.
    pub mesh_nanos: u64,
    /// Saturating successful output bytes produced, including discarded work.
    pub output_bytes: usize,
    /// Saturating successful allocation counts, including discarded work.
    pub allocations: usize,
}

struct Job {
    snapshot: SectionSnapshot,
    job_id: u64,
    admitted: Instant,
}

enum Slot {
    Vacant,
    Queued(Job),
    Running(u64),
    Completed(u64),
}

impl Slot {
    fn job_id(&self) -> Option<u64> {
        match self {
            Self::Vacant => None,
            Self::Queued(job) => Some(job.job_id),
            Self::Running(job_id) | Self::Completed(job_id) => Some(*job_id),
        }
    }
}

struct State {
    slots: [Slot; MAX_OUTSTANDING_JOBS],
    completions: [Option<Completion>; MAX_OUTSTANDING_JOBS],
    stats: WorkerStats,
}

struct Shared {
    state: Mutex<State>,
    wake: Condvar,
    generation: AtomicU64,
    cancelled: [AtomicBool; MAX_OUTSTANDING_JOBS],
}

impl Shared {
    fn new(generation: u64) -> Self {
        Self {
            state: Mutex::new(State {
                slots: std::array::from_fn(|_| Slot::Vacant),
                completions: std::array::from_fn(|_| None),
                stats: WorkerStats {
                    generation,
                    ..WorkerStats::default()
                },
            }),
            wake: Condvar::new(),
            generation: AtomicU64::new(generation),
            cancelled: std::array::from_fn(|_| AtomicBool::new(false)),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Two reusable workers with eight fixed ownership slots and no completion backlog.
/// Invalidation reuses workers; shutdown is terminal and restart requires `new`.
pub struct WorkerPool {
    shared: Arc<Shared>,
    threads: [Option<JoinHandle<()>>; MAX_WORKERS],
}

impl WorkerPool {
    /// Starts exactly `MAX_WORKERS` threads, cleaning up partial startup on error.
    ///
    /// # Errors
    ///
    /// Returns `InvalidGeneration` for a generation outside the positive Java-long
    /// range, or `Spawn` if an operating-system worker cannot be started.
    pub fn new(generation: u64) -> Result<Self, WorkerError> {
        validate_generation(generation)?;
        let mut pool = Self {
            shared: Arc::new(Shared::new(generation)),
            threads: std::array::from_fn(|_| None),
        };
        for index in 0..MAX_WORKERS {
            let shared = Arc::clone(&pool.shared);
            let handle = thread::Builder::new()
                .name(format!("ravel-mesh-{index}"))
                .spawn(move || worker(&shared))
                .map_err(WorkerError::Spawn)?;
            pool.threads[index] = Some(handle);
        }
        Ok(pool)
    }

    /// Transfers ownership only after checking generation, uniqueness and both ceilings.
    /// A cancelled running owner retains its key until its worker releases the reservation.
    ///
    /// # Errors
    ///
    /// Returns `Stopped`, `Stale`, `DuplicateJobId` or `Full` without admitting the
    /// snapshot when shutdown, generation, ownership or resource checks fail.
    pub fn try_submit(&self, snapshot: SectionSnapshot, job_id: u64) -> Result<(), SubmitError> {
        let mut state = self.shared.lock();
        if state.stats.stopped {
            return Err(SubmitError::Stopped);
        }
        if snapshot.generation() != state.stats.generation {
            return Err(SubmitError::Stale);
        }
        if state.slots.iter().any(|slot| slot.job_id() == Some(job_id)) {
            return Err(SubmitError::DuplicateJobId);
        }
        if state.stats.outstanding >= MAX_OUTSTANDING_JOBS
            || state.stats.reserved_bytes > MAX_RESERVED_BYTES - JOB_RESERVED_BYTES
        {
            return Err(SubmitError::Full);
        }
        let index = state
            .slots
            .iter()
            .position(|slot| matches!(slot, Slot::Vacant))
            .ok_or(SubmitError::Full)?;
        self.shared.cancelled[index].store(false, Ordering::Release);
        state.stats.outstanding += 1;
        state.stats.reserved_bytes += JOB_RESERVED_BYTES;
        state.stats.max_outstanding = state.stats.max_outstanding.max(state.stats.outstanding);
        state.stats.max_reserved_bytes = state
            .stats
            .max_reserved_bytes
            .max(state.stats.reserved_bytes);
        state.slots[index] = Slot::Queued(Job {
            snapshot,
            job_id,
            admitted: Instant::now(),
        });
        drop(state);
        self.shared.wake.notify_one();
        Ok(())
    }

    /// Removes at most one exact-current-generation completion under the invalidation lock.
    /// Once returned, the completion belongs to the caller, outside the pool reservation.
    #[must_use]
    pub fn poll(&self) -> Option<Completion> {
        let mut state = self.shared.lock();
        for index in 0..MAX_OUTSTANDING_JOBS {
            if let Some(completion) = state.completions[index].take() {
                state.slots[index] = Slot::Vacant;
                release(&mut state);
                if !state.stats.stopped && completion.generation == state.stats.generation {
                    return Some(completion);
                }
            }
        }
        None
    }

    /// Borrows current completions under the invalidation lock without releasing owners.
    /// The callback must not reenter the pool; failure and success both retain ownership.
    pub fn peek_ready<T>(
        &self,
        publish: impl FnOnce(&[Option<&Completion>; MAX_OUTSTANDING_JOBS]) -> T,
    ) -> T {
        let state = self.shared.lock();
        let ready = std::array::from_fn(|index| {
            state.completions[index].as_ref().filter(|completion| {
                !state.stats.stopped && completion.generation == state.stats.generation
            })
        });
        publish(&ready)
    }

    /// Borrows one exact generation/key while blocking invalidation and acknowledgement.
    /// Missing, stopped and stale owners are reported as `None`. Never consumes.
    pub fn with_completion<T>(
        &self,
        generation: u64,
        job_id: u64,
        publish: impl FnOnce(Option<&Completion>) -> T,
    ) -> T {
        self.peek_ready(|ready| {
            publish(
                ready.iter().flatten().copied().find(|completion| {
                    completion.generation == generation && completion.job_id == job_id
                }),
            )
        })
    }

    /// Releases only a completed exact-current-generation owner; queued/running keys remain.
    #[cfg(test)]
    pub fn acknowledge(&self, generation: u64, job_id: u64) -> bool {
        self.publish_acknowledgement(generation, job_id, Ok::<bool, std::convert::Infallible>)
            .unwrap_or(false)
    }

    /// Publishes before releasing an exact completed owner; failure preserves ownership.
    /// The callback must not reenter the pool.
    ///
    /// # Errors
    ///
    /// Propagates the callback's error without releasing the completed owner.
    pub fn publish_acknowledgement<T, E>(
        &self,
        generation: u64,
        job_id: u64,
        publish: impl FnOnce(bool) -> Result<T, E>,
    ) -> Result<T, E> {
        let mut state = self.shared.lock();
        if state.stats.stopped || state.stats.generation != generation {
            return publish(false);
        }
        let index = state.completions.iter().position(|completion| {
            completion.as_ref().is_some_and(|completion| {
                completion.generation == generation && completion.job_id == job_id
            })
        });
        let value = publish(index.is_some())?;
        if let Some(index) = index {
            state.completions[index] = None;
            state.slots[index] = Slot::Vacant;
            release(&mut state);
        }
        drop(state);
        Ok(value)
    }

    /// Publishes current ready owners transactionally while holding the invalidation lock.
    /// The callback runs exactly once, including when every entry is `None`.
    /// Failure preserves all owners and reservations; success releases ready owners.
    /// The callback must not reenter this pool because its mutex remains locked.
    ///
    /// # Errors
    ///
    /// Propagates the callback's error without releasing any ready owners.
    pub fn consume_ready<T, E>(
        &self,
        publish: impl FnOnce(&[Option<&Completion>; MAX_OUTSTANDING_JOBS]) -> Result<T, E>,
    ) -> Result<T, E> {
        let mut state = self.shared.lock();
        let ready = std::array::from_fn(|index| {
            state.completions[index].as_ref().filter(|completion| {
                !state.stats.stopped && completion.generation == state.stats.generation
            })
        });
        let value = publish(&ready)?;
        let consumed = ready.map(|completion| completion.is_some());
        for (index, consumed) in consumed.into_iter().enumerate() {
            if consumed {
                state.completions[index] = None;
                state.slots[index] = Slot::Vacant;
                release(&mut state);
            }
        }
        drop(state);
        Ok(value)
    }

    /// Atomically changes generation, discards queued/completed owners and cancels running work.
    /// Even invalidating to the same generation cancels all prior jobs; it never restarts shutdown.
    ///
    /// # Errors
    ///
    /// Returns `InvalidGeneration` without changing the pool when the generation
    /// is outside the positive Java-long range.
    pub fn invalidate(&self, generation: u64) -> Result<(), WorkerError> {
        validate_generation(generation)?;
        let mut state = self.shared.lock();
        state.stats.generation = generation;
        self.shared.generation.store(generation, Ordering::Release);
        discard(&self.shared, &mut state);
        drop(state);
        self.shared.wake.notify_all();
        Ok(())
    }

    /// Cancels, joins every thread and drains all reservations; safe to call repeatedly.
    ///
    /// # Errors
    ///
    /// Returns `ThreadPanicked` after joining all workers and draining reservations
    /// if any worker terminated by panicking.
    pub fn shutdown(&mut self) -> Result<(), WorkerError> {
        {
            let mut state = self.shared.lock();
            state.stats.stopped = true;
            discard(&self.shared, &mut state);
            drop(state);
            self.shared.wake.notify_all();
        }
        let mut panicked = false;
        for handle in &mut self.threads {
            if let Some(handle) = handle.take() {
                panicked |= handle.join().is_err();
            }
        }
        let mut state = self.shared.lock();
        state.slots = std::array::from_fn(|_| Slot::Vacant);
        state.completions = std::array::from_fn(|_| None);
        state.stats.outstanding = 0;
        state.stats.reserved_bytes = 0;
        drop(state);
        if panicked {
            Err(WorkerError::ThreadPanicked)
        } else {
            Ok(())
        }
    }

    /// Captures counters consistently; running invalidated jobs retain their reservations.
    #[must_use]
    pub fn stats(&self) -> WorkerStats {
        let state = self.shared.lock();
        let mut counters = state.stats;
        for slot in &state.slots {
            match slot {
                Slot::Vacant => {}
                Slot::Queued(_) => counters.queued += 1,
                Slot::Running(_) => counters.running += 1,
                Slot::Completed(_) => counters.completed += 1,
            }
        }
        drop(state);
        counters
    }
}

impl Drop for WorkerPool {
    fn drop(&mut self) {
        // Explicit shutdown exposes join failures; destruction must still drain every worker.
        drop(self.shutdown());
    }
}

fn validate_generation(generation: u64) -> Result<(), WorkerError> {
    if generation == 0 || generation > MAX_GENERATION {
        Err(WorkerError::InvalidGeneration)
    } else {
        Ok(())
    }
}

fn release(state: &mut State) {
    state.stats.outstanding -= 1;
    state.stats.reserved_bytes -= JOB_RESERVED_BYTES;
}

fn discard(shared: &Shared, state: &mut State) {
    for index in 0..MAX_OUTSTANDING_JOBS {
        shared.cancelled[index].store(true, Ordering::Release);
        if !matches!(state.slots[index], Slot::Running(_) | Slot::Vacant) {
            state.slots[index] = Slot::Vacant;
            state.completions[index] = None;
            release(state);
        }
    }
}

struct RunningOwner<'a> {
    shared: &'a Shared,
    index: usize,
    armed: bool,
}

impl<'a> RunningOwner<'a> {
    fn new(shared: &'a Shared, index: usize) -> Self {
        Self {
            shared,
            index,
            armed: true,
        }
    }
}

impl Drop for RunningOwner<'_> {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let mut state = self.shared.lock();
        state.stats.stopped = true;
        discard(self.shared, &mut state);
        if matches!(state.slots[self.index], Slot::Running(_)) {
            state.slots[self.index] = Slot::Vacant;
            state.completions[self.index] = None;
            release(&mut state);
        }
        drop(state);
        self.shared.wake.notify_all();
    }
}

fn worker(shared: &Shared) {
    while let Some((index, job)) = take_job(shared) {
        let mut owner = RunningOwner::new(shared, index);
        let completion = execute(shared, index, job);
        finish_job(shared, index, completion);
        owner.armed = false;
    }
}

fn take_job(shared: &Shared) -> Option<(usize, Job)> {
    let mut state = shared.lock();
    loop {
        if state.stats.stopped {
            return None;
        }
        let index = state
            .slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| {
                if let Slot::Queued(job) = slot {
                    Some((index, job.admitted))
                } else {
                    None
                }
            })
            .min_by_key(|&(_, admitted)| admitted)
            .map(|(index, _)| index);
        if let Some(index) = index {
            let slot = std::mem::replace(&mut state.slots[index], Slot::Vacant);
            if let Slot::Queued(job) = slot {
                state.slots[index] = Slot::Running(job.job_id);
                return Some((index, job));
            }
        }
        state = shared
            .wake
            .wait(state)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
    }
}

fn execute(shared: &Shared, index: usize, job: Job) -> Completion {
    let queue_wait_nanos = nanos(job.admitted.elapsed().as_nanos());
    let started = Instant::now();
    let result = mesh(&job.snapshot, job.job_id, || {
        shared.generation.load(Ordering::Acquire) != job.snapshot.generation()
            || shared.cancelled[index].load(Ordering::Acquire)
    });
    let mesh_nanos = nanos(started.elapsed().as_nanos());
    let output_bytes = result.as_ref().map_or(0, MeshResult::total_bytes);
    let allocations = result.as_ref().map_or(0, MeshResult::allocations);
    let generation = job.snapshot.generation();
    drop(job.snapshot);
    Completion {
        generation,
        job_id: job.job_id,
        result,
        queue_wait_nanos,
        mesh_nanos,
        output_bytes,
        allocations,
    }
}

fn finish_job(shared: &Shared, index: usize, completion: Completion) {
    let mut state = shared.lock();
    state.stats.queue_wait_nanos = state
        .stats
        .queue_wait_nanos
        .saturating_add(completion.queue_wait_nanos);
    state.stats.mesh_nanos = state.stats.mesh_nanos.saturating_add(completion.mesh_nanos);
    state.stats.output_bytes = state
        .stats
        .output_bytes
        .saturating_add(completion.output_bytes);
    state.stats.allocations = state
        .stats
        .allocations
        .saturating_add(completion.allocations);
    if state.stats.stopped
        || state.stats.generation != completion.generation
        || shared.cancelled[index].load(Ordering::Acquire)
    {
        state.slots[index] = Slot::Vacant;
        release(&mut state);
    } else {
        state.slots[index] = Slot::Completed(completion.job_id);
        state.completions[index] = Some(completion);
    }
}

fn nanos(value: u128) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests;
