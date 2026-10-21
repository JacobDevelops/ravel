#!/usr/bin/env python3
"""Require fresh official premain/JNI evidence; never infer an unexercised success."""
import gzip
import json
import os
from pathlib import Path
import re
import struct
import subprocess
import tempfile
import time
import tomllib

from acquire import ROOT, WORK, atomic_write
SECTIONS = {'cluster', 'content', 'hierarchy', 'initialization', 'load', 'members',
            'outward', 'persistence', 'refusal', 'screens', 'settings', 'side', 'transformation'}
SUPPLEMENTAL = {'legacy.world-open', 'legacy.world-open-integrated', 'legacy.command-install',
                'legacy.command-execute', 'legacy.item-fire-damage', 'legacy.title-init',
                'legacy.global-palette-size'}
RENDERER_SEAMS = ('reload', 'unload', 'frame', 'terrain', 'layer', 'update',
                  'compileNormalReturn', 'taskCallbackNormalReturn', 'upload')


def renderer_proof(path: Path, started: float) -> dict:
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
    if not (count > 0 and snapshot['seed'] == 8675309 and snapshot['edge'] == 18
            and snapshot['comparison'] == 'decoded-vs-Java-copied-post-compile-region'
            and snapshot['light'] == 'raw-stored-sky-and-block-not-combined-mesh-light'
            and snapshot['oracle'] == 'coordinate-only-same-mappings'
            and snapshot['modelKeys'] == 'state-derived-not-baked-model-identities'
            and snapshot['maxConcurrentCaptures'] == 1
            and snapshot['timingSampleLimit'] == 4096
            and snapshot['javaBatchBytes'] == 34992
            and snapshot['retainedStaleProofBytes'] == 34992
            and snapshot['maxJavaBatchPayloadBytes'] == 69984
            and snapshot['maxNativeCopyBytes'] == 34992
            and snapshot['maxNativeScratchBytes'] > snapshot['maxNativeCopyBytes']
            and snapshot['geometry'] == 'captured-game-isOpaqueCube-and-isFullCube'
            and snapshot['fullOpaqueCells'] > 0
            and snapshot['cells'] == count * 5832 and snapshot['borderCells'] == count * 1736
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
    mesh_proof(proof['mesh'])
    return proof


MESH_LABELS = {'geometry': 'game-isOpaqueCube-and-isFullCube-per-state-air-key-zero-empty',
               'fidelity': 'geometry-only-no-texture-ao-or-model-parity',
               'upload': 'none-native-owned-until-drained-then-discarded',
               'denominator': 'admitted-native-geometry-jobs-not-vanilla-compile-tasks',
               'frameInterval': 'observational-stress-window-frame-interval-not-mesh-attribution',
               'timingSampleLimit': 4096, 'keySlots': 16, 'recordWidth': 11,
               'maxRecordsPerPollLimit': 8, 'statsWidth': 18, 'stalePollRejected': True,
               'stress': {'seed': 8675309, 'teleports': 6, 'stepBlocks': 128, 'minimumDrainedPerStep': 32},
               'closeDrain': {'stopped': 1, 'outstanding': 0, 'queued': 0, 'running': 0,
                              'completed': 0, 'reservedBytes': 0, 'draining': 0, 'workerFailure': 0,
                              'javaKeysHeld': 0}}


def mesh_timing(mesh: dict) -> bool:
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


def unload_drained(mesh: dict) -> bool:
    unload = mesh['unloadDrain']
    return (unload['generation'] > 0 and unload['retiredPoolGeneration'] == unload['generation']
            and 0 < unload['drainChecks'] <= mesh['drainChecks']
            and unload['generation'] <= mesh['lastRetiredPoolGeneration']
            and mesh['lifecyclesWithoutRetiredPool'] >= 0 and mesh['retiredUnadmitted'] >= 0)


def mesh_proof(mesh: dict) -> None:
    stress = mesh['stress']
    retired = mesh['retiredBeforeDrain']
    evidence = 'actual-retired-before-drain' if retired > 0 else 'deterministic-harness-only'
    if not (all(mesh[key] == value for key, value in MESH_LABELS.items())
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


def launch(action: str, connected: bool = False, suffix: str = '') -> dict[str, object]:
    environment = os.environ.copy()
    environment.setdefault('ALSOFT_DRIVERS', 'null')
    environment['RAVEL_LEGACY_PROBES'] = str(WORK / 'evidence-harness-agent/classes')
    environment['RAVEL_PACKET_LIVE'] = tempfile.mkdtemp(prefix='packets-', dir=WORK)
    environment['RAVEL_LEGACY_EVIDENCE_SUFFIX'] = suffix
    if connected:
        environment['RAVEL_LEGACY_CONNECT_CLIENT'] = '1'
    began = time.monotonic()
    result = subprocess.run(['bash', str(ROOT / 'jvm/legacy-platform/launch.sh'),
                             action, 'xvfb'], cwd=ROOT, env=environment, check=False)
    return {'action': action, 'connectedClient': connected, 'suffix': suffix,
            'exit': result.returncode, 'durationMs': round((time.monotonic() - began) * 1000)}


def counts(path: Path) -> dict[str, int]:
    text = path.read_text()
    if re.search(r'\[ravel\] (failed|no distribution|the distribution)', text):
        raise RuntimeError(f'Agent installation/transformation failed in {path}')
    if not re.search(r'[1-9][0-9]* hook invocations reached native code', text):
        raise RuntimeError(f'No successful native hook crossing in {path}')
    if 'nativeAccess agent=dev.ravel.agent:true platform=false' not in text:
        raise RuntimeError(f'Named-module native boundary was not observed in {path}')
    return {name: int(value) for name, value in re.findall(
        r'\[ravel-legacy\] hook (legacy\.[a-z-]+)=([0-9]+)', text)}


def saved_visits(path: Path | None = None) -> int:
    if path is None:
        path = WORK / 'dedicated-agent/ravel-legacy/data/ravel_mods.dat'
    raw = gzip.decompress(path.read_bytes())
    key = b'\x07\x00\x09aggregate'
    start = raw.find(key)
    if start < 0:
        raise RuntimeError(f'No real aggregate ByteArrayTag in {path}')
    offset = start + len(key)
    length = struct.unpack_from('>i', raw, offset)[0]
    payload = json.loads(raw[offset + 4:offset + 4 + length])
    entry = next(entry for entry in payload['owners'] if entry['owner'] == 'saved_data')
    if entry['dataVersion'] != 3:
        raise RuntimeError('Official server checkpoint did not preserve current mod schema')
    return entry['payload']['visits']


def collect(started: float, inventory: dict) -> tuple[dict[str, int], list[str]]:
    hooks = {hook['id']: 0 for hook in inventory['hook']}
    errors = []
    for label in ('xvfb-agent', 'xvfb-agent-dedicated', 'server-agent', 'server-agent-restart'):
        log = WORK / f'evidence-{label}' / ('client.log' if label.startswith('xvfb') else 'server.log')
        try:
            if not log.is_file() or log.stat().st_mtime < started:
                raise RuntimeError(f'No fresh process evidence: {log}')
            for name, count in counts(log).items():
                if name in hooks:
                    hooks[name] += count
        except (OSError, RuntimeError) as failure:
            errors.append(str(failure))
    return hooks, errors


def main() -> None:
    output = WORK / 'evidence-agent-acceptance'
    output.mkdir(parents=True, exist_ok=True)
    record = output / 'result.json'
    record.unlink(missing_ok=True)
    started = time.time()
    harness = subprocess.run(['bash', str(ROOT / 'jvm/harness/run-legacy-harness.sh')],
                             cwd=ROOT, check=False)
    processes = [launch('client-agent'), launch('server-agent', connected=True)]
    persistence = {}
    errors = []
    renderer = {}
    try:
        renderer = renderer_proof(WORK / 'evidence-xvfb-agent/renderer.json', started)
    except (OSError, ValueError, KeyError, TypeError, RuntimeError) as failure:
        errors.append(f'Renderer lifecycle: {failure}')
    try:
        integrated = WORK / 'evidence-xvfb-agent'
        for name in ('first', 'reopened'):
            checkpoint = integrated / f'integrated-{name}.dat'
            if checkpoint.stat().st_mtime < started:
                raise RuntimeError(f'No fresh integrated checkpoint: {checkpoint}')
            persistence[f'integrated{name.title()}Visits'] = saved_visits(checkpoint)
        if persistence['integratedReopenedVisits'] != persistence['integratedFirstVisits'] + 1:
            raise RuntimeError('Integrated reopen did not checkpoint exactly one additional visit')
    except (OSError, ValueError, KeyError, StopIteration, RuntimeError) as failure:
        errors.append(f'Integrated checkpoint: {failure}')
    try:
        persistence['beforeRestartVisits'] = saved_visits()
    except (OSError, ValueError, KeyError, StopIteration, RuntimeError) as failure:
        errors.append(f'Dedicated checkpoint: {failure}')
    processes.append(launch('server-agent', suffix='-restart'))
    try:
        persistence['afterRestartVisits'] = saved_visits()
        if persistence['afterRestartVisits'] != persistence['beforeRestartVisits'] + 1:
            raise RuntimeError('Dedicated restart did not admit and checkpoint exactly one additional visit')
    except (OSError, ValueError, KeyError, StopIteration, RuntimeError) as failure:
        errors.append(f'Dedicated restart: {failure}')
    inventory = tomllib.loads((ROOT / 'minecraft/versions/1.12.2/hooks.toml').read_text())
    hooks, collection_errors = collect(started, inventory)
    errors.extend(collection_errors)
    section_record = ROOT / 'target/ravel-jvm/legacy-harness-sections.txt'
    sections = set()
    if section_record.is_file() and section_record.stat().st_mtime >= started:
        sections = set(re.split(r'[\s,]+', section_record.read_text().strip()))
    missing_hooks = sorted(name for name, count in hooks.items() if count == 0)
    missing_sections = sorted(SECTIONS - sections)
    passed = (not errors and not missing_hooks and not missing_sections and harness.returncode == 0
              and all(process['exit'] == 0 for process in processes))
    atomic_write(record, (json.dumps({'processes': processes, 'harnessExit': harness.returncode,
                                     'reviewedHooks': {k: v for k, v in hooks.items() if k not in SUPPLEMENTAL},
                                     'supplementalHooks': {k: v for k, v in hooks.items() if k in SUPPLEMENTAL},
                                     'sections': sorted(sections), 'persistence': persistence,
                                     'renderer': renderer,
                                     'missingHooks': missing_hooks, 'missingSections': missing_sections,
                                     'errors': errors, 'passed': passed}, indent=2) + '\n').encode())
    if not passed:
        raise SystemExit(f'Legacy acceptance incomplete; actual results: {record}')


if __name__ == '__main__':
    main()
