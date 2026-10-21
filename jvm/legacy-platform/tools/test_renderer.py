"""Offline renderer proof acceptance and refusal regression tests."""
import json
from pathlib import Path
import struct
import tempfile
import unittest
import zlib

import renderer_proof


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
            'glUploadedBytes': 224, 'glPeakBufferBytes': 112, 'glUploadNanos': 1, 'glComparisonNanos': 1,
            **dict.fromkeys(renderer_proof.BAKED_REFUSALS, 0),
            'refusedUnaccountedVertices': 1, 'refusedQuadBudget': 1,
            'controlledDeferredSections': 3, 'world': [16, 0, 0, 0, 16, 16, 64], 'goldens': goldens}


def png_chunk(kind: bytes, data: bytes) -> bytes:
    return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))


class RendererProofTests(unittest.TestCase):
    def test_baked_requires_independent_atlas_and_observable_mismatch(self):
        with tempfile.TemporaryDirectory() as raw:
            directory = Path(raw)
            proof = baked_proof_fixture(directory)
            renderer_proof.baked_proof(proof, directory, 0)
            for field, invalid in (('atlasGeneration', 2), ('resourceGeneration', 0),
                                   ('firstMismatchedWord', 0), ('mismatchedWords', 1),
                                   ('drawable', False), ('comparedSections', 0),
                                   ('glComparedSections', 0), ('glStaleSections', -1),
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
                    ('shadedNativeResidency', 1, 1), ('shadedNativeResidency', 2, 5_636_193),
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
