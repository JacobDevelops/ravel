//! Selected-only copied baked inputs, independently emitted against vanilla's final buffer.

use super::status::BakedFailure;
use jni::objects::{JByteArray, JClass, JIntArray, JLongArray};
use jni::sys::jlong;
use jni::{Env, EnvUnowned};
use ravel_renderer::baked::{BakedMesh, BakedSnapshot, QuadInput};
use ravel_renderer::mesh::RenderLayer;
use std::sync::PoisonError;
use std::time::Instant;

const RECORD: usize = 232;
const MAX_QUADS: usize = 8192;
const INPUT_BOUND: usize = 32 + MAX_QUADS * RECORD;
const ORACLE_BOUND: usize = 16 + MAX_QUADS * 112;

mod residency;
pub(super) use residency::Residency;

/// Publishes bounded Rust-emitted BLOCK words separately from the final-buffer oracle.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelRendererBaked")]
pub fn ravel_renderer_baked<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    generation: jlong,
    origin: JLongArray<'local>,
    source: JByteArray<'local>,
    oracle: JByteArray<'local>,
    output: JIntArray<'local>,
) -> JLongArray<'local> {
    env.with_env(
        |env: &mut Env<'local>| -> Result<_, super::work::BoundaryError> {
            let result = admit(&super::RENDERER, generation, || {
                if origin.len(env)? != 3 {
                    return publish(env, &BakedFailure::Bounds.receipt());
                }
                let mut section = [0; 3];
                origin.get_region(env, 0, &mut section)?;
                let input_size = source.len(env)?;
                let oracle_size = oracle.len(env)?;
                if !valid_bounds(input_size, oracle_size, output.len(env)?) {
                    return publish(env, &BakedFailure::Bounds.receipt());
                }
                if input_size < 32 {
                    return publish(env, &BakedFailure::Decode.receipt());
                }
                let mut header = [0_i8; 4];
                source.get_region(env, 0, &mut header)?;
                let count = usize::try_from(u32::from_le_bytes(header.map(i8::cast_unsigned)))
                    .unwrap_or(usize::MAX);
                if count > MAX_QUADS || input_size < 32 + count * RECORD {
                    return publish(env, &BakedFailure::Decode.receipt());
                }
                let mut copied = vec![0_i8; 32 + count * RECORD];
                source.get_region(env, 0, &mut copied)?;
                let source = copied
                    .into_iter()
                    .map(i8::cast_unsigned)
                    .collect::<Vec<_>>();
                let oracle = env.convert_byte_array(&oracle)?;
                let mut emitted = Vec::new();
                let mut mesh = None;
                let result = emit_and_compare_with(
                    &source,
                    &oracle,
                    &mut emitted,
                    || !active(generation),
                    Vec::try_reserve_exact,
                    |value| mesh = Some(value),
                );
                let values = commit(
                    &super::RENDERER,
                    generation,
                    section,
                    Prepared {
                        source,
                        emitted,
                        result,
                        mesh,
                    },
                    |words| output.set_region(env, 0, words),
                )?;
                publish(env, &values)
            })?;
            match result {
                Some(receipt) => Ok(receipt),
                None => Ok(JLongArray::new(env, 0)?),
            }
        },
    )
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

fn valid_bounds(input: usize, oracle: usize, output: usize) -> bool {
    input <= INPUT_BOUND && oracle <= ORACLE_BOUND && output == 4 + MAX_QUADS * 28
}

fn admit<T, E>(
    renderer: &super::Lifecycle,
    generation: jlong,
    capture: impl FnOnce() -> Result<T, E>,
) -> Result<Option<T>, E> {
    let _permit = match renderer.capture.try_lock() {
        Ok(permit) => permit,
        Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner(),
        Err(std::sync::TryLockError::WouldBlock) => return Ok(None),
    };
    {
        let locked = renderer.lock().unwrap_or_else(PoisonError::into_inner);
        if !locked.can_capture(generation) || locked.shaded.busy() {
            return Ok(None);
        }
    }
    capture().map(Some)
}

struct Prepared {
    source: Vec<u8>,
    emitted: Vec<i32>,
    result: [jlong; 6],
    mesh: Option<BakedMesh>,
}

fn commit<E>(
    renderer: &super::Lifecycle,
    generation: jlong,
    origin: [jlong; 3],
    prepared: Prepared,
    write: impl FnOnce(&[i32]) -> Result<(), E>,
) -> Result<Vec<jlong>, E> {
    let Prepared {
        source,
        emitted,
        result,
        mesh,
    } = prepared;
    let mut locked = renderer.lock().unwrap_or_else(PoisonError::into_inner);
    if !locked.can_capture(generation) {
        return Ok(Vec::new());
    }
    if result[0] < 0 {
        return Ok(result.to_vec());
    }
    write(&emitted)?;
    let resource = |offset| {
        jlong::from(word(&source, offset).expect("admitted resource word"))
            | (jlong::from(word(&source, offset + 4).expect("admitted resource word")) << 32)
    };
    let identity = [
        generation,
        0,
        resource(16),
        resource(24),
        origin[0],
        origin[1],
        origin[2],
    ];
    let Some(mesh) = mesh else {
        return Ok(Vec::new());
    };
    let Some(key) = locked
        .shaded
        .publish(residency::Retained { identity, mesh })
    else {
        return Ok(Vec::new());
    };
    drop(locked);
    Ok(vec![
        result[0], result[1], result[2], result[3], result[4], result[5], key,
    ])
}

/// Settles only the exact pending shaded transfer; zero observes bounded residency.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelRendererBakedLease")]
pub fn ravel_renderer_baked_lease<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    generation: jlong,
    key: jlong,
    operation: jni::sys::jint,
) -> JLongArray<'local> {
    env.with_env(
        |env: &mut Env<'local>| -> Result<_, super::work::BoundaryError> {
            let mut locked = super::RENDERER
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if operation == 3 {
                let configured = locked.can_capture(generation) && locked.shaded.configure(key);
                return publish(env, &[jlong::from(configured)]);
            }
            if operation == 4 {
                let current =
                    locked.can_capture(generation) && locked.shaded.contains(generation, key);
                return publish(env, &[jlong::from(current)]);
            }
            if operation == 5 {
                return publish(env, &locked.shaded.bounds());
            }
            if operation == 6 {
                return publish(env, &locked.shaded.dedupe());
            }
            if operation != 0
                && (!locked.can_capture(generation)
                    || !matches!(operation, 1 | 2)
                    || !locked.shaded.settle(generation, key, operation == 1))
            {
                return Ok(JLongArray::new(env, 0)?);
            }
            publish(env, &locked.shaded.metrics())
        },
    )
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

/// Reorders only the exact resident lease on a vanilla transparency upload.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelRendererBakedResort")]
pub fn ravel_renderer_baked_resort<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    generation: jlong,
    key: jlong,
    camera: jni::objects::JFloatArray<'local>,
    output: JIntArray<'local>,
) -> jni::sys::jint {
    env.with_env(
        |env: &mut Env<'local>| -> Result<_, super::work::BoundaryError> {
            if camera.len(env)? != 3 || !(4..=4 + MAX_QUADS * 28).contains(&output.len(env)?) {
                return Ok(0);
            }
            let mut position = [0.; 3];
            camera.get_region(env, 0, &mut position)?;
            let mut locked = super::RENDERER
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if !locked.can_capture(generation) {
                return Ok(0);
            }
            let Some(mesh) = locked
                .shaded
                .resort(generation, key, position, output.len(env)?)
            else {
                return Ok(0);
            };
            for (index, quad) in mesh.layers()[3].iter().enumerate() {
                let mut vertices = quad.vertices.iter().flatten();
                let words: [i32; 28] = std::array::from_fn(|_| {
                    i32::from_ne_bytes(vertices.next().expect("fixed quad words").to_ne_bytes())
                });
                let offset = i32::try_from(4 + index * 28).expect("bounded output offset");
                output.set_region(env, offset, &words)?;
            }
            drop(locked);
            Ok(1)
        },
    )
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

fn active(generation: jlong) -> bool {
    super::RENDERER
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .can_capture(generation)
}

fn publish<'local>(
    env: &mut Env<'local>,
    values: &[jlong],
) -> Result<JLongArray<'local>, super::work::BoundaryError> {
    let result = JLongArray::new(env, values.len())?;
    result.set_region(env, 0, values)?;
    Ok(result)
}

#[cfg(test)]
fn emit_and_compare(
    source: &[u8],
    oracle: &[u8],
    output: &mut Vec<i32>,
    cancelled: impl Fn() -> bool,
) -> [jlong; 6] {
    emit_and_compare_with(
        source,
        oracle,
        output,
        cancelled,
        Vec::try_reserve_exact,
        |_| {},
    )
}

fn emit_and_compare_with(
    source: &[u8],
    oracle: &[u8],
    output: &mut Vec<i32>,
    cancelled: impl Fn() -> bool,
    reserve: impl FnOnce(&mut Vec<i32>, usize) -> Result<(), std::collections::TryReserveError>,
    retain: impl FnOnce(BakedMesh),
) -> [jlong; 6] {
    let started = Instant::now();
    let Some((snapshot, camera)) = decode(source) else {
        return BakedFailure::Decode.receipt();
    };
    let input_bytes = snapshot.heap_bytes() + source.len() + oracle.len();
    let Ok(mesh) = snapshot.mesh(snapshot.generation(), camera, &cancelled) else {
        return BakedFailure::Emission.receipt();
    };
    if oracle.len() < 16 {
        return BakedFailure::OracleHeader.receipt();
    }
    if reserve(output, 4 + mesh.payload_bytes().div_euclid(4)).is_err() {
        return BakedFailure::Allocation.receipt();
    }
    output.resize(4, 0);
    let mut offset = 16;
    let mut quads = 0;
    let mut mismatches = 0;
    let mut first = -1;
    for (layer_index, layer) in mesh.layers().iter().enumerate() {
        if cancelled() {
            return BakedFailure::Emission.receipt();
        }
        if word(oracle, layer_index * 4)
            != Some(u32::try_from(layer.len()).expect("bounded layer count"))
        {
            let mut failure = BakedFailure::LayerCount.receipt();
            failure[2] = super::metric_long(layer_index);
            return failure;
        }
        output[layer_index] = i32::try_from(layer.len()).expect("bounded layer count");
        for quad in layer {
            quads += 1;
            for expected in quad.vertices.into_iter().flatten() {
                output.push(i32::from_ne_bytes(expected.to_ne_bytes()));
                let Some(actual) = word(oracle, offset) else {
                    return BakedFailure::OraclePayload.receipt();
                };
                if expected != actual {
                    if first < 0 {
                        first = super::metric_long((offset - 16).div_euclid(4));
                    }
                    mismatches += 1;
                }
                offset += 4;
            }
        }
    }
    let receipt = [
        quads,
        mismatches,
        first,
        super::metric_long(started.elapsed().as_nanos()),
        super::metric_long(input_bytes),
        super::metric_long(mesh.heap_bytes() + output.capacity() * 4),
    ];
    retain(mesh);
    receipt
}

fn decode(source: &[u8]) -> Option<(BakedSnapshot, [f32; 3])> {
    let count = usize::try_from(word(source, 0)?).ok()?;
    if count > MAX_QUADS || source.len() < 32 + count * RECORD {
        return None;
    }
    let camera = std::array::from_fn(|axis| {
        f32::from_bits(word(source, 4 + axis * 4).expect("bounded header word"))
    });
    let mut quads = Vec::new();
    quads.try_reserve_exact(count).ok()?;
    for index in 0..count {
        let base = 32 + index * RECORD;
        let layer = match word(source, base + 8)? {
            0 => RenderLayer::Solid,
            1 => RenderLayer::CutoutMipped,
            2 => RenderLayer::Cutout,
            3 => RenderLayer::Translucent,
            _ => return None,
        };
        quads.push(QuadInput {
            state_key: word(source, base)?,
            sprite: word(source, base + 4)?,
            layer,
            tint: word(source, base + 12)?,
            offset: std::array::from_fn(|axis| {
                let offset = base + 16 + axis * 8;
                u64::from(word(source, offset).expect("bounded record word"))
                    | (u64::from(word(source, offset + 4).expect("bounded record word")) << 32)
            }),
            vertices: std::array::from_fn(|vertex| {
                std::array::from_fn(|item| {
                    word(source, base + 40 + (vertex * 7 + item) * 4).expect("bounded record word")
                })
            }),
            ambient_occlusion: std::array::from_fn(|item| {
                word(source, base + 152 + item * 4).expect("bounded record word")
            }),
            multipliers: std::array::from_fn(|vertex| {
                std::array::from_fn(|axis| {
                    word(source, base + 168 + (vertex * 3 + axis) * 4).expect("bounded record word")
                })
            }),
            combined_light: std::array::from_fn(|item| {
                word(source, base + 216 + item * 4).expect("bounded record word")
            }),
        });
    }
    let generation = u64::from(word(source, 16)?) | (u64::from(word(source, 20)?) << 32);
    let atlas = u64::from(word(source, 24)?) | (u64::from(word(source, 28)?) << 32);
    Some((BakedSnapshot::new(generation, atlas, quads).ok()?, camera))
}

fn word(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

#[cfg(test)]
mod tests;
