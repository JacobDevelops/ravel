//! Selected-only copied baked inputs, independently emitted against vanilla's final buffer.

use jni::objects::{JByteArray, JClass, JIntArray, JLongArray};
use jni::sys::jlong;
use jni::{Env, EnvUnowned};
use ravel_renderer::baked::{BakedSnapshot, QuadInput};
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
            let _permit = match super::RENDERER.capture.try_lock() {
                Ok(permit) => permit,
                Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner(),
                Err(std::sync::TryLockError::WouldBlock) => return Ok(JLongArray::new(env, 0)?),
            };
            if !active(generation)
                || super::RENDERER
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .shaded
                    .busy()
            {
                return Ok(JLongArray::new(env, 0)?);
            }
            if origin.len(env)? != 3 {
                return publish(env, &[-1, 1, 0, 0, 0, 0]);
            }
            let mut section = [0; 3];
            origin.get_region(env, 0, &mut section)?;
            let input_size = source.len(env)?;
            let oracle_size = oracle.len(env)?;
            if input_size > INPUT_BOUND
                || oracle_size > ORACLE_BOUND
                || output.len(env)? != 4 + MAX_QUADS * 28
            {
                return publish(env, &[-1, 1, 0, 0, 0, 0]);
            }
            if input_size < 32 {
                return publish(env, &[-1, 2, 0, 0, 0, 0]);
            }
            let mut header = [0_i8; 4];
            source.get_region(env, 0, &mut header)?;
            let count = usize::try_from(u32::from_le_bytes(header.map(i8::cast_unsigned)))
                .unwrap_or(usize::MAX);
            if count > MAX_QUADS || input_size < 32 + count * RECORD {
                return publish(env, &[-1, 2, 0, 0, 0, 0]);
            }
            let mut copied = vec![0_i8; 32 + count * RECORD];
            source.get_region(env, 0, &mut copied)?;
            let source = copied
                .into_iter()
                .map(i8::cast_unsigned)
                .collect::<Vec<_>>();
            let oracle = env.convert_byte_array(&oracle)?;
            let mut emitted = Vec::new();
            let result = compare(generation.unsigned_abs(), &source, &oracle, &mut emitted);
            complete(env, generation, section, source, emitted, result, &output)
        },
    )
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

fn complete<'local>(
    env: &mut Env<'local>,
    generation: jlong,
    origin: [jlong; 3],
    source: Vec<u8>,
    emitted: Vec<i32>,
    result: [jlong; 6],
    output: &JIntArray<'local>,
) -> Result<JLongArray<'local>, super::work::BoundaryError> {
    let mut locked = super::RENDERER
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if !locked.can_capture(generation) {
        return Ok(JLongArray::new(env, 0)?);
    }
    if result[0] < 0 {
        return publish(env, &result);
    }
    output.set_region(env, 0, &emitted)?;
    let resource = |offset| {
        jlong::from(word(&source, offset).unwrap_or_default())
            | (jlong::from(word(&source, offset + 4).unwrap_or_default()) << 32)
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
    let Some(key) = locked.shaded.publish(residency::Retained {
        identity,
        source,
        emitted,
    }) else {
        return Ok(JLongArray::new(env, 0)?);
    };
    drop(locked);
    publish(
        env,
        &[
            result[0], result[1], result[2], result[3], result[4], result[5], key,
        ],
    )
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

fn compare(generation: u64, source: &[u8], oracle: &[u8], output: &mut Vec<i32>) -> [jlong; 6] {
    let started = Instant::now();
    let Some((snapshot, camera)) = decode(source) else {
        return [-1, 2, 0, 0, 0, 0];
    };
    let input_bytes = snapshot.heap_bytes() + source.len() + oracle.len();
    let Ok(mesh) = snapshot.mesh(snapshot.generation(), camera, || {
        !active(super::metric_long(generation))
    }) else {
        return [-1, 3, 0, 0, 0, 0];
    };
    if oracle.len() < 16 {
        return [-1, 4, 0, 0, 0, 0];
    }
    if output
        .try_reserve_exact(4 + mesh.payload_bytes().div_euclid(4))
        .is_err()
    {
        return [-1, 7, 0, 0, 0, 0];
    }
    output.resize(4, 0);
    let mut offset = 16;
    let mut quads = 0;
    let mut mismatches = 0;
    let mut first = -1;
    for (layer_index, layer) in mesh.layers().iter().enumerate() {
        if word(oracle, layer_index * 4) != Some(u32::try_from(layer.len()).unwrap_or(u32::MAX)) {
            return [-1, 5, super::metric_long(layer_index), 0, 0, 0];
        }
        output[layer_index] = i32::try_from(layer.len()).unwrap_or(i32::MAX);
        for quad in layer {
            quads += 1;
            for expected in quad.vertices.into_iter().flatten() {
                output.push(i32::from_ne_bytes(expected.to_ne_bytes()));
                let Some(actual) = word(oracle, offset) else {
                    return [-1, 6, 0, 0, 0, 0];
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
    [
        quads,
        mismatches,
        first,
        super::metric_long(started.elapsed().as_nanos()),
        super::metric_long(input_bytes),
        super::metric_long(mesh.heap_bytes() + output.capacity() * 4),
    ]
}

fn decode(source: &[u8]) -> Option<(BakedSnapshot, [f32; 3])> {
    let count = usize::try_from(word(source, 0)?).ok()?;
    if count > MAX_QUADS || source.len() < 32 + count * RECORD {
        return None;
    }
    let camera =
        std::array::from_fn(|axis| f32::from_bits(word(source, 4 + axis * 4).unwrap_or(u32::MAX)));
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
                u64::from(word(source, offset).unwrap_or_default())
                    | (u64::from(word(source, offset + 4).unwrap_or_default()) << 32)
            }),
            vertices: std::array::from_fn(|vertex| {
                std::array::from_fn(|item| {
                    word(source, base + 40 + (vertex * 7 + item) * 4).unwrap_or_default()
                })
            }),
            ambient_occlusion: std::array::from_fn(|item| {
                word(source, base + 152 + item * 4).unwrap_or_default()
            }),
            multipliers: std::array::from_fn(|vertex| {
                std::array::from_fn(|axis| {
                    word(source, base + 168 + (vertex * 3 + axis) * 4).unwrap_or_default()
                })
            }),
            combined_light: std::array::from_fn(|item| {
                word(source, base + 216 + item * 4).unwrap_or_default()
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
