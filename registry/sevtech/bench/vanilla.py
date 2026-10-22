#!/usr/bin/env python3
"""Prepare matched official 1.12.2 clients, without Forge or pack mods.

Generate the private Ravel runtime pin after building the generated legacy agent
and host: vanilla.py --write-candidate PATH --java /jdk27/bin/java.
Run launch.py --instance vanilla|ravel --workload vanilla --output OUT,
adding --candidate PATH for Ravel. Resolution finishes before measurement.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import zipfile
from typing import Any

import launch
from prepare import HARNESS_SETTINGS, JAVA, configure_options, digest, require, write_json

MODULES = 'org.lwjgl,org.lwjgl.glfw,org.lwjgl.opengl,org.lwjgl.openal'
FIELDS = {'javaPath', 'platform', 'agent', 'host', 'screen', 'lwjgl', 'sha256'}


def snapshot(source: Path) -> Path:
    source = source.resolve()
    require(source.is_file(), 'missing runtime input: ' + str(source))
    value = digest(source)
    target = launch.private_path(launch.LOCAL, 'resolved/vanilla-runtime/' + value + '/' + source.name)
    target.parent.mkdir(parents=True, exist_ok=True)
    if not target.exists():
        shutil.copyfile(source, target)
    require(digest(target) == value, 'retained runtime SHA256 differs: ' + str(target))
    return target.resolve()


def validate_host(path: Path) -> None:
    with path.open('rb') as stream:
        require(stream.read(4) == b'\x7fELF', 'candidate host must be an ELF shared library')
    try:
        result = subprocess.run(['nm', '-D', '--defined-only', '--format=posix', str(path)],
                                capture_output=True, text=True, check=True, timeout=30)
    except (OSError, subprocess.SubprocessError) as error:
        raise ValueError('cannot inspect candidate host exports: ' + str(path)) from error
    exports = {line.split()[0] for line in result.stdout.splitlines() if line.strip()}
    require('Java_dev_ravel_jvm_LegacyPacketLive_receipts' not in exports,
            'vanilla candidate host contains fixture-mods exports')
    functions = {fields[0] for line in result.stdout.splitlines()
                 if len(fields := line.split()) >= 2 and fields[1] in ('T', 'W')}
    require({'Java_dev_ravel_jvm_Hook_ravelInitialize',
             'Java_dev_ravel_jvm_Hook_ravelInitializeIntegratedServer',
             'Java_dev_ravel_jvm_Hook_legacyBootstrap'} <= functions,
            'candidate host missing legacy composed-host JNI exports')


def validate_candidate(data: dict[str, Any]) -> list[Path]:
    require(set(data) == FIELDS, 'vanilla candidate fields must be exactly ' + ', '.join(sorted(FIELDS)))
    require(isinstance(data['javaPath'], str) and data['javaPath'], 'candidate javaPath missing')
    require(isinstance(data['lwjgl'], list) and data['lwjgl'], 'candidate LWJGL inputs missing')
    paths = [Path(data[key]).resolve() for key in ('platform', 'agent', 'host', 'screen')]
    paths += [Path(value).resolve() for value in data['lwjgl']]
    require(len(paths) == len(set(paths)), 'duplicate candidate runtime input')
    require(set(data['sha256']) == set(map(str, paths)), 'candidate SHA256 input set differs')
    for path in paths:
        require(path.is_file() and digest(path) == data['sha256'][str(path)],
                'candidate SHA256 differs: ' + str(path))
    validate_host(Path(data['host']).resolve())
    return paths


def lwjgl_pins() -> dict[str, str]:
    pins = {}
    for line in (launch.ROOT / 'jvm/legacy-platform/dependencies.sha256').read_text().splitlines():
        if line.strip() and not line.startswith('#'):
            checksum, filename = line.split()
            pins[filename] = checksum
    return pins


def write_candidate(path: Path, java: Path, host: Path | None = None) -> None:
    require(path.resolve().is_relative_to(launch.LOCAL.resolve()), 'candidate must remain private')
    work = launch.ROOT / 'tests/fixtures/local/legacy-platform'
    lwjgl = []
    for filename, checksum in lwjgl_pins().items():
        source = work / filename
        require(source.is_file() and digest(source) == checksum, 'pinned LWJGL input differs: ' + filename)
        lwjgl.append(str(snapshot(source)))
    data = {'javaPath': str(java.resolve()), 'platform': str(snapshot(work / 'platform.jar')),
            'agent': str(snapshot(launch.ROOT / 'target/ravel-jvm/ravel-agent.jar')),
            'host': str(snapshot(host if host is not None else launch.ROOT / 'target/release/libravel_bootstrap.so')),
            'screen': str(snapshot(launch.ROOT / 'target/ravel-jvm/ravel-screen.jar')),
            'lwjgl': lwjgl}
    inputs = [data[key] for key in ('platform', 'agent', 'host', 'screen')] + lwjgl
    data['sha256'] = {value: digest(Path(value)) for value in inputs}
    validate_candidate(data)
    validate_agent(Path(data['agent']))
    launch.validate_java(data['javaPath'], 27)
    path.parent.mkdir(parents=True, exist_ok=True)
    require(not path.exists(), 'candidate already exists; use a new cohort path')
    write_json(path, data)
    path.chmod(0o600)


def validate_agent(path: Path) -> None:
    with zipfile.ZipFile(path) as archive:
        manifest = archive.read('META-INF/MANIFEST.MF').decode().splitlines()
        require('Ravel-Legacy-Adapter: true' in manifest and
                'Automatic-Module-Name: dev.ravel.agent' in manifest,
                'requires generated legacy dev.ravel.agent')
        require({'dev/ravel/jvm/Hook.class', 'dev/ravel/jvm/LegacyRuntime.class'} <= set(archive.namelist()),
                'generated legacy agent classes missing')


def instance_directory(instance: str) -> Path:
    name = 'vanilla-java8' if instance == 'vanilla' else 'vanilla-ravel'
    return launch.private_path(launch.LOCAL, 'prism/instances/' + name)


def retain_agent_bundle(agent: Path, screen: Path) -> tuple[Path, Path]:
    bundle = launch.private_path(launch.LOCAL, 'resolved/vanilla-runtime/agent-' + digest(agent) + '-' + digest(screen))
    bundle.mkdir(parents=True, exist_ok=True)
    retained = []
    for artifact, name in ((agent, 'ravel-agent.jar'), (screen, 'ravel-screen.jar')):
        target = launch.private_path(bundle, name)
        if not target.exists():
            shutil.copyfile(artifact, target)
        require(digest(target) == digest(artifact), 'retained agent bundle SHA256 differs: ' + str(target))
        retained.append(target)
    return retained[0], retained[1]


def ravel_command(candidate: Path | str | None, settings: dict[str, Any], directory: Path,
                  out: Path, observer: Path, classpath: list[Path], natives: Path,
                  args: list[str], renderer: str = 'vanilla') -> tuple[list[str], list[Path], dict[str, Any]]:
    launch.validate_renderer_selection('ravel', 'vanilla', renderer)
    require(candidate is not None, 'ravel requires an explicit real candidate JSON')
    source = Path(candidate).resolve()
    data = json.loads(source.read_text())
    paths = validate_candidate(data)
    require(data['javaPath'] == settings['javaPath'], 'candidate Java differs from prepared settings')
    retained = [snapshot(path) for path in paths]
    platform, agent, host, screen, *lwjgl = retained
    retained[1], retained[3] = retain_agent_bundle(agent, screen)
    agent = retained[1]
    validate_agent(agent)
    modules = [path for path in lwjgl if '-natives-' not in path.name]
    native_jars = [path for path in lwjgl if '-natives-' in path.name]
    require({path.name: digest(path) for path in lwjgl} == lwjgl_pins(), 'exact pinned LWJGL3 runtime set differs')
    child = launch.private_path(directory, 'benchmark-child-classpath.txt')
    child.write_text(''.join(str(path) + '\n' for path in classpath))
    logging_args = [value for value in settings['jvmArgs'] if value.startswith('-Dlog4j')]
    renderer_args = ['-Dravel.renderer=' + renderer,
                     '-Dravel.renderer.metrics=' + str(out.resolve() / 'geometry-upload.json')]
    if renderer == 'resident-world':
        renderer_args.append('-Dravel.renderer.validation=true')
    command = [data['javaPath'], '-Xms4096M', '-Xmx8192M', '-XX:+UseG1GC',
               '--module-path', os.pathsep.join(map(str, [agent, *modules])),
               '--add-modules', MODULES + ',dev.ravel.agent', '--illegal-native-access=deny',
               '--enable-native-access=' + MODULES + ',dev.ravel.agent',
               '-javaagent:' + str(agent), '-Dravel.host=' + str(host),
               '-Dravel.report.targets=true', '-Dravel.report.loader=true',
               '-Dravel.harness.settings=' + str(HARNESS_SETTINGS),
               '-Dravel.legacy.benchmark=true', *renderer_args,
               '-javaagent:' + str(observer) + '=' + str(out),
               '-Xlog:gc*,safepoint:file=' + str(out / 'gc.log') + ':time,uptime,level,tags',
               '-Djava.library.path=' + str(natives), *logging_args,
               '-cp', os.pathsep.join(map(str, [platform, *native_jars])),
               'dev.ravel.legacy.launch.LegacyLauncher', 'client', str(child), str(out), *args]
    provenance = {'candidate': str(source), 'candidateSha256': digest(source),
                  'childClasspath': list(map(str, classpath)),
                  'renderer': renderer, 'rendererJvmArgs': renderer_args}
    return command, retained + [child, source], provenance


def prepare_vanilla(out: Path, instance: str, candidate: Path | str | None = None) -> None:
    out = Path(out).resolve()
    require(instance in ('vanilla', 'ravel'), 'vanilla workload requires vanilla or ravel instance')
    require(out.is_relative_to(launch.LOCAL.resolve()), 'trial output must be below private baseline')
    out.mkdir(parents=True, exist_ok=True, mode=0o700)
    directory = instance_directory(instance)
    game = launch.private_path(directory, '.minecraft')
    game.mkdir(parents=True, exist_ok=True)
    require(not (game / 'mods').exists(), 'vanilla workload must not contain mods')
    require(not (directory / 'mmc-pack.json').exists(), 'vanilla workload must not reuse a pack instance')
    observer = launch.LOCAL / 'observer.jar'
    require(observer.is_file(), 'shared observer missing')
    if instance == 'vanilla':
        home = subprocess.check_output(['mise', 'where', JAVA], text=True, timeout=60).strip()
        java = str(Path(home) / 'bin/java')
        jvm = ['-XX:+UseG1GC', '-javaagent:' + str(observer.resolve()) + '=' + str(out),
               '-Xloggc:' + str(out / 'gc.log'), '-XX:+PrintGCDetails',
               '-XX:+PrintGCDateStamps', '-XX:+PrintGCTimeStamps']
    else:
        require(candidate is not None, 'ravel requires an explicit real candidate JSON')
        data = json.loads(Path(candidate).read_text())
        validate_candidate(data)
        java, jvm = data['javaPath'], []
    settings = {'fixture': 'vanilla-1.12.2', 'instance': instance, 'account': 'BenchOffline',
                'javaPath': java, 'jvmArgs': jvm, 'minMemoryMiB': 4096, 'maxMemoryMiB': 8192,
                'artifactCount': 0, 'agentSha256': digest(observer), 'rendererProbes': True,
                'options': configure_options(game), 'output': str(out)}
    write_json(out / 'settings.json', settings)


def prepare_command(out: Path, instance: str, candidate: Path | str | None = None,
                    renderer: str = 'vanilla') -> tuple[list[str], Path]:
    launch.validate_renderer_selection(instance, 'vanilla', renderer)
    out = Path(out).resolve()
    require(instance in ('vanilla', 'ravel'), 'vanilla workload requires vanilla or ravel instance')
    require(out.is_relative_to(launch.LOCAL.resolve()), 'trial output must be below private baseline')
    require(launch.platform.system() == 'Linux' and launch.platform.machine() in ('x86_64', 'amd64'),
            'direct benchmark resolver requires Linux x86_64')
    for name in ('JAVA_TOOL_OPTIONS', '_JAVA_OPTIONS', 'JDK_JAVA_OPTIONS', 'CLASSPATH', 'LD_PRELOAD'):
        require(not os.environ.get(name), 'unpinned runtime environment: ' + name)
    directory = instance_directory(instance)
    game = directory / '.minecraft'
    settings = json.loads((out / 'settings.json').read_text())
    require(settings['fixture'] == 'vanilla-1.12.2' and settings['instance'] == instance,
            'vanilla preparation settings differ')
    require(not (game / 'mods').exists(), 'vanilla workload must not contain mods')
    observer = (launch.LOCAL / 'observer.jar').resolve()
    require(digest(observer) == settings['agentSha256'], 'shared observer SHA256 differs')
    ledger = launch.LOCAL / 'launch-inputs.json'
    records = json.loads(ledger.read_text()) if ledger.exists() else {}
    official, client = launch.official_metadata(records)
    profile = dict(official, tweakers=[])
    profile['libraries'] = [row for row in official['libraries'] if launch.allowed(row) and
                            (instance == 'vanilla' or not row['name'].startswith(('org.lwjgl:', 'org.lwjgl.lwjgl:')))]
    classpath, natives = launch.resolve_libraries(profile, directory, records)
    classpath.append(snapshot(client))
    assets = launch.resolve_assets(official['assetIndex'], game, records,
                                   launch.private_path(launch.LOCAL, 'resolved/vanilla-assets'))
    java_version = launch.validate_java(settings['javaPath'], 8 if instance == 'vanilla' else 27)
    if instance == 'vanilla':
        require((natives / 'liblwjgl64.so').is_file(), 'official LWJGL2 native missing')
    if 'logging' in official:
        logging = official['logging']['client']
        config = launch.private_path(assets, 'log_configs/' + logging['file']['id'])
        launch.fetch(config, logging['file'], launch.LOCAL, records)
        settings['jvmArgs'].append(logging['argument'].replace('${path}', str(config)))
    command = launch.java_command(settings, profile, classpath, game, assets, natives, official['assetIndex']['id'])
    runtime = classpath + [observer, Path(settings['javaPath']).resolve()]
    provenance = {'javaVersion': java_version, 'inputs': records}
    if instance == 'ravel':
        args = command[command.index(profile['mainClass']) + 1:]
        command, extra, candidate_provenance = ravel_command(candidate, settings, directory, out,
                                                            observer, classpath, natives, args, renderer=renderer)
        runtime += extra
        provenance.update(candidate_provenance)
    runtime += [path for path in natives.rglob('*') if path.is_file()]
    provenance['runtimeInputs'] = [{'path': str(path), 'sha256': digest(path)} for path in runtime]
    provenance['classpath'] = [{'path': path, 'sha256': digest(Path(path))}
                               for path in command[command.index('-cp') + 1].split(os.pathsep)]
    write_json(ledger, records)
    ledger.chmod(0o600)
    write_json(out / 'command.json', {'argv': command, 'cwd': str(game), 'instance': instance,
                                     'workload': 'vanilla', 'account': 'BenchOffline', 'provenance': provenance})
    (out / 'command.json').chmod(0o600)
    return command, game


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write-candidate', type=Path)
    parser.add_argument('--java', type=Path)
    parser.add_argument('--host', type=Path, help='explicit composed native host to retain in the candidate')
    parser.add_argument('--output', type=Path)
    parser.add_argument('--instance', choices=('vanilla', 'ravel'), default='vanilla')
    parser.add_argument('--candidate', type=Path)
    args = parser.parse_args()
    if args.write_candidate:
        require(args.java is not None, '--write-candidate requires --java')
        write_candidate(args.write_candidate, args.java, args.host)
    else:
        require(args.output is not None, '--output is required')
        prepare_vanilla(args.output, args.instance, args.candidate)


if __name__ == '__main__':
    main()
