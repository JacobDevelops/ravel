//! Process/client-scoped renderer observations, independent of runtime selection.
//!
//! Only copied scalars cross this seam: no game handles, meshes, callbacks or GL
//! submission. The first admitted frame/reload observation pins the Java client
//! thread for this process. Workers may observe compile/task activity only after that.
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
//! These observations do not authorize submission or replace the renderer.

use std::sync::{Mutex, PoisonError};

use jni::EnvUnowned;
use jni::objects::JClass;
use jni::sys::{JNI_FALSE, jboolean, jint, jlong};

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

static RENDERER: Mutex<Renderer> = Mutex::new(Renderer::new());

struct Renderer {
    generation: jlong,
    owner_thread: jlong,
}

impl Renderer {
    const fn new() -> Self {
        Self {
            generation: 1,
            owner_thread: 0,
        }
    }

    fn accepts(&self, generation: jlong) -> bool {
        generation > 0 && generation == self.generation
    }

    fn admits(&self, event: jint, generation: jlong, thread_id: jlong, owner_thread: bool) -> bool {
        if !matches!(
            event,
            RELOAD | UNLOAD | FRAME | TERRAIN | LAYER | UPDATE | COMPILE | TASK | UPLOAD
        ) || !self.accepts(generation)
            || thread_id <= 0
        {
            return false;
        }
        if owner_thread {
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
        owner_thread: bool,
    ) -> jlong {
        if event == CURRENT {
            return self.generation;
        }
        if !self.admits(event, generation, thread_id, owner_thread) {
            return self.generation;
        }
        if owner_thread {
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
    _env: EnvUnowned<'local>,
    _class: JClass<'local>,
    event: jint,
    generation: jlong,
    thread_id: jlong,
    owner_thread: jboolean,
) -> jlong {
    RENDERER
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .observe(event, generation, thread_id, owner_thread != JNI_FALSE)
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

#[cfg(test)]
mod tests;
