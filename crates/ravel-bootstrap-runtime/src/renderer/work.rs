//! Fixed-width metadata only; owned geometry is consumed without GL upload.

use std::sync::PoisonError;

use jni::sys::{jint, jlong};
use ravel_renderer::SectionSnapshot;
use ravel_renderer::workers::{
    Completion, MAX_OUTSTANDING_JOBS, MAX_RESERVED_BYTES, WorkerError, WorkerPool, WorkerStats,
};

use super::Renderer;

#[derive(Debug)]
pub(super) enum BoundaryError {
    Jni(jni::errors::Error),
    Worker(WorkerError),
}

impl std::fmt::Display for BoundaryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Jni(error) => std::fmt::Display::fmt(error, formatter),
            Self::Worker(error) => std::fmt::Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for BoundaryError {}

impl From<jni::errors::Error> for BoundaryError {
    fn from(error: jni::errors::Error) -> Self {
        Self::Jni(error)
    }
}

impl From<WorkerError> for BoundaryError {
    fn from(error: WorkerError) -> Self {
        Self::Worker(error)
    }
}

impl Renderer {
    pub(super) fn can_capture(&self, generation: jlong) -> bool {
        self.accepts(generation) && !self.closed && !self.draining && !self.worker_failure
    }

    pub(super) fn submit(
        &mut self,
        snapshot: SectionSnapshot,
        key: jlong,
    ) -> Result<bool, WorkerError> {
        if !self.can_capture(wire(snapshot.generation())) || key <= 0 {
            return Ok(false);
        }
        if self.workers.is_none() {
            self.workers = Some(WorkerPool::new(snapshot.generation())?);
        }
        Ok(self
            .workers
            .as_ref()
            .is_some_and(|workers| workers.try_submit(snapshot, key.unsigned_abs()).is_ok()))
    }
}

pub(super) fn observe(
    renderer: &super::Lifecycle,
    event: jint,
    generation: jlong,
    thread: jlong,
    owner: bool,
) -> jlong {
    let retired = {
        let mut locked = renderer.lock().unwrap_or_else(PoisonError::into_inner);
        while locked.draining && matches!(event, super::RELOAD | super::UNLOAD) {
            locked = renderer.wait(locked);
        }
        let previous = locked.generation;
        let current = locked.observe(event, generation, thread, owner);
        let retired = if previous == current {
            None
        } else {
            locked.closed = false;
            retire(&mut locked)
        };
        drop(locked);
        retired
    };
    finish_retirement(renderer, retired);
    renderer
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .generation
}

fn retire(renderer: &mut Renderer) -> Option<WorkerPool> {
    let retired = renderer.workers.take();
    if let Some(workers) = &retired {
        renderer.draining = true;
        renderer.retired_stats = Some(workers.stats());
    }
    retired
}

struct Retirement<'a> {
    renderer: &'a super::Lifecycle,
    workers: Option<WorkerPool>,
    failed: bool,
}

impl Drop for Retirement<'_> {
    fn drop(&mut self) {
        // Join/drop the owner before waking a lifecycle waiter, even on unwind.
        drop(self.workers.take());
        let mut locked = self.renderer.lock().unwrap_or_else(PoisonError::into_inner);
        locked.worker_failure |= self.failed;
        locked.draining = false;
        drop(locked);
        self.renderer.drained.notify_all();
    }
}

fn finish_retirement(renderer: &super::Lifecycle, retired: Option<WorkerPool>) {
    let Some(workers) = retired else {
        return;
    };
    let mut retirement = Retirement {
        renderer,
        workers: Some(workers),
        failed: true,
    };
    if let Some(workers) = retirement.workers.as_mut() {
        let failed = workers.shutdown().is_err();
        renderer
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retired_stats = Some(workers.stats());
        retirement.failed = failed;
    }
}

pub(super) fn stop(renderer: &super::Lifecycle, generation: jlong) {
    let retired = {
        let mut locked = renderer.lock().unwrap_or_else(PoisonError::into_inner);
        while locked.draining {
            locked = renderer.wait(locked);
        }
        if !locked.accepts(generation) {
            return;
        }
        locked.closed = true;
        let retired = retire(&mut locked);
        drop(locked);
        retired
    };
    finish_retirement(renderer, retired);
}

pub(super) fn publish<T, E>(
    renderer: &super::Lifecycle,
    operation: jint,
    generation: jlong,
    key: jlong,
    publish: impl FnOnce(&[jlong]) -> Result<T, E>,
) -> Result<T, E> {
    if operation == 2 && key == 0 {
        stop(renderer, generation);
    }
    let locked = renderer.lock().unwrap_or_else(PoisonError::into_inner);
    if key != 0 || !locked.accepts(generation) || !matches!(operation, 0..=3) {
        return publish(&[]);
    }
    if operation == 3 {
        return match locked.retired_stats.as_ref() {
            Some(stats) => publish(&statistics_values(
                &locked,
                wire(stats.generation),
                Some(stats),
            )),
            None => publish(&[]),
        };
    }
    if operation != 0 {
        return publish(&statistics(&locked));
    }
    let Some(workers) = &locked.workers else {
        return publish(&[1, generation, 0, 11]);
    };
    let result = workers.consume_ready(|ready| {
        let mut values = [0; 4 + MAX_OUTSTANDING_JOBS * 11];
        values[..4].copy_from_slice(&[1, generation, 0, 11]);
        let mut length = 4;
        for completion in ready.iter().flatten() {
            if locked.accepts(wire(completion.generation)) {
                values[length..length + 11].copy_from_slice(&completion_values(completion));
                values[2] += 1;
                length += 11;
            }
        }
        publish(&values[..length])
    });
    drop(locked);
    result
}

fn completion_values(completion: &Completion) -> [jlong; 11] {
    let (status, vertices, indices, unsupported, visibility) =
        completion.result.as_ref().map_or((3, 0, 0, 0, 0), |mesh| {
            (
                if mesh.is_complete() { 1 } else { 2 },
                mesh.layers()
                    .iter()
                    .map(|layer| layer.vertices().len())
                    .sum(),
                mesh.layers()
                    .iter()
                    .map(|layer| layer.indices().len())
                    .sum(),
                mesh.unsupported_keys().len(),
                mesh.visibility().bits(),
            )
        });
    [
        wire(completion.generation),
        wire(completion.key),
        status,
        wire(completion.queue_wait_nanos),
        wire(completion.mesh_nanos),
        wire_usize(completion.output_bytes),
        wire_usize(completion.allocations),
        wire_usize(vertices),
        wire_usize(indices),
        wire_usize(unsupported),
        wire(visibility),
    ]
}

fn statistics(renderer: &Renderer) -> [jlong; 18] {
    let stats = renderer.workers.as_ref().map(WorkerPool::stats);
    let stats = stats.as_ref().or(renderer.retired_stats.as_ref());
    let stats =
        stats.filter(|stats| renderer.draining || wire(stats.generation) == renderer.generation);
    statistics_values(renderer, renderer.generation, stats)
}

fn statistics_values(
    renderer: &Renderer,
    generation: jlong,
    stats: Option<&WorkerStats>,
) -> [jlong; 18] {
    let Some(stats) = stats else {
        return [
            1,
            generation,
            1,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            jlong::from(renderer.draining),
            wire_usize(MAX_OUTSTANDING_JOBS),
            wire_usize(MAX_RESERVED_BYTES),
            jlong::from(renderer.worker_failure),
        ];
    };
    [
        1,
        generation,
        jlong::from(stats.stopped),
        wire_usize(stats.outstanding),
        wire_usize(stats.queued),
        wire_usize(stats.running),
        wire_usize(stats.completed),
        wire_usize(stats.reserved_bytes),
        wire_usize(stats.max_outstanding),
        wire_usize(stats.max_reserved_bytes),
        wire(stats.queue_wait_nanos),
        wire(stats.mesh_nanos),
        wire_usize(stats.output_bytes),
        wire_usize(stats.allocations),
        jlong::from(renderer.draining),
        wire_usize(MAX_OUTSTANDING_JOBS),
        wire_usize(MAX_RESERVED_BYTES),
        jlong::from(renderer.worker_failure),
    ]
}

fn wire(value: u64) -> jlong {
    jlong::try_from(value).unwrap_or(jlong::MAX)
}

fn wire_usize(value: usize) -> jlong {
    jlong::try_from(value).unwrap_or(jlong::MAX)
}

#[cfg(test)]
pub(super) fn exchange(
    renderer: &super::Lifecycle,
    operation: jint,
    generation: jlong,
    key: jlong,
) -> Result<Vec<jlong>, WorkerError> {
    publish(renderer, operation, generation, key, |values| {
        Ok(values.to_vec())
    })
}

#[cfg(test)]
pub(super) fn append_completion(values: &mut Vec<jlong>, completion: &Completion) {
    values.extend_from_slice(&completion_values(completion));
}
