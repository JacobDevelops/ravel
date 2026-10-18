#!/usr/bin/env python3
"""Acquire official 1.12.2 inputs without any LWJGL2 artifacts."""
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import shutil
import sys
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parents[3]
WORK = ROOT / 'tests/fixtures/local/legacy-platform'
DEFAULT_INPUT = ROOT / 'tests/fixtures/local/tooling/minecraft-1.12.2'
INPUT = Path(os.environ.get('RAVEL_LEGACY_INPUTS', DEFAULT_INPUT))
MANIFEST_SHA256 = '827aaf8f97209f7747897e6c78875ae2c1bf732bb81ab85a298e6d22223cf385'
MANIFEST_URL = 'https://piston-meta.mojang.com/v1/packages/6e69e85d0f85f4f4b9e12dd99d102092a6e15918/1.12.2.json'
LEDGER = ROOT / 'jvm/legacy-platform/official-dependencies.sha256'
PINS = dict((name, pin) for pin, name in
            (line.split('  ', 1) for line in LEDGER.read_text().splitlines()))


def digest(path, algorithm):
    with path.open('rb') as source:
        return hashlib.file_digest(source, algorithm).hexdigest()


def acquire(item, destination, local=None):
    destination.parent.mkdir(parents=True, exist_ok=True)
    if not destination.exists():
        temporary = destination.with_suffix(destination.suffix + '.download')
        if local is not None and local.exists():
            shutil.copyfile(local, temporary)
        else:
            with urllib.request.urlopen(item['url'], timeout=90) as response, temporary.open('wb') as output:
                shutil.copyfileobj(response, output)
        if digest(temporary, 'sha1') != item['sha1']:
            temporary.unlink()
            raise RuntimeError('Official SHA1 mismatch: ' + str(destination))
        temporary.replace(destination)
    if digest(destination, 'sha1') != item['sha1']:
        raise RuntimeError('Official SHA1 mismatch: ' + str(destination))
    name = str(destination.relative_to(WORK))
    actual = digest(destination, 'sha256')
    if name not in PINS or actual != PINS[name]:
        raise RuntimeError('Pinned SHA256 mismatch: ' + name)
    return name, actual


def allowed(library):
    if library['name'].startswith(('org.lwjgl.lwjgl:', 'net.java.jinput:', 'net.java.jutils:')):
        return False
    result = 'rules' not in library
    for rule in library.get('rules', []):
        system = rule.get('os', {})
        if system.get('name', 'linux') == 'linux' and 'version' not in system:
            result = rule['action'] == 'allow'
    return result


def asset_jobs(version):
    index = version['assetIndex']
    index_path = WORK / 'assets/indexes' / (index['id'] + '.json')
    acquire(index, index_path)
    return [({'sha1': item['hash'], 'url': 'https://resources.download.minecraft.net/'
              + item['hash'][:2] + '/' + item['hash']},
             WORK / 'assets/objects' / item['hash'][:2] / item['hash'], None)
            for item in json.loads(index_path.read_text())['objects'].values()]


def main():
    WORK.mkdir(parents=True, exist_ok=True)
    manifest = INPUT / 'version.json'
    if not manifest.exists():
        manifest = WORK / 'version.json'
        if not manifest.exists():
            with urllib.request.urlopen(MANIFEST_URL, timeout=90) as response:
                manifest.write_bytes(response.read())
    if digest(manifest, 'sha256') != MANIFEST_SHA256:
        raise RuntimeError('Pinned 1.12.2 version manifest SHA256 mismatch')
    version = json.loads(manifest.read_text())
    if manifest != WORK / 'version.json':
        shutil.copyfile(manifest, WORK / 'version.json')
    PINS['version.json'] = MANIFEST_SHA256
    jobs = []
    for side in ('client', 'server'):
        jobs.append((version['downloads'][side], WORK / (side + '.jar'), INPUT / (side + '.jar')))
    libraries = []
    for library in version['libraries']:
        if not allowed(library):
            continue
        artifact = library['downloads'].get('artifact')
        if artifact:
            path = WORK / 'libraries' / artifact['path']
            jobs.append((artifact, path, None))
            if path not in libraries:
                libraries.append(path)
        native = library['downloads'].get('classifiers', {}).get('natives-linux')
        if native:
            path = WORK / 'libraries' / native['path']
            jobs.append((native, path, None))
            if path not in libraries:
                libraries.append(path)
    if sys.argv[1:] == ['--assets']:
        jobs.extend(asset_jobs(version))
    elif sys.argv[1:]:
        raise SystemExit('usage: acquire.py [--assets]')
    unique = {str(job[1]): job for job in jobs}
    with concurrent.futures.ThreadPoolExecutor(max_workers=12) as pool:
        for _ in pool.map(lambda job: acquire(*job), unique.values()):
            pass
    native_dir = WORK / 'natives'
    native_dir.mkdir(exist_ok=True)
    for path in libraries:
        if path.name.endswith('-natives-linux.jar'):
            with zipfile.ZipFile(path) as archive:
                for name in archive.namelist():
                    if name.endswith('.so'):
                        (native_dir / Path(name).name).write_bytes(archive.read(name))
    for side in ('client', 'server'):
        entries = [WORK / (side + '.jar')]
        if side == 'client':
            entries.extend(libraries)
        (WORK / (side + '-classpath.txt')).write_text(''.join(str(path) + '\n' for path in entries))
    print('Official SHA1 and local SHA256 ledger enforced:', LEDGER)


if __name__ == '__main__':
    main()
