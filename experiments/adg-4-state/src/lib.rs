//! ADG-4 native side of the three state-ownership models.
//!
//! The same flow is computed three ways. Only who holds the state differs: the
//! JVM object, this library, or both. Everything else — the arithmetic, the
//! sentinels, the failure behaviour — is shared, so the measured difference is
//! ownership, and the difference is visible as a trade: bytes duplicated against
//! bytes crossed per flow.

use std::cell::Cell;
use std::process;

use jni::objects::JClass;
use jni::sys::{jdouble, jint, jlong};
use jni::{Env, EnvUnowned, jni_str};

/// Value a flow returns for input it rejects.
const REJECTED: jdouble = jdouble::NAN;

/// Bytes one copy of the state occupies: four doubles.
const STATE_BYTES: jlong = 4 * 8;

thread_local! {
    /// The state this library holds, in the models where it holds any.
    static HELD: Cell<State> = const { Cell::new(State::EMPTY) };
}

/// The entity-shaped state one flow reads.
#[derive(Clone, Copy)]
struct State {
    x: jdouble,
    y: jdouble,
    dx: jdouble,
    dy: jdouble,
}

impl State {
    const EMPTY: Self = Self {
        x: 0.0,
        y: 0.0,
        dx: 0.0,
        dy: 0.0,
    };

    /// The one value a flow computes. Shared by every model.
    fn flow(self) -> jdouble {
        self.x
            .mul_add(self.dx, self.y * self.dy)
            .mul_add(0.5, self.dx)
    }
}

/// Whether a payload is one of the sentinels the failure matrix uses.
fn sentinel(step: jdouble) -> bool {
    !step.is_finite()
}

/// Applies a sentinel, which either rejects, throws, or ends the process.
fn reject(unowned: &mut EnvUnowned<'_>, step: jdouble) -> jdouble {
    if step == jdouble::INFINITY {
        // Deliberate crash for the ADG-4 failure matrix.
        process::abort();
    }
    if step == jdouble::NEG_INFINITY {
        let outcome = unowned.with_env(|env: &mut Env<'_>| {
            env.throw_new(
                jni_str!("java/lang/IllegalStateException"),
                jni_str!("the flow failed"),
            )
        });
        let () = outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>();
    }
    REJECTED
}

/// Java owns the state: every flow ships the whole of what it needs.
///
/// This library keeps nothing between flows, so nothing can go stale and nothing
/// is duplicated. The price is that the state crosses the boundary every time.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_ravel_spike4_Flow_javaAuthoritative(
    mut unowned: EnvUnowned<'_>,
    _class: JClass<'_>,
    x: jdouble,
    y: jdouble,
    dx: jdouble,
    dy: jdouble,
) -> jdouble {
    if sentinel(dx) {
        return reject(&mut unowned, dx);
    }
    State { x, y, dx, dy }.flow()
}

/// This library owns the state: Java ships a handle and the one value that moved.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_ravel_spike4_Flow_rustAuthoritative(
    mut unowned: EnvUnowned<'_>,
    _class: JClass<'_>,
    handle: jlong,
    step: jdouble,
) -> jdouble {
    if handle == 0 {
        return REJECTED;
    }
    if sentinel(step) {
        return reject(&mut unowned, step);
    }
    HELD.with(|held| {
        let mut state = held.get();
        state.dx = step;
        let value = state.flow();
        // Owning the state means owning its evolution too.
        state.x += value;
        held.set(state);
        value
    })
}

/// Both sides hold a copy: Java ships only the value that moved.
///
/// The mirror is what buys the smaller flow, and the mirror is what can go stale.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_ravel_spike4_Flow_mirrored(
    mut unowned: EnvUnowned<'_>,
    _class: JClass<'_>,
    step: jdouble,
) -> jdouble {
    if sentinel(step) {
        return reject(&mut unowned, step);
    }
    HELD.with(|held| {
        let mut state = held.get();
        state.dx = step;
        held.set(state);
        state.flow()
    })
}

/// Seeds the state this library holds and returns its handle.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_ravel_spike4_Flow_createState(
    _unowned: EnvUnowned<'_>,
    _class: JClass<'_>,
    x: jdouble,
    y: jdouble,
    dx: jdouble,
    dy: jdouble,
) -> jlong {
    HELD.with(|held| held.set(State { x, y, dx, dy }));
    // One state, so one handle. A real loader would hand out an index.
    1
}

/// Bytes this library holds for a model, which is that model's duplicated state.
///
/// Models are numbered rather than named, because a name would mean a string
/// crossing the boundary for a question asked once.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_ravel_spike4_Flow_duplicatedBytes(
    _unowned: EnvUnowned<'_>,
    _class: JClass<'_>,
    model: jint,
) -> jlong {
    match model {
        // Java holds the only copy; this library holds nothing between flows.
        0 => 0,
        // This library holds the only copy; Java holds an eight-byte handle.
        1 => 8,
        // Both sides hold the four values.
        2 => STATE_BYTES,
        _ => -1,
    }
}
