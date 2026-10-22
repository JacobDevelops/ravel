"""Offline renderer proof acceptance and refusal regression tests."""
import json
import math
from pathlib import Path
import struct
import tempfile
import unittest
import zlib

import renderer_proof
from harness_settings import SETTINGS, SCENES, client_options

def composite_fixture(directory: Path) -> None:
    arm = bytes((32, 64, 96, 255)) + struct.pack('<f', 0.5)
    omitted = bytes((0, 0, 0, 255)) + struct.pack('<f', 1.0)
    for phase in range(3):
        for designated in range(len(SCENES)):
            scene = 7 if designated >= 5 else designated if phase == 0 else phase + 4
            draws, vertices = (0, 0) if 5 <= designated < 8 else (1, 4)
            header = struct.pack('<8i', scene, designated, 1, 1, draws, vertices,
                                 5 if designated >= 8 else 4 if designated >= 5 else 3, vertices)
            sample = header + arm + arm + omitted
            if designated >= 5:
                if designated >= 8:
                    sample += struct.pack('<4i', *( (1, 0, 0, 0) if designated == 8 else (0, 0, 1, 0)))
                summary = ((2, 16, 0, 0), (3, 1, 5, 0), (8, 0, 0, 16),
                           (2, 16, 0, 0), (8, 0, 0, 16))[designated - 5]
                sample += struct.pack('<4i', *summary)
            (directory / f'composited-{phase}-{designated}.raw').write_bytes(sample)
            if designated == 3:
                for point, (dx, dz) in enumerate(renderer_proof.MOVING_PATH):
                    yaw = math.degrees(math.atan2(dx, -dz))
                    y = int(renderer_proof.validate_scene_settings()['scene.y']) + 6
                    metadata = struct.pack('<i5d', point, 104 + dx, y, 312 + dz, yaw, 30)
                    (directory / f'moving-{phase}-{point}.raw').write_bytes(sample + metadata)


def production_fixture(root: Path) -> None:
    metrics = [1] * 13
    metrics[8:11] = [10, 5, 3]
    (root / 'geometry-upload.json').write_text(json.dumps({'productionWorldDraw': metrics}))
    for kind in (1, 2):
        after = metrics.copy()
        after[10 + kind] += 1
        (root / f'production-fallback-{kind}.json').write_text(json.dumps(
            {'kind': kind, 'acknowledged': [1] * 7, 'before': metrics, 'after': after}))
    arm = bytes((32, 64, 96, 255)) + struct.pack('<f', 0.5)
    omitted = bytes((0, 0, 0, 255)) + struct.pack('<f', 1.0)
    for phase in range(3):
        for designated, point in [(value, -1) for value in (0, 1, 2, 4)] + [(3, value) for value in range(6)]:
            suffix = str(designated) if point < 0 else f'moving-{point}'
            scene = designated if phase == 0 else phase + 4
            raw = struct.pack('<8i', scene, designated, 1, 1, 3, 12, 3, 12) + arm + arm + omitted
            (root / f'production-{phase}-{suffix}.raw').write_bytes(raw)
            moving = ((-6, 0), (0, -5), (-1, 0), (6, 0), (0, 6), (8, 0))
            dx, dz = (0, 0) if point < 0 else moving[point]
            camera = [100 + designated, 206, 300, 0, 0] if point < 0 else [
                104 + dx, 206, 312 + dz, math.degrees(math.atan2(dx, -dz)), 30]
            identity = {'residency': [1, 2, 3, 3, 96, 200, 304], 'origin': [96, 200, 304],
                        'camera': camera, 'clientTick': 10 + point, 'drawCounts': [1, 1, 0, 1, 0, 1],
                        'sortCounters': [10 + point, 8 + point, 0], 'nativeBudget': [100, 0, 0, 0, 50, 80],
                        'sortCamera': camera[:3],
                        'designatedTranslucentDraws': 1,
                        'globalSortCounters': [100 + point, 80 + point, 0],
                        'glBudget': [100, 50, 80, 1, 0]}
            if point == 5:
                identity['residency'][1] = 4
                identity['sortCounters'] = [0, 0, 0]
            (root / f'production-{phase}-{suffix}.json').write_text(json.dumps(identity))


class ProductionFrameTests(unittest.TestCase):
    def test_requires_translucent_replacements_at_every_moving_stop(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            production_fixture(root)
            rows = renderer_proof.production_proof(root, 0)
            self.assertEqual(len(rows), 30)
            self.assertEqual(sum(row['point'] >= 0 for row in rows), 18)
            path = root / 'production-1-moving-2.json'
            initial = json.loads(path.read_text())
            for key, value in [('drawCounts', [1, 2, 0, 0, 0, 1]), ('sortCounters', [0, 0, 0]),
                               ('sortCounters', [10, 8, 0]),
                               ('globalSortCounters', [0, 0, 0]),
                               ('residency', [1, 3, 3, 3, 96, 200, 304]),
                               ('sortCamera', [104, 206, 312]),
                               ('designatedTranslucentDraws', 0),
                               ('camera', [104, 206, 312, -90, 30]), ('clientTick', 0)]:
                changed = dict(initial, **{key: value})
                path.write_text(json.dumps(changed))
                with self.subTest(key=key), self.assertRaises(RuntimeError):
                    renderer_proof.production_proof(root, 0)
            path.write_text(json.dumps(initial))
            raw = root / 'production-2-moving-4.raw'
            corrupted = bytearray(raw.read_bytes())
            corrupted[40] ^= 1
            raw.write_bytes(corrupted)
            with self.assertRaisesRegex(RuntimeError, 'fidelity failed'):
                renderer_proof.production_proof(root, 0)

    def test_first_stop_and_crossing_require_owned_translucent_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            production_fixture(root)
            for point, change in ((0, {'sortCamera': [104, 206, 306]}),
                                  (5, {'designatedTranslucentDraws': 0})):
                path = root / f'production-0-moving-{point}.json'
                initial = json.loads(path.read_text())
                path.write_text(json.dumps(initial | change))
                with self.subTest(point=point), self.assertRaises(RuntimeError):
                    renderer_proof.production_proof(root, 0)
                path.write_text(json.dumps(initial))
            self.assertEqual(len(renderer_proof.production_proof(root, 0)), 30)

    def test_crossing_requires_fresh_residency_and_preserves_global_history(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            production_fixture(root)
            path = root / 'production-1-moving-5.json'
            initial = json.loads(path.read_text())
            self.assertEqual(initial['sortCounters'], [0, 0, 0])
            for key, value in (
                    ('residency', [1, 2, 3, 3, 96, 200, 304]),
                    ('residency', [2, 4, 3, 3, 96, 200, 304]),
                    ('globalSortCounters', [1, 1, 0])):
                path.write_text(json.dumps(initial | {key: value}))
                with self.subTest(key=key), self.assertRaises(RuntimeError):
                    renderer_proof.production_proof(root, 0)
            path.write_text(json.dumps(initial))
            self.assertEqual(len(renderer_proof.production_proof(root, 0)), 30)


class CompositeFrameTests(unittest.TestCase):
    def test_recounts_complete_lifecycle_matrix_and_rejects_missing_or_stale_frames(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            composite_fixture(root)
            rows = renderer_proof.composited_proof(root, 0)
            self.assertEqual(len(rows), 45)
            self.assertTrue(all(row['rgbaMismatchPixels'] == row['depthMismatchPixels'] == 0 for row in rows))
            with self.assertRaisesRegex(RuntimeError, 'fresh composited'):
                renderer_proof.composited_proof(root, float('inf'))
            (root / 'composited-2-4.raw').unlink()
            with self.assertRaisesRegex(RuntimeError, 'fresh composited'):
                renderer_proof.composited_proof(root, 0)

    def test_never_accepts_different_empty_or_unsubstituted_frames(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for offset, replacement in ((40, bytes((255,))), (44, struct.pack('<f', 0.75)),
                                        (16, bytes(4)), (20, bytes(4)), (16, struct.pack('<i', 2)),
                                        (28, struct.pack('<i', 8)),
                                        (48, bytes((32, 64, 96, 255)) + struct.pack('<f', 0.5)),
                                        (36, struct.pack('<f', float('nan')))):
                composite_fixture(root)
                path = root / 'composited-0-0.raw'
                raw = bytearray(path.read_bytes())
                raw[offset:offset + len(replacement)] = replacement
                path.write_bytes(raw)
                with self.subTest(offset=offset), self.assertRaises(RuntimeError):
                    renderer_proof.composited_proof(root, 0)

    def test_rejects_missing_camera_motion_and_translucent_resident_draws(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for field, value in ((16, struct.pack('<i', 0)), (20, struct.pack('<i', 0)),
                                 (-44, struct.pack('<i', 4)),
                                 (-40, struct.pack('<d', 104)), (-8, struct.pack('<d', 20)),
                                 (-32, struct.pack('<d', float('inf')))):
                composite_fixture(root)
                path = root / 'moving-0-0.raw'
                raw = bytearray(path.read_bytes())
                offset = field if field >= 0 else len(raw) + field
                raw[offset:offset + len(value)] = value
                path.write_bytes(raw)
                with self.subTest(field=field), self.assertRaises(RuntimeError):
                    renderer_proof.composited_proof(root, 0)
            composite_fixture(root)
            (root / 'moving-2-4.raw').unlink()
            with self.assertRaisesRegex(RuntimeError, 'fresh moving'):
                renderer_proof.composited_proof(root, 0)

    def test_fluid_sections_require_vanilla_routing_and_unchanged_strict_oracle(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            composite_fixture(root)
            rows = renderer_proof.composited_proof(root, 0)
            fluids = [row for row in rows if row['fluidVanillaFallback'] and row['substitutedDraws'] == 0]
            self.assertEqual(len(fluids), 9)
            self.assertEqual({row['designated'] for row in fluids}, {'still-water', 'flowing-water', 'lava'})
            for designated in range(5, 8):
                path = root / f'composited-0-{designated}.raw'
                original = path.read_bytes()
                for offset, replacement in ((16, struct.pack('<i', 1)), (20, struct.pack('<i', 4)),
                                            (40, bytes((255,))), (44, struct.pack('<f', 0.75)),
                                            (48, original[32:40]), (len(original) - 16, bytes(16))):
                    raw = bytearray(original)
                    raw[offset:offset + len(replacement)] = replacement
                    path.write_bytes(raw)
                    with self.subTest(designated=designated, offset=offset), self.assertRaises(RuntimeError):
                        renderer_proof.composited_proof(root, 0)
                    path.write_bytes(original)
                path.unlink()
                with self.assertRaisesRegex(RuntimeError, 'fresh composited'):
                    renderer_proof.composited_proof(root, 0)
                path.write_bytes(original)

    def test_mixed_fluid_layers_are_exact_across_all_lifecycles(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            composite_fixture(root)
            rows = renderer_proof.composited_proof(root, 0)
            mixed = [row for row in rows if row['designated'] in ('water-stone', 'lava-cutout-plants')]
            self.assertEqual(len(mixed), 6)
            for row in mixed:
                expected = [1, 0, 0, 0] if row['designated'] == 'water-stone' else [0, 0, 1, 0]
                self.assertEqual(row['replacedLayerDraws'], expected)
                self.assertEqual(row['substitutedDraws'], 1)
            for designated in (8, 9):
                path = root / f'composited-0-{designated}.raw'
                original = path.read_bytes()
                for layer in range(4):
                    raw = bytearray(original)
                    struct.pack_into('<i', raw, len(raw) - 32 + layer * 4, 2)
                    path.write_bytes(raw)
                    with self.subTest(designated=designated, layer=layer), self.assertRaises(RuntimeError):
                        renderer_proof.composited_proof(root, 0)
                path.write_bytes(original)

    def test_rejects_camera_path_shifted_away_from_section_interior(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for field, delta in ((-40, 1), (-32, 16)):
                composite_fixture(root)
                for path in root.glob('moving-*.raw'):
                    raw = bytearray(path.read_bytes())
                    offset = len(raw) + field
                    value = struct.unpack_from('<d', raw, offset)[0]
                    struct.pack_into('<d', raw, offset, value + delta)
                    path.write_bytes(raw)
                with self.subTest(field=field), self.assertRaisesRegex(RuntimeError, 'camera path'):
                    renderer_proof.composited_proof(root, 0)



class SceneSettingsTests(unittest.TestCase):
    def test_pinned_client_options_and_visible_scene_offsets(self):
        values = renderer_proof.validate_scene_settings()
        options = client_options()
        self.assertEqual({name: options[name] for name in
                          ('renderDistance', 'fancyGraphics', 'maxFps', 'enableVsync', 'guiScale')},
                         {'renderDistance': '2', 'fancyGraphics': 'false', 'maxFps': '60',
                          'enableVsync': 'false', 'guiScale': '2'})
        self.assertEqual([values[f'scene.{index}.name'] for index in range(len(SCENES))], list(SCENES))
        self.assertTrue(all(max(map(abs, map(int, values[f'scene.{index}.chunk'].split(',')))) <= 2
                            for index in range(len(SCENES))))

    def test_rejects_outside_distance_before_placement(self):
        for offset in ('3,0', '0,-3', '-3,3'):
            with self.subTest(offset=offset), tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / 'settings.properties'
                path.write_text(SETTINGS.read_text().replace('scene.0.chunk=1,-1', 'scene.0.chunk=' + offset))
                with self.assertRaisesRegex(ValueError, 'outside render distance'):
                    renderer_proof.validate_scene_settings(path)

    def test_rejects_overlapping_and_malformed_scene_definitions(self):
        for original, replacement in (('scene.0.chunk=1,-1', 'scene.0.chunk=0,1'),
                                      ('scene.0.blocks=minecraft:stone 0,minecraft:planks 0',
                                       'scene.0.blocks=minecraft:stone 0'),
                                      ('scene.0.name=opaque', 'scene.0.name=other')):
            with self.subTest(replacement=replacement), tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / 'settings.properties'
                path.write_text(SETTINGS.read_text().replace(original, replacement))
                with self.assertRaises(ValueError):
                    renderer_proof.validate_scene_settings(path)


def renderer_proof_fixture() -> renderer_proof.Proof:
    return {'beforeReload': 2, 'afterReload': 3, 'afterUnload': 4,
            'reopened': 4, 'finalUnload': 5, 'staleRejected': True,
            'ownerThread': 1, 'workerObserved': True,
            'injectedStaleObservationRejected': True,
            'seams': {name: 1 for name in renderer_proof.RENDERER_SEAMS},
            'snapshot': {
                'captures': 2, 'seed': 8675309, 'edge': 18,
                'comparison': 'decoded-vs-Java-copied-post-compile-region',
                'light': 'raw-stored-sky-and-block-not-combined-mesh-light',
                'oracle': 'coordinate-only-same-mappings',
                'modelKeys': 'state-derived-not-baked-model-identities',
                'maxConcurrentCaptures': 1, 'timingSampleLimit': 4096,
                'javaBatchBytes': 34992, 'retainedStaleProofBytes': 34992,
                'maxJavaBatchPayloadBytes': 69984,
                'maxNativeCopyBytes': 34992, 'maxNativeScratchBytes': 80536,
                'geometry': 'captured-game-isOpaqueCube-and-isFullCube', 'fullOpaqueCells': 1,
                'cells': 11664, 'borderCells': 3472, 'mismatches': 0,
                'nonAirCells': 1, 'adjacentVariation': 1, 'workerCaptures': 1,
                'oracleChecks': 125, 'oracleMismatches': 0, 'oracleBorderChecks': 98,
                'oracleRetryCells': 0, 'oracleRetryCandidates': 0,
                'staleCaptureRejections': 1, 'lastThread': 2, 'lastGeneration': 4,
                'rejected': 0, 'skipped': 1, 'maxBytes': 100, 'totalBytes': 200,
                'p50Nanos': 10, 'p95Nanos': 20, 'p99Nanos': 30,
                'maxNanos': 40, 'totalNanos': 80},
            'mesh': mesh_proof_fixture()}


def mesh_proof_fixture() -> renderer_proof.Proof:
    return {'geometry': 'game-isOpaqueCube-and-isFullCube-per-state-air-key-zero-empty',
            'fidelity': 'geometry-only-no-texture-ao-or-model-parity',
            'upload': 'none-native-owned-until-drained-then-discarded',
            'denominator': 'admitted-native-geometry-jobs-not-vanilla-compile-tasks',
            'frameInterval': 'observational-stress-window-frame-interval-not-mesh-attribution',
            'cancellationEvidence': 'actual-retired-before-drain',
            'timingSampleLimit': 4096, 'keySlots': 16, 'recordWidth': 11,
            'maxRecordsPerPollLimit': 8, 'statsWidth': 18,
            'stress': {'seed': 8675309, 'teleports': 6, 'stepBlocks': 128, 'minimumDrainedPerStep': 32},
            'stalePollRejected': True,
            'closeDrain': {'stopped': 1, 'outstanding': 0, 'queued': 0, 'running': 0,
                           'completed': 0, 'reservedBytes': 0, 'draining': 0, 'workerFailure': 0,
                           'javaKeysHeld': 0},
            'unloadDrain': {'generation': 6, 'retiredPoolGeneration': 6, 'drainChecks': 1},
            'lifecyclesWithoutRetiredPool': 2, 'retiredUnadmitted': 1, 'lastRetiredPoolGeneration': 6,
            'submitted': 300, 'refused': 2, 'tableFull': 0, 'drained': 290,
            'complete': 200, 'incomplete': 90, 'errors': 0,
            'unsupportedKeys': 120, 'maxUnsupportedKeys': 5,
            'vertices': 4000, 'indices': 6000, 'outputBytes': 9000, 'maxOutputBytes': 900,
            'allocations': 800, 'maxAllocations': 5, 'nonEmptyMeshes': 150, 'occludingMeshes': 40,
            'rejectedRecords': 0, 'retiredBeforeDrain': 3, 'stalePolls': 1, 'polls': 500,
            'maxRecordsPerPoll': 8,
            'firstQueueNanos': 5, 'firstMeshNanos': 50, 'firstDrainNanos': 500, 'firstCaptureNanos': 70,
            'maxQueueNanos': 60, 'maxMeshNanos': 400, 'maxDrainNanos': 5000, 'maxCaptureNanos': 700,
            'drainChecks': 3, 'drainFailures': 0,
            'maxOutstanding': 8, 'maxReservedBytes': 4000,
            'maxOutstandingLimit': 8, 'maxReservedBytesLimit': 8000,
            'frameSamples': 300, 'maxFrameNanos': 900, 'firstFiveSecondsMaxFrameNanos': 800,
            'stressSubmitted': 250, 'stressDrained': 240, 'maxTableOccupancy': 9,
            'injectedStaleRecordRejections': 1,
            'captureFailures': 0, 'lastFailureCode': 0, 'lastFailureDetailA': 0,
            'lastFailureDetailB': 0, 'lastFailureDetailC': 0, 'workerFailures': 0,
            'queueP50Nanos': 10, 'queueP95Nanos': 20, 'queueP99Nanos': 30,
            'meshP50Nanos': 100, 'meshP95Nanos': 200, 'meshP99Nanos': 300,
            'drainP50Nanos': 1000, 'drainP95Nanos': 2000, 'drainP99Nanos': 3000,
            'frameP50Nanos': 100, 'frameP95Nanos': 200, 'frameP99Nanos': 300}


def geometry_proof_fixture(directory: Path) -> renderer_proof.Proof:
    composite_fixture(directory)
    names = ('copiedMeshes copiedBytes copyNanos uploads uploadBytes uploadNanos residentBytes '
             'maxResidentBytes stagingBytes maxStagingBytes submissions submissionNanos refusedMeshes '
             'cleanups liveBuffers liveFramebuffers referenceMismatchPixels vanillaComparisons '
             'vanillaRgbaMismatchPixels vanillaCoverageMismatchPixels vanillaDepthMismatchPixels generation key').split()
    metrics = dict.fromkeys(names, 1)
    metrics.update(referenceMismatchPixels=0, refusedMeshes=0, vanillaComparisons=1,
                   vanillaRgbaMismatchPixels=1, vanillaCoverageMismatchPixels=1, vanillaDepthMismatchPixels=1)
    final = metrics | dict.fromkeys(('residentBytes', 'stagingBytes', 'liveBuffers', 'liveFramebuffers'), 0)
    return {
        'selection': 'geometry-proof', 'offDefault': True, 'worldSubstitution': False,
        'fidelity': 'not-proven', 'reference': 'self-reference-not-vanilla-oracle',
        'width': 64, 'height': 64, 'rowOrder': 'bottom-up',
        'vanillaScope': 'published-SOLID-only-section-position-coverage-depth',
        'vanillaComparison': {'status': 1, 'generation': 1, 'key': 1, 'vanillaVertices': 4,
            'vanillaLayers': 1, 'rgbaMismatchPixels': 1, 'coverageMismatchPixels': 1,
            'depthMismatchPixels': 1, 'totalPixels': 4096},
        'metrics': metrics, 'info': ['geometry-proof', 'OpenGL15',
            'geometric-only-no-uv-sprite-tint-ao-baked-light-world-replacement-refused'],
        'shadedNativeResidency': [0, 0, 200, 2, 2] + [0] * 7,
        'shadedGlResidency': [0, 2_752_512, 3, 2, 1] + [0] * 9 + [1, 0],
        'shadedNativeLive': [100, 0, 200, 2, 0, 1, 2, 1, 1, -16, 32, 48],
        'shadedGlLive': [2_752_512, 2_752_512, 3, 2, 0, 3, 1, 2, 1, 1, -16, 32, 48, 1, 1, 0],
        'shadedNativeBudget': [16 * 1024 * 1024, 0, 0, 0, 0, 200],
        'shadedNativeBudgetLive': [16 * 1024 * 1024, 1, 0, 0, 150, 200],
        'finalMetrics': final, 'baked': baked_proof_fixture(directory)}


def baked_sample(scene: int, designated: int, oracle_red: int = 10) -> bytes:
    pixels = renderer_proof.PROOF_IMAGE_PIXELS
    emitted, oracle = bytearray(pixels * 4), bytearray(pixels * 4)
    emitted[0], emitted[3], oracle[0], oracle[3] = 10, 255, oracle_red, 255
    depth = struct.pack(f'<{pixels}f', 0.5, *([1.0] * (pixels - 1)))
    layer = (0, 1, 2, 3, 0)[designated] if 0 <= designated < 5 else 0
    return struct.pack('<4i', scene, layer, 4, designated) + bytes(emitted) + depth + bytes(oracle) + depth


def baked_proof_fixture(directory: Path) -> renderer_proof.Proof:
    goldens = []
    for scene, name in enumerate(renderer_proof.BAKED_SCENES[:-1]):
        sections = [scene] if scene < renderer_proof.BAKED_DESIGNATED else list(range(5))
        files = [f'baked-golden-{scene}-{index}.raw' for index in range(len(sections))]
        for file, section in zip(files, sections):
            (directory / file).write_bytes(baked_sample(scene, section))
        count = len(sections)
        goldens.append({'scene': name, 'status': 'measured', 'reason': renderer_proof.BAKED_MEASURED,
                        'values': [count, 0, 0, 0, count, count, 4 * count], 'readbacks': files})
    goldens.append({'scene': 'translucent-fluid', 'status': 'unsupported',
                    'reason': renderer_proof.BAKED_FLUID, 'values': [0] * 7, 'readbacks': []})
    return {'comparison': 'Rust-emission-vs-final-vanilla-BLOCK-words', 'drawable': True,
            'worldSubstitution': False, 'maxQuadsPerSection': 8192, 'comparedSections': 4,
            'busySections': 0, 'unsupportedSections': 2, 'staleSections': 0, 'quads': 1,
            'mismatchedWords': 0, 'firstMismatchedWord': -1, 'cpuNanos': 1,
            'maxInputBytes': 1000, 'maxOutputBytes': 1000, 'animatedSpriteObservations': 1,
            'resourceGeneration': 1, 'atlasGeneration': 1, 'glComparedSections': 4,
            'glStaleSections': 0, 'glUnsupportedSections': 0,
            'translucentSorts': 1, 'translucentUploads': 1, 'translucentRefusals': 0,
            'glUploadedBytes': 224, 'glPeakBufferBytes': 112, 'glUploadNanos': 1, 'glComparisonNanos': 1,
            **dict.fromkeys(renderer_proof.BAKED_REFUSALS, 0),
            'refusedUnaccountedVertices': 1, 'refusedQuadBudget': 1,
            'controlledDeferredSections': 3, 'world': [16, 0, 0, 0, 16, 16, 64], 'goldens': goldens}


def png_chunk(kind: bytes, data: bytes) -> bytes:
    return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))


class RendererProofTests(unittest.TestCase):
    def test_normal_world_requires_fresh_capture_files_and_strict_parity(self):
        with tempfile.TemporaryDirectory() as raw:
            directory = Path(raw)
            production_fixture(directory)
            (directory / 'production-2-moving-4.raw').unlink()
            with self.assertRaisesRegex(RuntimeError, 'fresh normal world'):
                renderer_proof.production_proof(directory, 0)
            production_fixture(directory)
            rows = renderer_proof.production_proof(directory, 0)
            self.assertEqual(len(rows), 30)
            self.assertEqual(rows[0]['substitutedDraws'], 3)
            metadata = directory / 'production-0-0.json'
            original = json.loads(metadata.read_text())
            metadata.write_text(json.dumps(original | {'camera': [101, 206, 300, 0, 0]}))
            with self.assertRaisesRegex(RuntimeError, 'distinct camera positions'):
                renderer_proof.production_proof(directory, 0)
            metadata.write_text(json.dumps(original))
            metadata.write_text(json.dumps(original | {'nativeBudget': [100, 2, 0, 0, 50, 101]}))
            with self.assertRaisesRegex(RuntimeError, 'budget mismatch'):
                renderer_proof.production_proof(directory, 0)
            metadata.write_text(json.dumps(original))
            path = directory / 'production-0-0.raw'
            changed = bytearray(path.read_bytes())
            changed[40] ^= 1
            path.write_bytes(changed)
            with self.assertRaisesRegex(RuntimeError, 'Normal world fidelity failed'):
                renderer_proof.production_proof(directory, 0)

    def test_baked_requires_independent_atlas_and_observable_mismatch(self):
        with tempfile.TemporaryDirectory() as raw:
            directory = Path(raw)
            proof = baked_proof_fixture(directory)
            renderer_proof.baked_proof(proof, directory, 0)
            for field, invalid in (('atlasGeneration', 2), ('resourceGeneration', 0),
                                   ('firstMismatchedWord', 0), ('mismatchedWords', 1),
                                   ('drawable', False), ('comparedSections', 0),
                                   ('glComparedSections', 0), ('glStaleSections', -1),
                                   ('translucentSorts', 0), ('translucentUploads', 0),
                                   ('translucentRefusals', 1),
                                   ('glPeakBufferBytes', 8192 * 112 + 112),
                                   ('glUploadedBytes', 225), ('glUploadNanos', 0),
                                   ('maxInputBytes', 8_000_001), ('maxOutputBytes', 4_000_001),
                                   ('quads', True), ('goldens', []), ('refusedRustDecode', 1),
                                   ('refusedQuadBudget', 0), ('world', [3, 0, 0, 0, 3, 3, 12]),
                                   ('world', [17, 0, 0, 0, 16, 16, 64]), ('world', [16, 0, 0])):
                with self.subTest(field=field), self.assertRaises(RuntimeError):
                    renderer_proof.baked_proof(proof | {field: invalid}, directory, 0)
            with self.assertRaises(RuntimeError):
                renderer_proof.baked_proof(proof, directory, float('inf'))

    def test_baked_goldens_are_recounted_from_raw_readbacks(self):
        for change in ({'status': 'passed'}, {'status': 'unsupported'}, {'reason': ''},
                       {'values': [1, 1, 0, 0, 1, 1, 4]}, {'values': [1, 0, 0, 0, 1, 1, 8]},
                       {'values': [2, 0, 0, 0, 2, 2, 8]}, {'scene': 'different'}, {'readbacks': []},
                       {'readbacks': ['baked-golden-1-0.raw']}):
            with self.subTest(change=change), tempfile.TemporaryDirectory() as raw:
                directory = Path(raw)
                proof = baked_proof_fixture(directory)
                proof['goldens'][0].update(change)
                with self.assertRaises(RuntimeError):
                    renderer_proof.baked_proof(proof, directory, 0)
        with tempfile.TemporaryDirectory() as raw:
            directory = Path(raw)
            proof = baked_proof_fixture(directory)
            (directory / 'baked-golden-3-0.raw').write_bytes(baked_sample(3, 3, oracle_red=11))
            with self.assertRaises(RuntimeError):
                renderer_proof.baked_proof(proof, directory, 0)
            proof['goldens'][3]['values'][1] = proof['world'][1] = 1
            with self.assertRaises(RuntimeError):
                renderer_proof.baked_proof(proof, directory, 0)
            proof['goldens'][3]['values'][1] = proof['world'][1] = 0
            for sample in (baked_sample(4, 3), baked_sample(3, 3)[:-1], baked_sample(3, 5),
                           struct.pack('<i', 3) + struct.pack('<i', 0) + baked_sample(3, 3)[8:]):
                (directory / 'baked-golden-3-0.raw').write_bytes(sample)
                with self.subTest(sample=len(sample)), self.assertRaises(RuntimeError):
                    renderer_proof.baked_proof(proof, directory, 0)

    def test_baked_fluid_row_stays_unsupported_and_sections_are_complete(self):
        with tempfile.TemporaryDirectory() as raw:
            directory = Path(raw)
            proof = baked_proof_fixture(directory)
            proof['goldens'][7].update(status='measured')
            with self.assertRaises(RuntimeError):
                renderer_proof.baked_proof(proof, directory, 0)
            proof = baked_proof_fixture(directory)
            (directory / 'baked-golden-5-4.raw').write_bytes(baked_sample(5, 3))
            with self.assertRaises(RuntimeError):
                renderer_proof.baked_proof(proof, directory, 0)

    def test_geometry_selection_requires_fresh_separate_proof(self):
        proof = renderer_proof_fixture()
        proof['mesh']['upload'] = 'bounded-native-wire-java-owned-offscreen-gl'
        renderer_proof.mesh_proof(proof['mesh'], 'geometry-proof')
        with self.assertRaises(RuntimeError):
            renderer_proof.mesh_proof(proof['mesh'])
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'renderer.json'
            path.write_text(json.dumps(proof))
            with self.assertRaises(RuntimeError):
                renderer_proof.renderer_proof(path, 0)
            with self.assertRaises(RuntimeError):
                renderer_proof.renderer_proof(path, 0, 'geometry-proof')

    def assert_empty_geometry_rejected(
        self, path: Path, proof: renderer_proof.Proof, native: bytes, vanilla: bytes
    ) -> None:
        for empty in ('native', 'vanilla'):
            with self.subTest(emptyImage=empty):
                (path.parent / f'geometry-{empty}.rgba').write_bytes(bytes(16384))
                empty_proof = proof
                if empty == 'native':
                    (path.parent / 'geometry-reference.rgba').write_bytes(bytes(16384))
                    empty_proof = proof | {
                        'metrics': proof['metrics'] | {
                            'vanillaRgbaMismatchPixels': 2, 'vanillaCoverageMismatchPixels': 2},
                        'vanillaComparison': proof['vanillaComparison'] | {
                            'rgbaMismatchPixels': 2, 'coverageMismatchPixels': 2}}
                path.write_text(json.dumps(empty_proof))
                with self.assertRaises(RuntimeError):
                    renderer_proof.geometry_proof(path, 0)
                for name in ('native', 'reference'):
                    (path.parent / f'geometry-{name}.rgba').write_bytes(native)
                (path.parent / 'geometry-vanilla.rgba').write_bytes(vanilla)

    def test_geometry_self_reference_mismatch_bounds_and_cleanup_are_strict(self):
        with tempfile.TemporaryDirectory() as directory:
            proof = geometry_proof_fixture(Path(directory))
            metrics, final = proof['metrics'], proof['finalMetrics']
            path = Path(directory) / 'geometry-upload.json'
            png = (b'\x89PNG\r\n\x1a\n'
                   + png_chunk(b'IHDR', struct.pack('>IIBBBBB', 64, 64, 8, 6, 0, 0, 0))
                   + png_chunk(b'IDAT', zlib.compress(bytes(64 * (1 + 64 * 4))))
                   + png_chunk(b'IEND', b''))
            native = bytes([1]) + bytes(16383)
            vanilla = bytes([1, 0, 0, 0, 1]) + bytes(16379)
            for name in ('native', 'reference', 'vanilla'):
                (path.parent / f'geometry-{name}.rgba').write_bytes(native)
                (path.parent / f'geometry-{name}.png').write_bytes(png)
            (path.parent / 'geometry-vanilla.rgba').write_bytes(vanilla)
            path.write_text(json.dumps(proof))
            self.assertEqual(renderer_proof.geometry_proof(path, 0), proof)
            self.assert_empty_geometry_rejected(path, proof, native, vanilla)
            for changes in ({'referenceMismatchPixels': 1}, {'maxResidentBytes': 1179649},
                            {'maxStagingBytes': 1212417}, {'uploads': 0}, {'submissions': 0}):
                path.write_text(json.dumps(proof | {'metrics': metrics | changes}))
                with self.assertRaises(RuntimeError):
                    renderer_proof.geometry_proof(path, 0)
            path.write_text(json.dumps(proof | {'finalMetrics': final | {'liveBuffers': 1}}))
            with self.assertRaises(RuntimeError):
                renderer_proof.geometry_proof(path, 0)
            path.write_text(json.dumps(proof | {'vanillaComparison': proof['vanillaComparison'] | {'status': 0}}))
            with self.assertRaises(RuntimeError):
                renderer_proof.geometry_proof(path, 0)
            path.write_text(json.dumps(proof | {'vanillaComparison': proof['vanillaComparison'] | {'vanillaLayers': 2}}))
            with self.assertRaises(RuntimeError):
                renderer_proof.geometry_proof(path, 0)
            path.write_text(json.dumps(proof | {'finalMetrics': final | {'maxResidentBytes': 1179649}}))
            with self.assertRaises(RuntimeError):
                renderer_proof.geometry_proof(path, 0)
            path.write_text(json.dumps(proof))
            (path.parent / 'geometry-native.rgba').write_bytes(bytes([2]) + bytes(16383))
            with self.assertRaises(RuntimeError):
                renderer_proof.geometry_proof(path, 0)
            with self.assertRaises(RuntimeError):
                renderer_proof.geometry_proof(path, float('inf'))
            (path.parent / 'geometry-native.rgba').write_bytes(native)
            (path.parent / 'geometry-vanilla.rgba').unlink()
            with self.assertRaises(RuntimeError):
                renderer_proof.geometry_proof(path, 0)


    def test_shaded_residency_rejects_cross_generation_keys_bounds_and_live_leaks(self):
        with tempfile.TemporaryDirectory() as directory:
            proof = geometry_proof_fixture(Path(directory))
            renderer_proof.shaded_residency_proof(proof)
            for name, index, value in (('shadedNativeResidency', 0, 1),
                    ('shadedNativeResidency', 1, 1), ('shadedNativeResidency', 2, 16 * 1024 * 1024 + 1),
                    ('shadedNativeResidency', 3, 0), ('shadedGlResidency', 5, 2),
                    ('shadedGlResidency', 14, 0), ('shadedGlLive', 6, 2), ('shadedGlLive', 7, 3),
                    ('shadedGlLive', 8, 2), ('shadedGlLive', 9, 2), ('shadedGlLive', 10, 1),
                    ('shadedGlLive', 13, 0), ('shadedNativeLive', 1, -1)):
                changed = proof | {name: proof[name].copy()}
                changed[name][index] = value
                with self.subTest(name=name, index=index), self.assertRaises(RuntimeError):
                    renderer_proof.shaded_residency_proof(changed)
            for name in ('shadedNativeResidency', 'shadedGlResidency', 'shadedNativeLive', 'shadedGlLive'):
                with self.subTest(name=name), self.assertRaises(RuntimeError):
                    renderer_proof.shaded_residency_proof(proof | {name: []})

    def test_production_residency_forbids_offscreen_idle_submissions(self):
        with tempfile.TemporaryDirectory() as directory:
            proof = geometry_proof_fixture(Path(directory))
            with self.assertRaises(RuntimeError):
                renderer_proof.shaded_residency_proof(proof, 'resident-world')
            proof['shadedGlResidency'][14] = proof['shadedGlLive'][14] = 0
            renderer_proof.shaded_residency_proof(proof, 'resident-world')
            with self.assertRaises(RuntimeError):
                renderer_proof.shaded_residency_proof(proof, 'geometry-proof')

    def test_generations_seams_and_freshness(self):
        proof = renderer_proof_fixture()
        proof['snapshot'].update(oracleRetryCells=7, oracleRetryCandidates=2)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'renderer.json'
            path.write_text(json.dumps(proof))
            self.assertEqual(renderer_proof.renderer_proof(path, 0), proof)
            for change in ({'afterReload': 2}, {'afterUnload': 3}, {'reopened': 3},
                           {'finalUnload': 4}, {'staleRejected': False},
                           {'workerObserved': False}, {'ownerThread': 0},
                           {'injectedStaleObservationRejected': False},
                           {'seams': {'frame': 1}}):
                path.write_text(json.dumps(proof | change))
                with self.assertRaises(RuntimeError):
                    renderer_proof.renderer_proof(path, 0)
            path.write_text(json.dumps(proof))
            with self.assertRaises(RuntimeError):
                renderer_proof.renderer_proof(path, float('inf'))

    def test_bounded_vanilla_roundtrip_and_timing(self):
        proof = renderer_proof_fixture()
        snapshot = proof['snapshot']
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'renderer.json'
            for key, invalid in (('captures', 0), ('seed', 1), ('edge', 16),
                                 ('comparison', 'receipt-only'), ('modelKeys', 'baked'),
                                 ('maxConcurrentCaptures', 2), ('timingSampleLimit', 0),
                                 ('javaBatchBytes', 29160), ('retainedStaleProofBytes', 29160),
                                 ('maxJavaBatchPayloadBytes', 34992),
                                 ('maxNativeCopyBytes', 29160), ('maxNativeScratchBytes', 0),
                                 ('geometry', 'stone-only'), ('fullOpaqueCells', 0),
                                 ('cells', 5832), ('borderCells', 0), ('mismatches', 1),
                                 ('nonAirCells', 0), ('adjacentVariation', 0),
                                 ('workerCaptures', 0), ('staleCaptureRejections', 0),
                                 ('oracleChecks', 0), ('oracleChecks', 126), ('oracleMismatches', 1),
                                 ('oracleBorderChecks', 0), ('light', 'combined-mesh-light'),
                                 ('oracleRetryCells', -1), ('oracleRetryCandidates', -1),
                                 ('oracleRetryCells', 1), ('oracleRetryCandidates', 1),
                                 ('oracle', 'independent-mapping-oracle'),
                                 ('lastThread', 0), ('lastGeneration', 0),
                                 ('rejected', -1), ('skipped', -1), ('maxBytes', 0),
                                 ('totalBytes', 1), ('p50Nanos', 0), ('p95Nanos', 9),
                                 ('p99Nanos', 19), ('maxNanos', 29), ('totalNanos', 39)):
                with self.subTest(snapshotField=key):
                    path.write_text(json.dumps(proof | {'snapshot': snapshot | {key: invalid}}))
                    with self.assertRaises(RuntimeError):
                        renderer_proof.renderer_proof(path, 0)

    def test_mesh_admission_drain_and_bounded_timing(self):
        proof = renderer_proof_fixture()
        mesh = proof['mesh']
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'renderer.json'
            path.write_text(json.dumps(proof | {'mesh': mesh | {
                'retiredBeforeDrain': 0, 'cancellationEvidence': 'deterministic-harness-only'}}))
            renderer_proof.renderer_proof(path, 0)
            for key, invalid in (('geometry', 'stone-only'), ('fidelity', 'parity'),
                                 ('upload', 'gl'), ('denominator', 'compile-tasks'),
                                 ('frameInterval', 'mesh-cost'), ('cancellationEvidence', 'none'),
                                 ('retiredBeforeDrain', 0), ('timingSampleLimit', 0),
                                 ('keySlots', 8), ('recordWidth', 12), ('maxRecordsPerPollLimit', 9),
                                 ('statsWidth', 15), ('stress', {'seed': 1}),
                                 ('stalePollRejected', False),
                                 ('closeDrain', mesh['closeDrain'] | {'outstanding': 1}),
                                 ('closeDrain', mesh['closeDrain'] | {'stopped': 0}),
                                 ('submitted', 0), ('drained', 0), ('drained', 301),
                                 ('complete', 199), ('errors', 1), ('incomplete', 91),
                                 ('unsupportedKeys', 89), ('maxUnsupportedKeys', 121),
                                 ('vertices', 0), ('vertices', 4002), ('indices', 6001),
                                 ('outputBytes', 0), ('maxOutputBytes', 9001), ('allocations', 4),
                                 ('nonEmptyMeshes', 0), ('rejectedRecords', 1), ('polls', 0),
                                 ('maxRecordsPerPoll', 9), ('firstQueueNanos', -1),
                                 ('firstMeshNanos', 0), ('firstDrainNanos', 5001),
                                 ('firstCaptureNanos', 701), ('maxQueueNanos', 29),
                                 ('drainChecks', 0), ('drainFailures', 1),
                                 ('maxOutstanding', 0), ('maxOutstanding', 9),
                                 ('maxOutstandingLimit', 16), ('maxReservedBytes', 8001),
                                 ('frameSamples', 0), ('firstFiveSecondsMaxFrameNanos', 901),
                                 ('stressSubmitted', 0), ('stressDrained', 191),
                                 ('maxTableOccupancy', 17), ('tableFull', -1), ('refused', -1),
                                 ('injectedStaleRecordRejections', 0),
                                 ('captureFailures', 1), ('lastFailureCode', 5), ('workerFailures', 1),
                                 ('closeDrain', mesh['closeDrain'] | {'workerFailure': 1}),
                                 ('closeDrain', mesh['closeDrain'] | {'javaKeysHeld': 1}),
                                 ('closeDrain', mesh['closeDrain'] | {'draining': 1}),
                                 ('unloadDrain', mesh['unloadDrain'] | {'drainChecks': 0}),
                                 ('unloadDrain', mesh['unloadDrain'] | {'drainChecks': 4}),
                                 ('unloadDrain', mesh['unloadDrain'] | {'retiredPoolGeneration': 5}),
                                 ('unloadDrain', mesh['unloadDrain'] | {'generation': 0,
                                                                        'retiredPoolGeneration': 0}),
                                 ('lastRetiredPoolGeneration', 5), ('lifecyclesWithoutRetiredPool', -1),
                                 ('retiredUnadmitted', -1), ('retiredBeforeDrain', 11),
                                 ('occludingMeshes', 0), ('occludingMeshes', 291),
                                 ('meshP50Nanos', 0), ('meshP95Nanos', 99), ('drainP99Nanos', 5001),
                                 ('frameP99Nanos', 901)):
                with self.subTest(meshField=key):
                    path.write_text(json.dumps(proof | {'mesh': mesh | {key: invalid}}))
                    with self.assertRaises(RuntimeError):
                        renderer_proof.renderer_proof(path, 0)
