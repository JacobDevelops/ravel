"""Validate fresh observational renderer evidence and bounded geometry artifacts."""
import json
from pathlib import Path
import struct
from typing import TypeAlias

JsonValue: TypeAlias = str | int | float | bool | None | list['JsonValue'] | dict[str, 'JsonValue']
Proof: TypeAlias = dict[str, JsonValue]

WORLD_SEED = 8675309
SNAPSHOT_EDGE = 18
SNAPSHOT_CELLS = SNAPSHOT_EDGE ** 3
SNAPSHOT_BORDER_CELLS = SNAPSHOT_CELLS - 16 ** 3
SNAPSHOT_BATCH_BYTES = SNAPSHOT_CELLS * 6
TIMING_SAMPLE_LIMIT = 4096
MAX_RESIDENT_BYTES = 1179648
MAX_STAGING_BYTES = 1212416
PROOF_IMAGE_EDGE = 64
PROOF_IMAGE_PIXELS = PROOF_IMAGE_EDGE ** 2
PROOF_IMAGE_BYTES = PROOF_IMAGE_PIXELS * 4


RENDERER_SEAMS = ('reload', 'unload', 'frame', 'terrain', 'layer', 'update',
                  'compileNormalReturn', 'taskCallbackNormalReturn', 'upload')


def renderer_proof(path: Path, started: float, selection: str = 'vanilla') -> Proof:
    if not path.is_file() or path.stat().st_mtime < started:
        raise RuntimeError(f'No fresh renderer proof: {path}')
    proof = json.loads(path.read_text())
    generations = [proof[key] for key in
                   ('beforeReload', 'afterReload', 'afterUnload', 'reopened', 'finalUnload')]
    before, reload, unload, reopened, final = generations
    if not (0 < before < reload < unload <= reopened < final
            and proof['staleRejected'] is True and proof['workerObserved'] is True
            and proof['injectedStaleObservationRejected'] is True
            and proof['ownerThread'] > 0
            and all(proof['seams'].get(name, 0) > 0 for name in RENDERER_SEAMS)):
        raise RuntimeError('Renderer generation ordering, stale rejection or seam proof missing')
    snapshot = proof['snapshot']
    count = snapshot['captures']
    if not (count > 0 and snapshot['seed'] == WORLD_SEED and snapshot['edge'] == SNAPSHOT_EDGE
            and snapshot['comparison'] == 'decoded-vs-Java-copied-post-compile-region'
            and snapshot['light'] == 'raw-stored-sky-and-block-not-combined-mesh-light'
            and snapshot['oracle'] == 'coordinate-only-same-mappings'
            and snapshot['modelKeys'] == 'state-derived-not-baked-model-identities'
            and snapshot['maxConcurrentCaptures'] == 1
            and snapshot['timingSampleLimit'] == TIMING_SAMPLE_LIMIT
            and snapshot['javaBatchBytes'] == SNAPSHOT_BATCH_BYTES
            and snapshot['retainedStaleProofBytes'] == SNAPSHOT_BATCH_BYTES
            and snapshot['maxJavaBatchPayloadBytes'] == 2 * SNAPSHOT_BATCH_BYTES
            and snapshot['maxNativeCopyBytes'] == SNAPSHOT_BATCH_BYTES
            and snapshot['maxNativeScratchBytes'] > snapshot['maxNativeCopyBytes']
            and snapshot['geometry'] == 'captured-game-isOpaqueCube-and-isFullCube'
            and snapshot['fullOpaqueCells'] > 0
            and snapshot['cells'] == count * SNAPSHOT_CELLS
            and snapshot['borderCells'] == count * SNAPSHOT_BORDER_CELLS
            and snapshot['mismatches'] == 0 and snapshot['nonAirCells'] > 0
            and snapshot['adjacentVariation'] > 0 and snapshot['workerCaptures'] > 0
            and snapshot['oracleChecks'] == 125 and snapshot['oracleMismatches'] == 0
            and snapshot['oracleBorderChecks'] == 98
            and snapshot['oracleRetryCandidates'] >= 0
            and snapshot['oracleRetryCandidates'] <= snapshot['oracleRetryCells']
            <= snapshot['oracleRetryCandidates'] * 125
            and snapshot['staleCaptureRejections'] > 0
            and snapshot['lastThread'] > 0 and snapshot['lastGeneration'] > 0
            and snapshot['rejected'] >= 0 and snapshot['skipped'] >= 0
            and 0 < snapshot['maxBytes'] <= snapshot['totalBytes']
            and 0 < snapshot['p50Nanos'] <= snapshot['p95Nanos'] <= snapshot['p99Nanos']
            <= snapshot['maxNanos'] <= snapshot['totalNanos']):
        raise RuntimeError('Bounded vanilla snapshot roundtrip, timing or stale capture proof missing')
    mesh_proof(proof['mesh'], selection)
    if selection == 'geometry-proof':
        geometry_proof(path.parent / 'geometry-upload.json', started)
    return proof


MESH_LABELS = {'geometry': 'game-isOpaqueCube-and-isFullCube-per-state-air-key-zero-empty',
               'fidelity': 'geometry-only-no-texture-ao-or-model-parity',
               'upload': 'none-native-owned-until-drained-then-discarded',
               'denominator': 'admitted-native-geometry-jobs-not-vanilla-compile-tasks',
               'frameInterval': 'observational-stress-window-frame-interval-not-mesh-attribution',
               'timingSampleLimit': TIMING_SAMPLE_LIMIT, 'keySlots': 16, 'recordWidth': 11,
               'maxRecordsPerPollLimit': 8, 'statsWidth': 18, 'stalePollRejected': True,
               'stress': {'seed': WORLD_SEED, 'teleports': 6, 'stepBlocks': 128, 'minimumDrainedPerStep': 32},
               'closeDrain': {'stopped': 1, 'outstanding': 0, 'queued': 0, 'running': 0,
                              'completed': 0, 'reservedBytes': 0, 'draining': 0, 'workerFailure': 0,
                              'javaKeysHeld': 0}}


def mesh_timing(mesh: Proof) -> bool:
    ordered = all(0 < mesh[f'{series}P50Nanos'] <= mesh[f'{series}P95Nanos'] <= mesh[f'{series}P99Nanos']
                  <= mesh[f'max{series.title()}Nanos'] for series in ('mesh', 'drain'))
    return (ordered and 0 <= mesh['queueP50Nanos'] <= mesh['queueP95Nanos'] <= mesh['queueP99Nanos']
            <= mesh['maxQueueNanos']
            and 0 < mesh['frameP50Nanos'] <= mesh['frameP95Nanos'] <= mesh['frameP99Nanos']
            <= mesh['maxFrameNanos']
            and 0 <= mesh['firstQueueNanos'] <= mesh['maxQueueNanos']
            and 0 < mesh['firstMeshNanos'] <= mesh['maxMeshNanos']
            and 0 < mesh['firstDrainNanos'] <= mesh['maxDrainNanos']
            and 0 < mesh['firstCaptureNanos'] <= mesh['maxCaptureNanos']
            and mesh['frameSamples'] > 0
            and 0 < mesh['firstFiveSecondsMaxFrameNanos'] <= mesh['maxFrameNanos'])


def unload_drained(mesh: Proof) -> bool:
    unload = mesh['unloadDrain']
    return (unload['generation'] > 0 and unload['retiredPoolGeneration'] == unload['generation']
            and 0 < unload['drainChecks'] <= mesh['drainChecks']
            and unload['generation'] <= mesh['lastRetiredPoolGeneration']
            and mesh['lifecyclesWithoutRetiredPool'] >= 0 and mesh['retiredUnadmitted'] >= 0)


def mesh_proof(mesh: Proof, selection: str = 'vanilla') -> None:
    stress = mesh['stress']
    retired = mesh['retiredBeforeDrain']
    evidence = 'actual-retired-before-drain' if retired > 0 else 'deterministic-harness-only'
    if selection not in ('vanilla', 'geometry-proof'):
        raise RuntimeError(f'Unsupported renderer selection: {selection}')
    labels = MESH_LABELS | ({'upload': 'bounded-native-wire-java-owned-offscreen-gl'}
                            if selection == 'geometry-proof' else {})
    if not (all(mesh[key] == value for key, value in labels.items())
            and mesh['cancellationEvidence'] == evidence and 0 <= retired <= mesh['submitted']
            and 0 < mesh['drained'] and mesh['drained'] + retired <= mesh['submitted'] and mesh['errors'] == 0
            and mesh['complete'] + mesh['incomplete'] + mesh['errors'] == mesh['drained']
            and mesh['incomplete'] <= mesh['unsupportedKeys']
            and mesh['maxUnsupportedKeys'] <= mesh['unsupportedKeys']
            and mesh['vertices'] > 0 and mesh['vertices'] % 4 == 0
            and mesh['indices'] * 4 == mesh['vertices'] * 6
            and 0 < mesh['maxOutputBytes'] <= mesh['outputBytes']
            and 0 < mesh['maxAllocations'] <= mesh['allocations']
            and 0 < mesh['nonEmptyMeshes'] <= mesh['drained']
            and 0 < mesh['occludingMeshes'] <= mesh['drained']
            and mesh['rejectedRecords'] == 0 and mesh['refused'] >= 0 and mesh['tableFull'] >= 0
            and mesh['stalePolls'] >= 0 and mesh['polls'] > 0
            and 0 < mesh['maxRecordsPerPoll'] <= mesh['maxRecordsPerPollLimit']
            and mesh['drainChecks'] > 0 and mesh['drainFailures'] == 0 and unload_drained(mesh)
            and 0 < mesh['maxOutstanding'] <= mesh['maxOutstandingLimit'] == 8
            and 0 < mesh['maxReservedBytes'] <= mesh['maxReservedBytesLimit']
            and 0 < mesh['maxTableOccupancy'] <= mesh['keySlots']
            and mesh['stressSubmitted'] > 0
            and mesh['stressDrained'] >= stress['teleports'] * stress['minimumDrainedPerStep']
            and mesh['captureFailures'] == 0 and mesh['lastFailureCode'] == 0 and mesh['workerFailures'] == 0
            and mesh['injectedStaleRecordRejections'] > 0 and mesh_timing(mesh)):
        raise RuntimeError('Bounded native geometry admission, drain, stale rejection or timing proof missing')


def geometry_proof(path: Path, started: float) -> Proof:
    if not path.is_file() or path.stat().st_mtime < started:
        raise RuntimeError(f'No fresh geometry upload proof: {path}')
    proof = json.loads(path.read_text())
    metrics, final = proof['metrics'], proof['finalMetrics']
    if not (proof['selection'] == 'geometry-proof' and proof['offDefault'] is True
            and proof['worldSubstitution'] is False and proof['fidelity'] == 'not-proven'
            and proof['reference'] == 'self-reference-not-vanilla-oracle'
            and proof['width'] == proof['height'] == 64 and proof['rowOrder'] == 'bottom-up'
            and len(proof['info']) == 3 and proof['info'][0] == 'geometry-proof'
            and proof['info'][1] in ('OpenGL15', 'ARB_vertex_buffer_object')
            and proof['info'][2] == 'geometric-only-no-uv-sprite-tint-ao-baked-light-world-replacement-refused'
            and all(type(value) is int and value >= 0 for value in metrics.values())
            and all(type(value) is int and value >= 0 for value in final.values())
            and all(metrics[key] > 0 for key in
                    ('copiedMeshes', 'copiedBytes', 'uploads', 'uploadBytes', 'submissions',
                     'generation', 'key', 'liveBuffers', 'liveFramebuffers', 'maxStagingBytes'))
            and metrics['referenceMismatchPixels'] == 0
            and 0 < metrics['residentBytes'] <= metrics['maxResidentBytes'] <= MAX_RESIDENT_BYTES
            and 0 <= metrics['stagingBytes'] <= metrics['maxStagingBytes'] <= MAX_STAGING_BYTES
            and final['cleanups'] > 0
            and metrics['maxResidentBytes'] <= final['maxResidentBytes'] <= MAX_RESIDENT_BYTES
            and metrics['maxStagingBytes'] <= final['maxStagingBytes'] <= MAX_STAGING_BYTES
            and all(final[key] == 0 for key in
                    ('residentBytes', 'stagingBytes', 'liveBuffers', 'liveFramebuffers'))):
        raise RuntimeError('Geometry selection, self-reference, upload bounds or lifecycle cleanup proof missing')
    vanilla = proof['vanillaComparison']
    if not (vanilla['status'] == 1 and vanilla['generation'] == metrics['generation']
            and vanilla['key'] == metrics['key'] and vanilla['vanillaVertices'] > 0
            and proof['vanillaScope'] == 'published-SOLID-only-section-position-coverage-depth'
            and vanilla['vanillaLayers'] == 1 and vanilla['totalPixels'] == PROOF_IMAGE_PIXELS
            and metrics['vanillaComparisons'] > 0
            and all(type(vanilla[key]) is int and 0 <= vanilla[key] <= PROOF_IMAGE_PIXELS for key in
                    ('rgbaMismatchPixels', 'coverageMismatchPixels', 'depthMismatchPixels'))
            and all(vanilla[field] == metrics[metric] for field, metric in (
                ('rgbaMismatchPixels', 'vanillaRgbaMismatchPixels'),
                ('coverageMismatchPixels', 'vanillaCoverageMismatchPixels'),
                ('depthMismatchPixels', 'vanillaDepthMismatchPixels')))):
        raise RuntimeError('Actual same-section vanilla comparison or truthful mismatch counts missing')
    pixels = []
    for name in ('native', 'reference', 'vanilla'):
        raw = path.parent / f'geometry-{name}.rgba'
        png = path.parent / f'geometry-{name}.png'
        for artifact in (raw, png):
            if not artifact.is_file() or artifact.stat().st_mtime < started:
                raise RuntimeError(f'No fresh geometry artifact: {artifact}')
        data, image = raw.read_bytes(), png.read_bytes()
        if (len(data) != PROOF_IMAGE_BYTES or image[:8] != b'\x89PNG\r\n\x1a\n'
                or image[12:16] != b'IHDR'
                or image[16:24] != struct.pack('>II', PROOF_IMAGE_EDGE, PROOF_IMAGE_EDGE)):
            raise RuntimeError('Geometry pixel artifact dimensions invalid')
        pixels.append(data)
    if not all(any(image[i] != 0 for i in range(0, PROOF_IMAGE_BYTES, 4)) for image in (pixels[0], pixels[2])):
        raise RuntimeError('Native and actual vanilla proof must both contain foreground coverage')
    mismatch = sum(pixels[0][i:i + 4] != pixels[1][i:i + 4] for i in range(0, PROOF_IMAGE_BYTES, 4))
    if mismatch != metrics['referenceMismatchPixels']:
        raise RuntimeError('Geometry self-reference mismatch count does not match raw pixels')
    rgba = sum(pixels[0][i:i + 4] != pixels[2][i:i + 4] for i in range(0, PROOF_IMAGE_BYTES, 4))
    # GeometryProof uses foreground red, not alpha: its opaque black background has alpha 255.
    coverage = sum((pixels[0][i] != 0) != (pixels[2][i] != 0) for i in range(0, PROOF_IMAGE_BYTES, 4))
    if rgba != vanilla['rgbaMismatchPixels'] or coverage != vanilla['coverageMismatchPixels']:
        raise RuntimeError('Vanilla mismatch counts do not match raw pixels')
    baked_proof(proof['baked'], path.parent, started)
    return proof


BAKED_SCENES = ('opaque', 'cutout-mipped', 'cutout', 'translucent', 'animated', 'staged-reload', 'unload',
                'translucent-fluid')
BAKED_REFUSALS = ('refusedQuadBudget', 'refusedUncolored', 'refusedForeignBuilder', 'refusedObservation',
                  'refusedIncompleteQuad', 'refusedUnaccountedVertices', 'refusedRetiredAtFinish',
                  'refusedRustDecode', 'refusedRustOther')
BAKED_SAMPLE_BYTES = 16 + PROOF_IMAGE_PIXELS * 16
BAKED_SAMPLE_LIMIT = 8
BAKED_DESIGNATED = 5
BAKED_DEPTH_TOLERANCE = 2.0 / 0xffffff
BAKED_MEASURED = 'controlled-section-rust-emission-vs-final-vanilla-buffer-offscreen'
BAKED_FLUID = 'fluid-tessellation-not-baked-quad-provenance'


def float32(value: float) -> float:
    return struct.unpack('<f', struct.pack('<f', value))[0]


def baked_sample(data: bytes, scene: int) -> tuple[list[int], int]:
    """Recounts one raw emitted/oracle readback pair exactly as the GL comparison counts it."""
    if len(data) != BAKED_SAMPLE_BYTES:
        raise RuntimeError('Baked raw readback size invalid')
    header, layer, vertices, designated = struct.unpack_from('<4i', data)
    if (header != scene or not 0 <= layer < 4 or vertices <= 0 or vertices % 4
            or not 0 <= designated < BAKED_DESIGNATED):
        raise RuntimeError('Baked raw readback header invalid')
    if layer != (0, 1, 2, 3, 0)[designated]:
        raise RuntimeError('Baked raw readback layer differs from its designated scene')
    pixels = PROOF_IMAGE_PIXELS
    emitted, oracle = data[16:16 + pixels * 4], data[16 + pixels * 8:16 + pixels * 12]
    emitted_depth = struct.unpack_from(f'<{pixels}f', data, 16 + pixels * 4)
    oracle_depth = struct.unpack_from(f'<{pixels}f', data, 16 + pixels * 12)
    counts = [1, 0, 0, 0, 0, 0, vertices]
    for pixel in range(pixels):
        left, right = emitted[pixel * 4:pixel * 4 + 4], oracle[pixel * 4:pixel * 4 + 4]
        counts[1] += left != right
        counts[2] += (left[3] != 0) != (right[3] != 0)
        counts[3] += abs(float32(emitted_depth[pixel] - oracle_depth[pixel])) > BAKED_DEPTH_TOLERANCE
        counts[4] += left[3] != 0
        counts[5] += right[3] != 0
    return counts, designated


def baked_golden(row: Proof, scene: int, directory: Path, started: float) -> None:
    name, values, files = BAKED_SCENES[scene], row.get('values'), row.get('readbacks')
    if not (row.get('scene') == name and isinstance(values, list) and len(values) == 7
            and all(type(value) is int and value >= 0 for value in values)
            and isinstance(files, list) and len(files) <= BAKED_SAMPLE_LIMIT):
        raise RuntimeError(f'Malformed baked golden row: {name}')
    if scene == len(BAKED_SCENES) - 1:
        if row.get('status') != 'unsupported' or row.get('reason') != BAKED_FLUID or any(values) or files:
            raise RuntimeError(f'Untruthful baked golden disposition: {name}')
        return
    if row.get('status') != 'measured' or row.get('reason') != BAKED_MEASURED:
        raise RuntimeError(f'Untruthful baked golden disposition: {name}')
    totals, sections = [0] * 7, []
    for index, file in enumerate(files):
        path = directory / f'baked-golden-{scene}-{index}.raw'
        if file != path.name or not path.is_file() or path.stat().st_mtime < started:
            raise RuntimeError(f'No fresh baked raw readback: {path}')
        counts, section = baked_sample(path.read_bytes(), scene)
        totals = [total + count for total, count in zip(totals, counts)]
        sections.append(section)
    expected = [scene] if scene < BAKED_DESIGNATED else list(range(BAKED_DESIGNATED))
    if (totals != values or sorted(sections) != expected or any(values[1:4])
            or values[4] == 0 or values[5] == 0):
        raise RuntimeError(f'Baked golden counts differ from raw readbacks: {name}')


def baked_proof(proof: Proof, directory: Path, started: float) -> None:
    labels = {'comparison': 'Rust-emission-vs-final-vanilla-BLOCK-words', 'drawable': True,
              'worldSubstitution': False, 'maxQuadsPerSection': 8192}
    nonnegative = ('busySections', 'unsupportedSections', 'staleSections', 'animatedSpriteObservations',
                   'glStaleSections', 'glUnsupportedSections', 'controlledDeferredSections', *BAKED_REFUSALS)
    positive = ('comparedSections', 'quads', 'cpuNanos', 'maxInputBytes', 'maxOutputBytes',
                'resourceGeneration', 'atlasGeneration', 'glComparedSections',
                'glUploadedBytes', 'glPeakBufferBytes', 'glUploadNanos', 'glComparisonNanos')
    if not (all(proof.get(key) == value and type(proof.get(key)) is type(value) for key, value in labels.items())
            and all(type(proof.get(key)) is int and proof[key] >= 0 for key in nonnegative)
            and all(type(proof.get(key)) is int and proof[key] > 0 for key in positive)
            and type(proof.get('mismatchedWords')) is int and proof['mismatchedWords'] == 0
            and type(proof.get('firstMismatchedWord')) is int and proof['firstMismatchedWord'] == -1
            and proof['resourceGeneration'] == proof['atlasGeneration']
            and sum(proof[key] for key in BAKED_REFUSALS) == proof['unsupportedSections']
            and proof['maxInputBytes'] <= 8_000_000 and proof['maxOutputBytes'] <= 4_000_000
            and proof['glComparedSections'] <= proof['comparedSections']
            and proof['glPeakBufferBytes'] <= 8192 * 112 and proof['glPeakBufferBytes'] % 112 == 0
            and proof['glUploadedBytes'] % 224 == 0 and proof['glUploadedBytes'] <= proof['quads'] * 224):
        raise RuntimeError('Bounded independently atlas-scoped Rust baked comparison missing or mismatched')
    world = proof.get('world')
    if not (isinstance(world, list) and len(world) == 7 and all(type(value) is int and value >= 0 for value in world)
            and 0 < world[0] <= 4 * proof['glComparedSections']
            and not any(world[1:4])
            and all(value <= world[0] * PROOF_IMAGE_PIXELS for value in world[1:6])):
        raise RuntimeError('Baked whole-run GL comparison totals missing or impossible')
    rows = proof.get('goldens')
    if not (isinstance(rows, list) and len(rows) == len(BAKED_SCENES) and all(isinstance(row, dict) for row in rows)):
        raise RuntimeError('Complete baked golden scene matrix missing')
    for scene, row in enumerate(rows):
        baked_golden(row, scene, directory, started)
    if any(sum(row['values'][index] for row in rows) > world[index] for index in range(7)):
        raise RuntimeError('Baked golden totals exceed whole-run GL comparison totals')
