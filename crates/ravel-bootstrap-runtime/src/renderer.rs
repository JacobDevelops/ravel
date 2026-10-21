//! Process/client-scoped renderer observations, independent of runtime selection.
//!
//! Only copied scalars and bounded primitive batches cross this seam. Native
//! workers own snapshots and meshes, never game handles or GL submission. The
//! first admitted frame/reload pins the Java client thread for this process.
//!
//! Event 0 queries the authoritative generation without claiming a thread. All
//! other events require the exact current token. Reload/unload invalidate it;
//! ordinary observations never change it. Tokens start at 1 and remain positive
//! Java longs. Exhausting `i64::MAX` permanently closes admission with token 0,
//! rather than wrapping and making old work current again.
//!
//! Each observation and acceptance check is individually linearized by the
//! mutex. Separate `accept` and event calls do not share a critical section:
//! `accept` is a point-in-time check, not a lease across later Java/GL work.
//! Java must recheck its generation before accepting drained completion metadata.

use std::sync::{Condvar, LockResult, Mutex, MutexGuard, PoisonError};

use jni::objects::{JByteArray, JClass, JIntArray, JLongArray};
use jni::sys::{JNI_FALSE, jboolean, jint, jlong};
use jni::{Env, EnvUnowned};

mod baked;
mod snapshot;
mod work;
pub use baked::ravel_renderer_baked;

// Shared wire values with the Java renderer observer (not raw hook IDs).
const CURRENT: jint = 0;
const RELOAD: jint = 1;
const UNLOAD: jint = 2;
const FRAME: jint = 3;
const TERRAIN: jint = 4;
const LAYER: jint = 5;
const UPDATE: jint = 6;
const COMPILE: jint = 7;
const TASK: jint = 8;
const UPLOAD: jint = 9;

static RENDERER: Lifecycle = Lifecycle::new(Renderer::new());

fn metric_long(value: impl TryInto<jlong>) -> jlong {
    value.try_into().unwrap_or(jlong::MAX)
}

struct Lifecycle {
    state: Mutex<Renderer>,
    drained: Condvar,
    capture: Mutex<()>,
}

impl Lifecycle {
    const fn new(renderer: Renderer) -> Self {
        Self {
            state: Mutex::new(renderer),
            drained: Condvar::new(),
            capture: Mutex::new(()),
        }
    }

    fn lock(&self) -> LockResult<MutexGuard<'_, Renderer>> {
        self.state.lock()
    }

    fn wait<'a>(&self, locked: MutexGuard<'a, Renderer>) -> MutexGuard<'a, Renderer> {
        self.drained
            .wait(locked)
            .unwrap_or_else(PoisonError::into_inner)
    }
}

struct Renderer {
    generation: jlong,
    owner_thread: jlong,
    workers: Option<ravel_renderer::workers::WorkerPool>,
    retired_stats: Option<ravel_renderer::workers::WorkerStats>,
    draining: bool,
    closed: bool,
    worker_failure: bool,
}

impl Renderer {
    const fn new() -> Self {
        Self {
            generation: 1,
            owner_thread: 0,
            workers: None,
            retired_stats: None,
            draining: false,
            closed: false,
            worker_failure: false,
        }
    }

    fn accepts(&self, generation: jlong) -> bool {
        generation > 0 && generation == self.generation
    }

    fn admits(
        &self,
        event: jint,
        generation: jlong,
        thread_id: jlong,
        on_renderer_thread: bool,
    ) -> bool {
        if !matches!(
            event,
            RELOAD | UNLOAD | FRAME | TERRAIN | LAYER | UPDATE | COMPILE | TASK | UPLOAD
        ) || !self.accepts(generation)
            || thread_id <= 0
        {
            return false;
        }
        if on_renderer_thread {
            if self.owner_thread == 0 {
                matches!(event, RELOAD | FRAME)
            } else {
                self.owner_thread == thread_id
            }
        } else {
            matches!(event, COMPILE | TASK) && self.owner_thread != 0
        }
    }

    fn observe(
        &mut self,
        event: jint,
        generation: jlong,
        thread_id: jlong,
        on_renderer_thread: bool,
    ) -> jlong {
        if event == CURRENT {
            return self.generation;
        }
        if !self.admits(event, generation, thread_id, on_renderer_thread) {
            return self.generation;
        }
        if on_renderer_thread {
            self.owner_thread = thread_id;
        }
        if matches!(event, RELOAD | UNLOAD) {
            self.generation = self.generation.checked_add(1).unwrap_or(0);
        }
        self.generation
    }
}

/// Observes copied renderer lifecycle scalars and returns the current generation.
///
/// Rejected observations return the same authoritative token without mutation;
/// callers must never relabel stale worker output with that returned token.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelRendererEvent")]
pub fn ravel_renderer_event<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    event: jint,
    generation: jlong,
    thread_id: jlong,
    on_renderer_thread: jboolean,
) -> jlong {
    env.with_env(
        |_env: &mut Env<'local>| -> Result<jlong, work::BoundaryError> {
            Ok(work::observe(
                &RENDERER,
                event,
                generation,
                thread_id,
                on_renderer_thread != JNI_FALSE,
            ))
        },
    )
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

/// Checks a copied worker generation under the same gate used by invalidation.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelRendererAccept")]
pub fn ravel_renderer_accept<'local>(
    _env: EnvUnowned<'local>,
    _class: JClass<'local>,
    generation: jlong,
) -> jboolean {
    jboolean::from(
        RENDERER
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .accepts(generation),
    )
}

/// Compares every compactly decoded cell with a bounded vanilla state/light batch.
///
/// Returns the existing six comparison values followed by `[admitted, key]`,
/// structured failure values, or empty for stale/busy/closed/invalid-key input. Only compared
/// immutable snapshots can enter the bounded native worker pool.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelRendererSnapshot")]
pub fn ravel_renderer_snapshot<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    generation: jlong,
    key: jlong,
    states: JIntArray<'local>,
    light: JByteArray<'local>,
    geometry: JByteArray<'local>,
) -> JLongArray<'local> {
    env.with_env(
        |env: &mut Env<'local>| -> Result<JLongArray<'local>, work::BoundaryError> {
            let result = JLongArray::new(env, 8)?;
            let receipt = snapshot::admit(&RENDERER, generation, key, || {
                snapshot::copy_and_compare(env, generation, &states, &light, &geometry)
            })?;
            let Some(receipt) = receipt else {
                return Ok(JLongArray::new(env, 0)?);
            };
            Ok(snapshot::publish(
                &RENDERER,
                generation,
                receipt,
                |values| {
                    if values.is_empty() {
                        return JLongArray::new(env, 0);
                    }
                    result.set_region(env, 0, values)?;
                    Ok(result)
                },
            )?)
        },
    )
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

/// Drains bounded native metadata, queries statistics, or synchronously stops workers.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelRendererWork")]
pub fn ravel_renderer_work<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    operation: jint,
    generation: jlong,
    key: jlong,
) -> JLongArray<'local> {
    env.with_env(
        |env: &mut Env<'local>| -> Result<JLongArray<'local>, work::BoundaryError> {
            work::publish(&RENDERER, operation, generation, key, |values| {
                let result = JLongArray::new(env, values.len())?;
                result.set_region(env, 0, values)?;
                Ok(result)
            })
        },
    )
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

/// Copies a bounded retained solid mesh into Java-owned memory without consuming it.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelRendererMesh")]
pub fn ravel_renderer_mesh<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    generation: jlong,
    key: jlong,
) -> JByteArray<'local> {
    env.with_env(
        |env: &mut Env<'local>| -> Result<JByteArray<'local>, work::BoundaryError> {
            work::publish_mesh(&RENDERER, generation, key, |bytes| {
                Ok(env.byte_array_from_slice(bytes)?)
            })
        },
    )
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod work_tests;
