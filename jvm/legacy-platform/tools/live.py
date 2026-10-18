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


def launch(action: str, connected: bool = False, suffix: str = '') -> dict[str, object]:
    environment = os.environ.copy()
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
                                     'missingHooks': missing_hooks, 'missingSections': missing_sections,
                                     'errors': errors, 'passed': passed}, indent=2) + '\n').encode())
    if not passed:
        raise SystemExit(f'Legacy acceptance incomplete; actual results: {record}')


if __name__ == '__main__':
    main()
