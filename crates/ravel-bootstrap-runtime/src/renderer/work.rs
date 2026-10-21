//! Bounded metadata polling and retained CPU geometry publication; Java owns GL.

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
    Mesh(ravel_renderer::mesh::MeshError),
}

impl std::fmt::Display for BoundaryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Jni(error) => std::fmt::Display::fmt(error, formatter),
            Self::Worker(error) => std::fmt::Display::fmt(error, formatter),
            Self::Mesh(error) => write!(formatter, "mesh transfer failed: {error}"),
        }
    }
}

impl std::error::Error for BoundaryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Jni(error) => Some(error),
            Self::Worker(error) => Some(error),
            Self::Mesh(error) => Some(error),
        }
    }
}

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

pub(super) fn publish_mesh<T>(
    renderer: &super::Lifecycle,
    generation: jlong,
    key: jlong,
    publish: impl FnOnce(&[u8]) -> Result<T, BoundaryError>,
) -> Result<T, BoundaryError> {
    let locked = renderer.lock().unwrap_or_else(PoisonError::into_inner);
    if !locked.can_capture(generation) || key <= 0 {
        return publish(&[]);
    }
    let Some(workers) = &locked.workers else {
        return publish(&[]);
    };
    let bytes = workers
        .with_completion(
            generation.unsigned_abs(),
            key.unsigned_abs(),
            |completion| completion.map(ravel_renderer::wire::encode).transpose(),
        )
        .map_err(BoundaryError::Mesh)?;
    // The owned encoding no longer borrows the pool. Keep the lifecycle gate
    // locked through Java publication so stop/reload/ack cannot race the copy.
    let result = publish(bytes.as_deref().unwrap_or_default());
    drop(locked);
    result
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
        if !self.can_capture(super::metric_long(snapshot.generation())) || key <= 0 {
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
    on_renderer_thread: bool,
) -> jlong {
    let retired = {
        let mut locked = renderer.lock().unwrap_or_else(PoisonError::into_inner);
        while locked.draining && matches!(event, super::RELOAD | super::UNLOAD) {
            locked = renderer.wait(locked);
        }
        let previous = locked.generation;
        let current = locked.observe(event, generation, thread, on_renderer_thread);
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
    renderer.shaded.retire();
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

#[derive(Clone, Copy, Eq, PartialEq)]
enum Operation {
    Consume,
    Statistics,
    Close,
    RetiredStatistics,
    Peek,
    Acknowledge,
}

impl TryFrom<jint> for Operation {
    type Error = ();

    fn try_from(value: jint) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Consume),
            1 => Ok(Self::Statistics),
            2 => Ok(Self::Close),
            3 => Ok(Self::RetiredStatistics),
            4 => Ok(Self::Peek),
            5 => Ok(Self::Acknowledge),
            _ => Err(()),
        }
    }
}

pub(super) fn publish<T, E>(
    renderer: &super::Lifecycle,
    operation: jint,
    generation: jlong,
    key: jlong,
    publish: impl FnOnce(&[jlong]) -> Result<T, E>,
) -> Result<T, E> {
    let Ok(operation) = Operation::try_from(operation) else {
        return publish(&[]);
    };
    if operation == Operation::Close && key == 0 {
        stop(renderer, generation);
    }
    let locked = renderer.lock().unwrap_or_else(PoisonError::into_inner);
    if !locked.accepts(generation) || (operation != Operation::Acknowledge && key != 0) {
        return publish(&[]);
    }
    if operation == Operation::Acknowledge {
        if !locked.can_capture(generation) || key <= 0 {
            return publish(&[]);
        }
        let Some(workers) = &locked.workers else {
            return publish(&[0]);
        };
        return workers.publish_acknowledgement(
            generation.unsigned_abs(),
            key.unsigned_abs(),
            |acknowledged| publish(&[jlong::from(acknowledged)]),
        );
    }
    if operation == Operation::Peek && !locked.can_capture(generation) {
        return publish(&[]);
    }
    if operation == Operation::RetiredStatistics {
        return match locked.retired_stats.as_ref() {
            Some(stats) => publish(&statistics_values(
                &locked,
                super::metric_long(stats.generation),
                Some(stats),
            )),
            None => publish(&[]),
        };
    }
    if !matches!(operation, Operation::Consume | Operation::Peek) {
        return publish(&statistics(&locked));
    }
    let Some(workers) = &locked.workers else {
        return publish(&[1, generation, 0, 11]);
    };
    let metadata = |ready: &[Option<&Completion>; MAX_OUTSTANDING_JOBS]| {
        publish_metadata(&locked, ready, publish)
    };
    let result = if operation == Operation::Peek {
        workers.peek_ready(metadata)
    } else {
        workers.consume_ready(metadata)
    };
    drop(locked);
    result
}

fn publish_metadata<T, E>(
    renderer: &Renderer,
    ready: &[Option<&Completion>; MAX_OUTSTANDING_JOBS],
    publish: impl FnOnce(&[jlong]) -> Result<T, E>,
) -> Result<T, E> {
    let mut values = [0; 4 + MAX_OUTSTANDING_JOBS * 11];
    values[..4].copy_from_slice(&[1, renderer.generation, 0, 11]);
    let mut length = 4;
    for completion in ready.iter().flatten() {
        if renderer.accepts(super::metric_long(completion.generation)) {
            values[length..length + 11].copy_from_slice(&completion_values(completion));
            values[2] += 1;
            length += 11;
        }
    }
    publish(&values[..length])
}

fn completion_values(completion: &Completion) -> [jlong; 11] {
    let (vertices, indices, unsupported, visibility) =
        completion.result.as_ref().map_or((0, 0, 0, 0), |mesh| {
            (
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
        super::metric_long(completion.generation),
        super::metric_long(completion.job_id),
        jlong::from(completion.outcome().code()),
        super::metric_long(completion.queue_wait_nanos),
        super::metric_long(completion.mesh_nanos),
        super::metric_long(completion.output_bytes),
        super::metric_long(completion.allocations),
        super::metric_long(vertices),
        super::metric_long(indices),
        super::metric_long(unsupported),
        super::metric_long(visibility),
    ]
}

fn statistics(renderer: &Renderer) -> [jlong; 18] {
    let stats = renderer.workers.as_ref().map(WorkerPool::stats);
    let stats = stats.as_ref().or(renderer.retired_stats.as_ref());
    let stats = stats.filter(|stats| {
        renderer.draining || super::metric_long(stats.generation) == renderer.generation
    });
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
            super::metric_long(MAX_OUTSTANDING_JOBS),
            super::metric_long(MAX_RESERVED_BYTES),
            jlong::from(renderer.worker_failure),
        ];
    };
    [
        1,
        generation,
        jlong::from(stats.stopped),
        super::metric_long(stats.outstanding),
        super::metric_long(stats.queued),
        super::metric_long(stats.running),
        super::metric_long(stats.completed),
        super::metric_long(stats.reserved_bytes),
        super::metric_long(stats.max_outstanding),
        super::metric_long(stats.max_reserved_bytes),
        super::metric_long(stats.queue_wait_nanos),
        super::metric_long(stats.mesh_nanos),
        super::metric_long(stats.output_bytes),
        super::metric_long(stats.allocations),
        jlong::from(renderer.draining),
        super::metric_long(MAX_OUTSTANDING_JOBS),
        super::metric_long(MAX_RESERVED_BYTES),
        jlong::from(renderer.worker_failure),
    ]
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
