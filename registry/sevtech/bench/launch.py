#!/usr/bin/env python3
"""Resolve private, pinned Linux profiles before the benchmark timer starts.

This module never starts Minecraft. prepare_command returns an argv and cwd for
Popen; its only account is the deterministic, unauthenticated BenchOffline user.
Java version probes are bounded to 30 seconds and finish before measurement.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import shlex
import shutil
import stat
import subprocess
import tempfile
import tomllib
from typing import Any, Iterable
import urllib.request
import uuid
import zipfile

from prepare import HARNESS_SETTINGS, digest, require, write_json

ROOT = Path(__file__).resolve().parents[3]
LOCAL = ROOT / 'tests/fixtures/local/sevtech-ages-3.2.3/baseline'
COMPONENT_PINS = {
    ('net.minecraft', '1.12.2'): '9f985e63b5df42a5bdf44c4a5688ed5605913bbe9f541a436c7aae950c6e21fc',
    ('org.lwjgl', '2.9.4-nightly-20150209'): '29a245e8def0c2eaa53d1373f70af0f8b43e677a88a529989e8de0f6a3e93d4c',
    ('net.minecraftforge', '14.23.5.2860'): 'f9030356b727ced32d9d1adaaacdb9049d88dcc519986f62c5de32d6898c33aa',
}


def private_path(root: Path, relative: str) -> Path:
    name = PurePosixPath(relative)
    require(not name.is_absolute() and name.parts and '..' not in name.parts and
            '\\' not in relative, 'unsafe artifact path: ' + relative)
    target = root.joinpath(*name.parts)
    current = root
    require(not current.is_symlink(), 'linked private input: ' + str(current))
    for part in name.parts:
        current = current / part
        require(not current.is_symlink(), 'linked private input: ' + str(current))
    require(target.resolve().is_relative_to(root.resolve()), 'artifact escapes private root')
    return target


def remember(path: Path, url: str, root: Path, records: dict[str, Any]) -> Path:
    key = str(path.relative_to(root))
    record = {'url': url, 'sha256': digest(path), 'size': path.stat().st_size}
    previous = records.get(key)
    require(previous is None or previous == record, 'previous SHA256/URL pin differs: ' + key)
    records[key] = record
    return path


def fetch(path: Path, spec: dict[str, Any], root: Path, records: dict[str, Any],
          source: Path | None = None) -> Path:
    spec = dict(spec)
    if spec['url'].startswith('http://'):
        spec['url'] = 'https://' + spec['url'][7:]
    require(not path.is_symlink(), 'linked download destination: ' + str(path))
    require(spec.get('sha1') or spec.get('sha256'), 'download has no content hash: ' + spec['url'])
    path.parent.mkdir(parents=True, exist_ok=True)
    if not path.exists():
        with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as stream:
            temporary = Path(stream.name)
        try:
            if source is not None and source.is_file():
                shutil.copyfile(source, temporary)
            else:
                try:
                    request = urllib.request.Request(spec['url'], headers={'User-Agent': 'RavelSevTechBaseline/1.0'})
                    with urllib.request.urlopen(request, timeout=90) as incoming, temporary.open('wb') as out:
                        shutil.copyfileobj(incoming, out)
                except urllib.error.URLError as error:
                    raise ValueError('artifact download failed: ' + spec['url'] + ': ' + str(error)) from error
            verify_download(temporary, spec)
            temporary.replace(path)
        finally:
            temporary.unlink(missing_ok=True)
    verify_download(path, spec)
    return remember(path, spec['url'], root, records)


def verify_download(path: Path, spec: dict[str, Any]) -> None:
    if 'size' in spec:
        require(path.stat().st_size == spec['size'], 'download size differs: ' + str(path))
    for algorithm in ('sha1', 'sha256'):
        if algorithm in spec:
            require(digest(path, algorithm) == spec[algorithm], algorithm.upper() + ' differs: ' + str(path))


def allowed(row: dict[str, Any]) -> bool:
    if 'rules' not in row:
        return True
    result = False
    for rule in row['rules']:
        system = rule.get('os', {})
        if system.get('name', 'linux') != 'linux':
            continue
        if 'arch' in system and not re.fullmatch(system['arch'], platform.machine()):
            continue
        if 'version' in system and not re.search(system['version'], platform.release()):
            continue
        if any(bool(value) for value in rule.get('features', {}).values()):
            continue
        require(rule['action'] in ('allow', 'disallow'), 'unknown library rule action')
        result = rule['action'] == 'allow'
    return result


def library_key(row: dict[str, Any]) -> tuple[str, str, tuple[str, ...], bool]:
    """Identify a Maven library independently of its version."""
    parts = row['name'].split(':')
    return (*parts[:2], tuple(parts[3:]), bool(row.get('natives')))


def merge_components(components: list[dict[str, Any]]) -> dict[str, Any]:
    result = {'libraries': [], 'tweakers': []}
    for component in sorted(components, key=lambda row: row.get('order', 0)):
        for key in ('mainClass', 'mainJar', 'minecraftArguments', 'assetIndex', 'logging'):
            if key in component:
                result[key] = component[key]
        for tweaker in component.get('+tweakers', []):
            if tweaker not in result['tweakers']:
                result['tweakers'].append(tweaker)
        for library in component.get('libraries', []) + component.get('+libraries', []):
            if not allowed(library):
                continue
            key = library_key(library)
            index = next((i for i, old in enumerate(result['libraries']) if library_key(old) == key), None)
            if index is None:
                result['libraries'].append(library)
            else:
                result['libraries'][index] = library
    return result


def maven_path(name: str, classifier: str | None = None) -> str:
    coordinate, _, extension = name.partition('@')
    parts = coordinate.split(':')
    require(len(parts) in (3, 4), 'unsupported Maven coordinate: ' + name)
    group, artifact, version = parts[:3]
    suffix = classifier or (parts[3] if len(parts) == 4 else '')
    filename = artifact + '-' + version + ('-' + suffix if suffix else '') + '.' + (extension or 'jar')
    return '/'.join((group.replace('.', '/'), artifact, version, filename))


def verify_overlay(path: Path, instance: Path) -> None:
    import cleanroom
    archive = LOCAL / ('cleanroom-' + cleanroom.VERSION + '.zip')
    require(archive.is_file() and digest(archive) == cleanroom.SHA256,
            'missing or changed pinned Cleanroom overlay: ' + str(archive))
    with zipfile.ZipFile(archive) as source:
        expected = source.read(str(path.relative_to(instance)))
    require(digest(path) == hashlib.sha256(expected).hexdigest(),
            'Cleanroom overlay entry differs: ' + str(path))


def read_components(instance: Path, records: dict[str, Any]) -> list[dict[str, Any]]:
    pack = json.loads((instance / 'mmc-pack.json').read_text())
    rows = pack['components']
    versions = {r['uid']: r['version'] for r in rows}
    require(versions.get('net.minecraft') == '1.12.2', 'only exact Minecraft 1.12.2 is supported')
    expected = {uid: version for uid, version in COMPONENT_PINS}
    if instance.name == 'cleanroom':
        expected.update({'net.minecraftforge': '0.7.8-alpha', 'org.lwjgl': '3.4.3'})
    require(versions == expected and len(rows) == len(expected), 'exact component set differs')
    components = []
    for row in rows:
        uid, version = row['uid'], row['version']
        patch = private_path(instance, 'patches/' + uid + '.json')
        if patch.is_file():
            require(instance.name == 'cleanroom', 'original component overrides are not permitted')
            verify_overlay(patch, instance)
            remember(patch, 'instance-overlay:' + uid + '/' + version, LOCAL, records)
            component = json.loads(patch.read_text())
        else:
            require((uid, version) in COMPONENT_PINS, 'no pinned component metadata: ' + uid + '/' + version)
            url = 'https://meta.prism' + 'launcher.org/v1/' + uid + '/' + version + '.json'
            path = private_path(LOCAL, 'resolved/meta/' + uid + '/' + version + '.json')
            fetch(path, {'url': url, 'sha256': COMPONENT_PINS[(uid, version)]}, LOCAL, records)
            component = json.loads(path.read_text())
        require(component.get('uid') == uid and component.get('version') == version,
                'component identity differs: ' + uid)
        for dependency in component.get('requires', []):
            require(dependency['uid'] in versions, 'missing component: ' + dependency['uid'])
            if 'equals' in dependency:
                require(versions[dependency['uid']] == dependency['equals'], 'component requirement differs')
        components.append(component)
    return components


def official_metadata(records: dict[str, Any]) -> tuple[dict[str, Any], Path]:
    inputs = ROOT / 'tests/fixtures/local/tooling/minecraft-1.12.2/inputs.toml'
    require(inputs.is_file(), 'missing pinned official Minecraft inputs: ' + str(inputs))
    pins = tomllib.loads(inputs.read_text())['inputs']
    pin = pins['versionManifest']
    spec = {'url': pin['upstream'], 'sha256': pin['sha256'], 'size': pin['bytes']}
    path = private_path(LOCAL, 'resolved/official/1.12.2.json')
    fetch(path, spec, LOCAL, records, ROOT / pin['path'])
    official = json.loads(path.read_text())
    require(official['id'] == '1.12.2', 'official client metadata identity differs')
    client = pins['clientJar']
    verify_download(ROOT / client['path'], {'sha256': client['sha256'], 'size': client['bytes']})
    return official, ROOT / client['path']


def resolve_library(row: dict[str, Any], instance: Path, records: dict[str, Any],
                    classifier: str | None = None) -> Path:
    relative = maven_path(row['name'], classifier)
    downloads = row.get('downloads', {})
    spec = downloads.get('classifiers', {}).get(classifier) if classifier else downloads.get('artifact')
    if row.get('MMC-hint') == 'local':
        require(classifier is None, 'unsupported local native classifier: ' + row['name'])
        path = private_path(instance / 'libraries', PurePosixPath(relative).name)
        require(path.is_file(), 'missing bundled Cleanroom library: ' + str(path))
        verify_overlay(path, instance)
        return remember(path, 'instance-overlay:' + relative, LOCAL, records)
    require(spec is not None, 'missing hashed library download: ' + row['name'] + ':' + str(classifier))
    relative = spec.get('path', relative)
    path = private_path(LOCAL / 'prism/libraries', relative)
    return fetch(path, spec, LOCAL, records)


def extract_natives(archive: Path, destination: Path, exclude: Iterable[str] = ()) -> None:
    destination.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(archive) as source:
        for entry in source.infolist():
            name = entry.filename
            target = private_path(destination, name)
            require(not stat.S_ISLNK(entry.external_attr >> 16), 'native archive contains symlink: ' + name)
            if name.upper().startswith('META-INF/') or any(name.startswith(p) for p in exclude):
                continue
            if entry.is_dir():
                target.mkdir(parents=True, exist_ok=True)
            else:
                target.parent.mkdir(parents=True, exist_ok=True)
                with source.open(entry) as incoming, target.open('wb') as out:
                    shutil.copyfileobj(incoming, out)


def resolve_libraries(profile: dict[str, Any], instance: Path,
                      records: dict[str, Any]) -> tuple[list[Path], Path]:
    classpath = []
    natives = private_path(instance, 'natives-direct')
    if natives.exists():
        shutil.rmtree(natives)
    natives.mkdir()
    for row in profile['libraries']:
        if row.get('downloads', {}).get('artifact') or row.get('MMC-hint') == 'local':
            path = resolve_library(row, instance, records)
            if path not in classpath:
                classpath.append(path)
        if 'natives' in row and 'linux' in row['natives']:
            classifier = row['natives']['linux'].replace('${arch}', '64')
            path = resolve_library(row, instance, records, classifier)
            if row['name'].startswith('org.lwjgl:'):
                classpath.append(path)
            extract_natives(path, natives, row.get('extract', {}).get('exclude', []))
    return classpath, natives


def resolve_assets(index: dict[str, Any], game: Path, records: dict[str, Any],
                   assets_root: Path | None = None) -> Path:
    assets = assets_root if assets_root is not None else LOCAL / 'prism/assets'
    path = private_path(assets, 'indexes/' + index['id'] + '.json')
    fetch(path, index, LOCAL, records)
    contents = json.loads(path.read_text())
    objects = {}
    for name, row in contents['objects'].items():
        value = row['hash']
        require(re.fullmatch('[0-9a-f]{40}', value) is not None, 'invalid asset hash')
        objects[value] = row
        private_path(assets, 'virtual/' + index['id'] + '/' + name)
    def acquire(item):
        value, row = item
        relative = value[:2] + '/' + value
        target = private_path(assets, 'objects/' + relative)
        spec = {'url': 'https://resources.download.minecraft.net/' + relative,
                'sha1': value, 'size': row['size']}
        return fetch(target, spec, LOCAL, records)
    with ThreadPoolExecutor(max_workers=8) as workers:
        list(workers.map(acquire, objects.items()))
    for name, row in contents['objects'].items():
        source = assets / 'objects' / row['hash'][:2] / row['hash']
        for destination, enabled in ((assets / 'virtual' / index['id'], contents.get('virtual')),
                                     (game / 'resources', contents.get('map_to_resources'))):
            if enabled:
                target = private_path(destination, name)
                target.parent.mkdir(parents=True, exist_ok=True)
                if not target.exists() or digest(target, 'sha1') != row['hash']:
                    shutil.copyfile(source, target)
    return assets


def java_command(settings: dict[str, Any], profile: dict[str, Any], classpath: list[Path],
                 game: Path, assets: Path, natives: Path, asset_index: str) -> list[str]:
    offline = uuid.UUID(bytes=hashlib.md5(b'OfflinePlayer:BenchOffline').digest(), version=3).hex
    values = {'auth_player_name': 'BenchOffline', 'version_name': '1.12.2',
              'game_directory': str(game), 'assets_root': str(assets),
              'game_assets': str(assets / 'virtual' / asset_index), 'assets_index_name': asset_index,
              'auth_uuid': offline, 'auth_access_token': '0', 'user_type': 'legacy',
              'version_type': 'release', 'user_properties': '{}'}
    arguments = []
    for token in shlex.split(profile['minecraftArguments']):
        for key, value in values.items():
            token = token.replace('${' + key + '}', value)
        require('${' not in token, 'unresolved game argument: ' + token)
        arguments.append(token)
    command = [settings['javaPath'], *settings['jvmArgs'],
               '-Xms' + str(settings['minMemoryMiB']) + 'M',
               '-Xmx' + str(settings['maxMemoryMiB']) + 'M',
               '-Djava.library.path=' + str(natives), '-cp', os.pathsep.join(map(str, classpath)),
               profile['mainClass'], *arguments, '--width', '1280', '--height', '720']
    for tweaker in profile['tweakers']:
        command.extend(('--tweakClass', tweaker))
    return command


def read_settings(instance: str, output: Path) -> dict[str, Any]:
    if instance == 'cleanroom':
        path = output / 'cleanroom-settings.json'
        require(path.is_file(), 'Cleanroom settings missing: run cleanroom.py for this trial first')
        raw = json.loads(path.read_text())
        require(raw.get('status') != 'setup-blocked', 'Cleanroom setup blocked: ' + raw.get('blocker', 'unknown'))
        return {'javaPath': raw['java'], 'jvmArgs': raw['jvm_args'],
                'minMemoryMiB': 4096, 'maxMemoryMiB': 8192}
    path = output / 'settings.json'
    if not path.exists():
        path = LOCAL / 'settings.json'
    settings = json.loads(path.read_text())
    require(settings.get('account') == 'BenchOffline', 'only BenchOffline is supported')
    args = settings['jvmArgs']
    observers = [i for i, value in enumerate(args) if value.startswith('-javaagent:')]
    require(len(observers) == 1, 'original requires exactly one shared observer')
    args[observers[0]] = '-javaagent:' + str((LOCAL / 'observer.jar').resolve()) + '=' + str(output)
    settings['jvmArgs'] = ['-Xloggc:' + str(output / 'gc.log') if value.startswith('-Xloggc:')
                           else value for value in args]
    return settings


def validate_java(java: str, major: int) -> str:
    result = subprocess.run([java, '-version'], capture_output=True, text=True, check=True, timeout=30)
    version = result.stderr + result.stdout
    prefix = '1.8' if major == 8 else str(major)
    require(re.search(r'version "' + re.escape(prefix) + r'(?:[.\-+\"]|$)', version) is not None,
            'launch requires actual Java ' + str(major) + ': ' + version.strip())
    return version.strip()


def candidate_command(candidate: dict[str, Any], game: Path, assets: Path, natives: Path,
                      agent: Path, output: Path) -> list[str]:
    paths = [Path(value).resolve() for value in candidate['classpath']]
    require(paths, 'candidate classpath is empty')
    for path in paths:
        require(path.is_file(), 'candidate classpath missing: ' + str(path))
        require(digest(path) == candidate['sha256'].get(str(path)), 'candidate SHA256 differs: ' + str(path))
    replacements = {'game': game, 'assets': assets, 'natives': natives, 'agent': agent, 'output': output}
    def tokens(key):
        values = candidate[key]
        require(isinstance(values, list) and all(isinstance(v, str) for v in values), key + ' must be token list')
        result = []
        for value in values:
            for name, replacement in replacements.items():
                value = value.replace('{' + name + '}', str(replacement))
            result.append(value)
        return result
    jvm = tokens('jvmArgs')
    for flag, expected in (('-Xms', '-Xms4096M'), ('-Xmx', '-Xmx8192M')):
        supplied = [value for value in jvm if value.startswith(flag)]
        require(not supplied or supplied == [expected],
                'candidate heap must use exactly ' + expected)
        if not supplied:
            jvm.append(expected)
    for value in jvm:
        require(not value.startswith('@') and
                not re.match(r'-XX:[+-]?(?:[^=]*(?:HeapSize|RAM)|AggressiveHeap|Flags|VMOptionsFile)', value),
                'candidate must not override the fixed 4096/8192 MiB heap: ' + value)
    observer = '-javaagent:' + str(agent) + '=' + str(output)
    agents = [v for v in jvm if v.startswith('-javaagent:')]
    require(not agents or agents == [observer], 'candidate must use exactly the shared observer once')
    if not agents:
        jvm.append(observer)
    jvm = [value for value in jvm if not value.startswith('-Dravel.harness.settings=')]
    jvm.append('-Dravel.harness.settings=' + str(HARNESS_SETTINGS))
    if not any(v.startswith('-Xlog:gc') for v in jvm):
        require(':' not in str(output), 'GC output path contains colon')
        jvm.append('-Xlog:gc*,safepoint:file=' + str(output / 'gc.log') + ':time,uptime,level,tags')
    require(isinstance(candidate['mainClass'], str) and candidate['mainClass'], 'candidate mainClass missing')
    return [candidate['javaPath'], *jvm, '-Djava.library.path=' + str(natives), '-cp',
            os.pathsep.join(map(str, paths)), candidate['mainClass'], *tokens('gameArgs')]


def prepare_candidate(candidate: Path | str | None, instance: Path, output: Path,
                      records: dict[str, Any]) -> tuple[list[str], Path, dict[str, Any]]:
    require(candidate is not None, 'ravel requires an explicit real candidate JSON')
    path = Path(candidate).resolve()
    data = json.loads(path.read_text())
    java_version = validate_java(data['javaPath'], 27)
    game = instance / '.minecraft'
    require(game.is_dir(), 'candidate game directory must be prepared first: ' + str(game))
    agent = (LOCAL / 'observer.jar').resolve()
    require(agent.is_file(), 'shared observer missing: ' + str(agent))
    official, _ = official_metadata(records)
    assets = resolve_assets(official['assetIndex'], game, records)
    natives = private_path(instance, 'natives-direct')
    natives.mkdir(exist_ok=True)
    command = candidate_command(data, game, assets, natives, agent, output)
    provenance = {'candidate': str(path), 'candidateSha256': digest(path), 'javaVersion': java_version,
                  'observerSha256': digest(agent), 'inputs': records,
                  'classpath': [{'path': str(Path(p).resolve()), 'sha256': digest(Path(p).resolve())}
                                for p in data['classpath']]}
    return command, game, provenance


def validate_renderer_selection(instance: str, workload: str, renderer: str) -> None:
    require(renderer in ('vanilla', 'geometry-proof'), 'unknown renderer: ' + renderer)
    require(renderer != 'geometry-proof' or (instance == 'ravel' and workload == 'vanilla'),
            'geometry-proof requires --instance ravel --workload vanilla')


def prepare_command(instance: str, output: Path | str,
                    candidate: Path | str | None = None,
                    workload: str = 'sevtech', renderer: str = 'vanilla') -> tuple[list[str], Path]:
    """Finish all resolver I/O, then return (Java argv, .minecraft cwd)."""
    require(workload in ('sevtech', 'vanilla'), 'unknown workload: ' + workload)
    validate_renderer_selection(instance, workload, renderer)
    if instance == 'vanilla' or workload == 'vanilla':
        from vanilla import prepare_command as vanilla_command
        return vanilla_command(Path(output), instance, candidate, renderer=renderer)
    require(instance in ('original', 'cleanroom', 'ravel'), 'unknown instance: ' + instance)
    require(platform.system() == 'Linux' and platform.machine() in ('x86_64', 'amd64'),
            'direct benchmark resolver requires Linux x86_64')
    output = Path(output).resolve()
    require(output.is_relative_to(LOCAL.resolve()), 'trial output must be below private baseline')
    output.mkdir(parents=True, exist_ok=True, mode=0o700)
    directory = private_path(LOCAL, 'prism/instances/' + instance)
    ledger = LOCAL / 'launch-inputs.json'
    records = json.loads(ledger.read_text()) if ledger.exists() else {}
    if instance == 'ravel':
        command, game, provenance = prepare_candidate(candidate, directory, output, records)
    else:
        settings = read_settings(instance, output)
        java_version = validate_java(settings['javaPath'], 8 if instance == 'original' else 27)
        game = directory / '.minecraft'
        require(game.is_dir(), 'prepared game directory missing: ' + str(game))
        components = read_components(directory, records)
        official, client_source = official_metadata(records)
        profile = merge_components(components)
        classpath, natives = resolve_libraries(profile, directory, records)
        if instance == 'original':
            require((natives / 'liblwjgl64.so').is_file(),
                    'exact LWJGL 2 Linux amd64 native liblwjgl64.so is missing')
        main = profile['mainJar']
        client = private_path(LOCAL / 'prism/libraries', maven_path(main['name']))
        fetch(client, official['downloads']['client'], LOCAL, records, client_source)
        classpath.append(client)
        assets = resolve_assets(profile['assetIndex'], game, records)
        if 'logging' in profile:
            logging = profile['logging']
            path = private_path(LOCAL, 'prism/assets/log_configs/' + logging['file']['id'])
            fetch(path, logging['file'], LOCAL, records)
            settings['jvmArgs'] = settings['jvmArgs'] + [logging['argument'].replace('${path}', str(path))]
        command = java_command(settings, profile, classpath, game, assets, natives, profile['assetIndex']['id'])
        provenance = {'javaVersion': java_version,
                      'components': [{'uid': c['uid'], 'version': c['version']} for c in components],
                      'classpath': list(map(str, classpath)), 'inputs': records}
    write_json(ledger, records)
    ledger.chmod(0o600)
    write_json(output / 'command.json', {'argv': command, 'cwd': str(game), 'instance': instance,
                                         'account': 'BenchOffline', 'provenance': provenance})
    (output / 'command.json').chmod(0o600)
    return command, game


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--instance', choices=('original', 'cleanroom', 'ravel', 'vanilla'), default='original')
    parser.add_argument('--workload', choices=('sevtech', 'vanilla'), default='sevtech')
    parser.add_argument('--renderer', choices=('vanilla', 'geometry-proof'), default='vanilla')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--prepare-only', action='store_true', help='explicit; this tool never launches')
    args = parser.parse_args()
    command, game = prepare_command(args.instance, args.output, args.candidate, args.workload, args.renderer)
    print(json.dumps({'command': command, 'cwd': str(game)}, indent=2))


if __name__ == '__main__':
    main()
