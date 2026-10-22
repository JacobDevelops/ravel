use super::*;

fn quad(layer: RenderLayer, x: f32, sprite: u32) -> QuadInput {
    let vertex = [
        x.to_bits(),
        0,
        0,
        u32::MAX,
        0.25_f32.to_bits(),
        0.75_f32.to_bits(),
        0,
    ];
    QuadInput {
        state_key: 16,
        sprite,
        layer,
        vertices: [vertex; 4],
        offset: [0_f64.to_bits(); 3],
        tint: 0x80_ff_ff,
        ambient_occlusion: [0.5_f32.to_bits(); 4],
        multipliers: [[0.5_f32.to_bits(), 1_f32.to_bits(), 0.25_f32.to_bits()]; 4],
        combined_light: [0x00f0_0080; 4],
    }
}

#[test]
fn acknowledged_opaque_payload_is_freed_but_translucent_can_still_resort() {
    let source = vec![
        quad(RenderLayer::Solid, 0.0, 1),
        quad(RenderLayer::CutoutMipped, 0.0, 2),
        quad(RenderLayer::Cutout, 0.0, 3),
        quad(RenderLayer::Translucent, 1.0, 4),
        quad(RenderLayer::Translucent, 4.0, 5),
    ];
    let snapshot = BakedSnapshot::new(1, 1, source).expect("snapshot");
    let mut mesh = snapshot.mesh(1, [0.0; 3], || false).expect("mesh");
    let before = mesh.heap_bytes();
    mesh.release_uploaded_opaque();
    assert_eq!(
        mesh.layers()[..3].iter().map(Vec::capacity).sum::<usize>(),
        0
    );
    assert_eq!(mesh.heap_bytes(), 2 * size_of::<ShadedQuad>());
    assert_eq!(mesh.released_bytes() + mesh.heap_bytes(), before);
    assert_eq!(mesh.layers()[3][0].sprite, 5);
    mesh.resort(1, [8.0, 0.0, 0.0], || false)
        .expect("retained translucent");
    assert_eq!(mesh.layers()[3][0].sprite, 4);
    mesh.release_uploaded_opaque();
    assert_eq!(mesh.released_bytes() + mesh.heap_bytes(), before);
}

#[test]
fn emits_model_uv_shading_and_light_without_cube_substitution() {
    let mut source = quad(RenderLayer::Cutout, 0.125, 7);
    source.offset = [2.5_f64.to_bits(), 3_f64.to_bits(), 4_f64.to_bits()];
    let snapshot = BakedSnapshot::new(1, 1, vec![source]).expect("valid owning-thread data");
    let mesh = snapshot.mesh(1, [0.0; 3], || false).expect("mesh");
    let output = &mesh.layers()[2][0];
    assert_eq!(
        output.vertices[0],
        [
            2.625_f32.to_bits(),
            3_f32.to_bits(),
            4_f32.to_bits(),
            0xff3f_ff7f,
            0.25_f32.to_bits(),
            0.75_f32.to_bits(),
            0x00f0_0080
        ]
    );
    assert_eq!(output.sprite, 7);
    assert_eq!(output.tint, 0x80_ff_ff);
    assert_eq!(output.ambient_occlusion, [0.5_f32.to_bits(); 4]);
}

#[test]
fn atlas_and_mesh_generations_cannot_be_mixed() {
    assert!(matches!(
        BakedSnapshot::new(1, 2, Vec::new()),
        Err(BakedError::Generation)
    ));
    assert!(matches!(
        BakedSnapshot::new(0, 0, Vec::new()),
        Err(BakedError::Generation)
    ));
    let snapshot = BakedSnapshot::new(1, 1, Vec::new()).expect("snapshot");
    assert_eq!(
        snapshot.mesh(2, [0.0; 3], || false),
        Err(BakedError::Generation)
    );
}

#[test]
fn refuses_nonfinite_model_shading_camera_and_offsets() {
    let mut source = quad(RenderLayer::Solid, 0.0, 1);
    source.vertices[0][0] = f32::NAN.to_bits();
    assert!(matches!(
        BakedSnapshot::new(1, 1, vec![source]),
        Err(BakedError::Nonfinite)
    ));
    source.vertices[0][0] = 0;
    source.multipliers[1][2] = f32::INFINITY.to_bits();
    assert!(matches!(
        BakedSnapshot::new(1, 1, vec![source]),
        Err(BakedError::Nonfinite)
    ));
    source.multipliers[1][2] = 0;
    source.offset[2] = f64::NAN.to_bits();
    assert!(matches!(
        BakedSnapshot::new(1, 1, vec![source]),
        Err(BakedError::Nonfinite)
    ));
    let snapshot = BakedSnapshot::new(1, 1, Vec::new()).expect("snapshot");
    assert_eq!(
        snapshot.mesh(1, [f32::NAN; 3], || false),
        Err(BakedError::Nonfinite)
    );
}

#[test]
fn translucent_sort_is_far_to_near_and_preserves_equal_distance_order() {
    let quads = vec![
        quad(RenderLayer::Translucent, 1.0, 1),
        quad(RenderLayer::Translucent, 4.0, 2),
        quad(RenderLayer::Translucent, -4.0, 3),
        quad(RenderLayer::Solid, 2.0, 4),
    ];
    let snapshot = BakedSnapshot::new(1, 1, quads).expect("snapshot");
    let mut mesh = snapshot.mesh(1, [0.0; 3], || false).expect("mesh");
    assert_eq!(
        mesh.layers()[3]
            .iter()
            .map(|q| q.sprite)
            .collect::<Vec<_>>(),
        vec![2, 3, 1]
    );
    assert_eq!(mesh.layers()[0][0].sprite, 4);
    let allocations = mesh.layers().each_ref().map(Vec::as_ptr);
    mesh.resort(1, [10.0, 0.0, 0.0], || false).expect("resort");
    assert_eq!(
        mesh.layers()[3]
            .iter()
            .map(|q| q.sprite)
            .collect::<Vec<_>>(),
        vec![3, 1, 2]
    );
    mesh.resort(1, [0.0; 3], || false).expect("tied resort");
    assert_eq!(
        mesh.layers()[3]
            .iter()
            .map(|q| q.sprite)
            .collect::<Vec<_>>(),
        vec![3, 2, 1]
    );
    assert_eq!(mesh.layers().each_ref().map(Vec::as_ptr), allocations);
}

#[test]
fn cancellation_prevents_any_completed_output() {
    let snapshot =
        BakedSnapshot::new(1, 1, vec![quad(RenderLayer::Solid, 0.0, 1)]).expect("snapshot");
    assert_eq!(
        snapshot.mesh(1, [0.0; 3], || true),
        Err(BakedError::Cancelled)
    );
}

#[test]
fn zero_is_a_real_sprite_identity_and_empty_layers_are_not_filled() {
    let snapshot =
        BakedSnapshot::new(1, 1, vec![quad(RenderLayer::CutoutMipped, 0.0, 0)]).expect("snapshot");
    let mesh = snapshot.mesh(1, [0.0; 3], || false).expect("mesh");
    assert_eq!(
        mesh.layers().iter().map(Vec::len).collect::<Vec<_>>(),
        vec![0, 1, 0, 0]
    );
    assert_eq!(mesh.layers()[1][0].sprite, 0);
}

#[test]
fn position_rounding_matches_vanilla_float_add_after_offset_cast() {
    let mut source = quad(RenderLayer::Cutout, 0.05, 1);
    source.offset[0] = (-0.216_666_664_928_197_86_f64).to_bits();
    let snapshot = BakedSnapshot::new(1, 1, vec![source]).expect("snapshot");
    let mesh = snapshot.mesh(1, [0.0; 3], || false).expect("mesh");
    assert_eq!(
        mesh.layers()[2][0].vertices[0][0],
        (-0.166_666_67_f32).to_bits()
    );
}

#[test]
fn validation_rejects_output_overflow_before_admission() {
    let mut source = quad(RenderLayer::Solid, f32::MAX, 1);
    source.offset[0] = f64::from(f32::MAX).to_bits();
    assert!(matches!(
        BakedSnapshot::new(1, 1, vec![source]),
        Err(BakedError::Nonfinite)
    ));
    source.vertices[0][0] = 0;
    source.offset[0] = 1e39_f64.to_bits();
    assert!(matches!(
        BakedSnapshot::new(1, 1, vec![source]),
        Err(BakedError::Nonfinite)
    ));
}

#[test]
fn sorting_can_be_cancelled_and_never_reallocates_other_layers() {
    let source = (0_u16..128)
        .map(|i| quad(RenderLayer::Translucent, f32::from(i), u32::from(i)))
        .collect();
    let snapshot = BakedSnapshot::new(1, 1, source).expect("snapshot");
    let mut mesh = snapshot.mesh(1, [0.0; 3], || false).expect("mesh");
    let visits = std::cell::Cell::new(0);
    let result = mesh.resort(1, [0.0; 3], || {
        visits.set(visits.get() + 1);
        visits.get() > 150
    });
    assert_eq!(result, Err(BakedError::Cancelled));
    assert_eq!(visits.get(), 151);
}

#[test]
fn retained_input_length_and_spare_capacity_are_both_bounded() {
    let source = quad(RenderLayer::Solid, 0.0, 1);
    assert_eq!(
        BakedSnapshot::new(1, 1, vec![source; MAX_QUADS + 1]),
        Err(BakedError::Limit)
    );
    let mut spare = Vec::with_capacity(MAX_QUADS + 1);
    spare.push(source);
    assert_eq!(BakedSnapshot::new(1, 1, spare), Err(BakedError::Limit));
    let mut exact = Vec::with_capacity(MAX_QUADS);
    exact.push(source);
    let admitted = BakedSnapshot::new(1, 1, exact).expect("exact capacity bound");
    assert_eq!(admitted.heap_bytes(), MAX_QUADS * size_of::<QuadInput>());
}
