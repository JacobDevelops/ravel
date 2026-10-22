#!/usr/bin/env python3
"""Run one native-display comparator trial; invoke under numen run.

Hardware probes and Java tooling are bounded; GPU queries share the explicit
trial deadline. Preparation and matrix lock acquisition remain operator-owned.
"""
import argparse
import csv
from typing import Any
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import time
import shutil
import urllib.request
import zipfile
from shade import relocate_class
from launch import prepare_command, validate_java, validate_renderer_selection
from summarize import validate_ravel_runtime
from preflight import launcher_environment, run_preflight

ROOT = Path(__file__).resolve().parents[3]
LOCAL = ROOT / 'tests/fixtures/local/sevtech-ages-3.2.3/baseline'
ASM_URL = 'https://repo.maven.apache.org/maven2/org/ow2/asm/asm/9.6/asm-9.6.jar'
ASM_SHA256 = '3c6fac2424db3d4a853b669f4e3d1d9c3c552235e19a319673f887083c2303a1'
GEOMETRY_METRICS = (
    'copiedMeshes', 'copiedBytes', 'copyNanos', 'uploads', 'uploadBytes', 'uploadNanos',
    'residentBytes', 'maxResidentBytes', 'stagingBytes', 'maxStagingBytes',
    'submissions', 'submissionNanos', 'refusedMeshes', 'cleanups', 'liveBuffers',
    'liveFramebuffers', 'referenceMismatchPixels', 'vanillaComparisons',
    'vanillaRgbaMismatchPixels', 'vanillaCoverageMismatchPixels', 'vanillaDepthMismatchPixels',
    'generation', 'key',
)
GEOMETRY_FIDELITY = 'geometric-only-no-uv-sprite-tint-ao-baked-light-world-replacement-refused'
BAKED_METRICS = 33
BAKED_REFUSALS = range(20, 29)
PROOF_IMAGE_EDGE = 64
PROOF_IMAGE_PIXELS = PROOF_IMAGE_EDGE ** 2
BAKED_MAX_QUADS_PER_SECTION = 8192
BAKED_RECORD_BYTES = 232
BAKED_QUAD_BYTES = 112
BAKED_NATIVE_SLOT_BYTES = 48 + BAKED_MAX_QUADS_PER_SECTION * (BAKED_RECORD_BYTES + BAKED_QUAD_BYTES)
BAKED_GL_RESERVED_BYTES = 3 * BAKED_MAX_QUADS_PER_SECTION * BAKED_QUAD_BYTES

def validate_shaded_export(data: dict[str, Any], renderer: str, preflight: bool = False) -> None:
    from prepare import require
    native, gl = data.get('shadedNativeResidency'), data.get('shadedGlResidency')
    require(isinstance(native, list) and len(native) == 12 and isinstance(gl, list) and len(gl) == 16
            and all(type(value) is int and value >= 0 for value in native + gl),
            'shaded residency schema differs')
    if renderer == 'vanilla':
        require(not any(native + gl), 'vanilla arm performed shaded residency work')
        return
    if preflight:
        return
    require(not any(native[:2] + native[5:] + gl[:1] + gl[5:14]),
            'shaded residency did not drain at shutdown')
    if renderer == 'resident-world':
        return
    require(0 < native[2] <= 2 * BAKED_NATIVE_SLOT_BYTES and native[3] > 0 and native[4] >= native[3]
            and gl[1] == BAKED_GL_RESERVED_BYTES and gl[2] == 3 * gl[4] > 0
            and gl[3] == native[3] and gl[14] > 0 and gl[15] == 0,
            'shaded reservation, exact acknowledgement, reuse or retirement proof missing')

def validate_baked_export(data: dict[str, Any], renderer: str, preflight: bool = False) -> None:
    from prepare import require
    baked, world = data.get('bakedMetrics'), data.get('bakedWorld')
    require(isinstance(baked, list) and len(baked) == BAKED_METRICS
            and all(type(value) is int for value in baked)
            and isinstance(world, list) and len(world) == 7
            and all(type(value) is int and value >= 0 for value in world),
            'baked metrics schema differs')
    counters = baked[:10] + baked[11:]
    require(baked[10] == -1 and all(value >= 0 for value in counters), 'baked counters must be nonnegative')
    if renderer == 'vanilla':
        require(not any(counters) and not any(world), 'vanilla arm performed baked work')
        return
    if preflight:
        require(baked[5] == 0 and baked[32] == 0 and not any(world[1:4]),
                'preflight baked metrics report a mismatch or refusal')
        return
    require(baked[0] > 0 and baked[4] > 0 and baked[5] == 0 and baked[11] == baked[12] > 0
            and baked[29] == 0 and baked[30] > 0 and baked[31] > 0 and baked[32] == 0,
            'selected arm admitted no complete matching baked comparison or translucent re-sort')
    require(baked[13] > 0 and baked[16] > 0 and 0 < world[0] <= 4 * baked[13]
            and world[4] > 0 and world[5] > 0
            and all(value <= world[0] * PROOF_IMAGE_PIXELS for value in world[1:6]),
            'selected arm drew no shaded baked comparison')
    require(not any(world[1:4]), 'selected shaded baked comparison mismatched pixels')
    require(sum(baked[index] for index in BAKED_REFUSALS) == baked[2],
            'baked refusal reasons do not account for unsupported sections')


def validate_production_shapes(data: dict[str, Any]) -> None:
    from prepare import require
    for name, length in (('draws', 13), ('globalSorts', 3), ('ownedSorts', 3),
                         ('native', 6), ('gl', 5)):
        values = data.get(name)
        require(isinstance(values, list) and len(values) == length
                and all(type(value) is int and value >= 0 for value in values),
                'production counter schema differs: ' + name)

def validate_production_export(out: Path, renderer: str, started_ns: int) -> None:
    from prepare import require
    path = out / 'production-renderer.json'
    require(path.is_file() and path.stat().st_mtime_ns >= started_ns,
            'production counters missing or stale')
    data = json.loads(path.read_text())
    require(data.get('selection') == renderer and data.get('acceptance') is False
            and data.get('validationOverride') is (renderer == 'resident-world'),
            'production selection or acceptance mode differs')
    validate_production_shapes(data)
    if renderer != 'resident-world':
        require(not any(data['draws'] + data['globalSorts'] + data['ownedSorts']),
                'unselected arm performed production work')
        return
    require(all(value > 0 for value in data['draws'][:4]),
            'selected arm has zero production replacements in a non-fluid layer')
    require(0 < data['draws'][9] <= data['draws'][8]
            and all(value > 0 for value in data['globalSorts'][:2] + data['ownedSorts'][:2])
            and all(value <= total for value, total in zip(data['ownedSorts'], data['globalSorts'])),
            'production frame or sort/upload counters differ')
    require(0 < data['native'][4] <= data['native'][5] <= data['native'][0]
            and 0 < data['gl'][1] <= data['gl'][2] <= data['gl'][0],
            'production resident peaks exceed budget')

def validate_geometry_export(out: Path, renderer: str, started_ns: int, *,
                             preflight: bool = False) -> None:
    from prepare import require
    path = out / 'geometry-upload.json'
    require(path.is_file(), 'geometry metrics export missing')
    require(path.stat().st_mtime_ns >= started_ns, 'geometry metrics export stale')
    data = json.loads(path.read_text())
    require(isinstance(data, dict) and type(data.get('schemaVersion')) is int
            and data['schemaVersion'] == 1, 'geometry metrics schema differs')
    require(data.get('selection') == renderer, 'geometry metrics selection differs')
    require(data.get('fidelity') == ('bounded-opaque-world-replacement-parity-unproven'
                                    if renderer == 'resident-world' else GEOMETRY_FIDELITY)
            and data.get('upload') == 'bounded-native-wire-java-owned-offscreen-gl',
            'geometry metrics fidelity/upload contract differs')
    metrics = data.get('metrics')
    require(isinstance(metrics, dict) and set(metrics) == set(GEOMETRY_METRICS),
            'geometry metrics fields differ')
    require(all(type(value) is int and value >= 0 for value in metrics.values()),
            'geometry metrics must be nonnegative integers')
    validate_baked_export(data, renderer, preflight=preflight or renderer == 'resident-world')
    validate_shaded_export(data, renderer, preflight=preflight)
    if renderer == 'resident-world':
        require(data.get('backend') in ('OpenGL15', 'ARB_vertex_buffer_object'),
                'production geometry backend differs')
        budget = data.get('shadedNativeBudget')
        require(isinstance(budget, list) and len(budget) == 6
                and all(type(value) is int and value >= 0 for value in budget),
                'production native budget schema differs')
        require(isinstance(data.get('productionWorldDraw'), list)
                and len(data['productionWorldDraw']) == 13
                and all(type(value) is int and value >= 0 for value in data['productionWorldDraw']),
                'production draw schema differs')
        return
    if preflight:
        require(data.get('backend') in ('selected-not-initialized', 'OpenGL15', 'ARB_vertex_buffer_object'),
                'preflight geometry backend differs')
        return
    if renderer == 'vanilla':
        require(data.get('backend') == 'not-selected' and not any(metrics.values()),
                'vanilla arm performed geometry work')
        return
    require(data.get('backend') in ('OpenGL15', 'ARB_vertex_buffer_object'),
            'geometry backend unsupported or unselected')
    require(all(metrics[name] > 0 for name in
                ('copiedMeshes', 'copiedBytes', 'uploads', 'uploadBytes', 'submissions', 'cleanups')),
            'geometry proof performed no copy/upload/submission/cleanup work')
    require(0 < metrics['maxResidentBytes'] <= 1_179_648
            and 0 < metrics['maxStagingBytes'] <= 1_212_416,
            'geometry residency/staging exceeded bounded capacity')
    require(not any(metrics[name] for name in
                    ('residentBytes', 'stagingBytes', 'liveBuffers', 'liveFramebuffers', 'generation', 'key')),
            'geometry cleanup retained residency or live objects')

def pinned_asm(version: str, checksum: str) -> Path:
    asm = LOCAL / ('asm-' + version + '.jar')
    url = ASM_URL.replace('/9.6/asm-9.6.jar', '/' + version + '/asm-' + version + '.jar')
    if not asm.exists():
        urllib.request.urlretrieve(url, asm)
    digest = hashlib.sha256(asm.read_bytes()).hexdigest()
    if digest != checksum:
        raise ValueError('observer dependency SHA256 differs')
    pin = LOCAL / ('observer-inputs-' + version + '.json')
    record = {'url': url, 'sha256': digest}
    if pin.exists() and json.loads(pin.read_text()) != record:
        raise ValueError('observer dependency changed')
    pin.write_text(json.dumps(record, indent=2) + '\n')
    return asm


def verify_observer(classes: Path, asm: Path, java_home: str, agent: Path) -> None:
    tests = LOCAL / 'observer-test-classes'
    tests.mkdir(exist_ok=True)
    classpath = os.pathsep.join(map(str, [classes, asm]))
    subprocess.run([str(Path(java_home) / 'bin/javac'), '-Xlint:all', '-Werror',
                    '-cp', classpath, '-d', str(tests),
                    str(Path(__file__).with_name('BenchmarkAgentTest.java'))],
                   check=True, timeout=300)
    current = subprocess.check_output(['mise', 'where', 'java'], text=True, timeout=60).strip()
    java27 = str(Path(current) / 'bin/java')
    validate_java(java27, 27)
    classpath = os.pathsep.join(map(str, [tests, agent, asm]))
    for java in (str(Path(java_home) / 'bin/java'), java27):
        subprocess.run([java, '-Xverify:all', '-cp', classpath,
                        'dev.ravel.metrics.BenchmarkAgentTest'], check=True, timeout=60)


def build() -> Path:
    LOCAL.mkdir(parents=True, exist_ok=True, mode=0o700)
    asm = pinned_asm('9.6', ASM_SHA256)
    runtime = pinned_asm('9.9.1', '6f3828a215c920059a5efa2fb55c233d6c54ec5cadca99ce1b1bdd10077c7ddd')
    classes = LOCAL / 'observer-classes'
    if classes.exists():
        shutil.rmtree(classes)
    classes.mkdir(exist_ok=True)
    java_home = subprocess.check_output(['mise', 'where', 'java@temurin-8.0.504+1'],
                                        text=True, timeout=60).strip()
    subprocess.run([str(Path(java_home) / 'bin/javac'), '-Xlint:all', '-Werror', '-cp', str(asm),
                    '-d', str(classes), str(Path(__file__).with_name('BenchmarkAgent.java'))],
                   check=True, timeout=300)
    agent = LOCAL / 'observer.jar'
    with zipfile.ZipFile(agent, 'w', zipfile.ZIP_DEFLATED) as out:
        out.writestr(zipfile.ZipInfo('META-INF/MANIFEST.MF'),
                     'Manifest-Version: 1.0\nPremain-Class: dev.ravel.metrics.BenchmarkAgent\n\n')
        for path in sorted(classes.rglob('*.class')):
            out.writestr(zipfile.ZipInfo(str(path.relative_to(classes))), relocate_class(path.read_bytes()))
        with zipfile.ZipFile(runtime) as source:
            for name in source.namelist():
                if name.startswith('org/') and name.endswith('.class'):
                    out.writestr(zipfile.ZipInfo(name.replace('org/objectweb/asm', 'ravel/metrics/asm')),
                                 relocate_class(source.read(name)))
    verify_observer(classes, asm, java_home, agent)
    return agent


def java_process(directory: Path) -> Path | None:
    expected = f'-javaagent:{(LOCAL / "observer.jar").resolve()}={directory.resolve()}'.encode()
    for proc in Path('/proc').iterdir():
        if not proc.name.isdigit():
            continue
        try:
            args = (proc / 'cmdline').read_bytes().split(b'\0')
            if expected in args:
                return proc
        except (FileNotFoundError, PermissionError, ProcessLookupError):
            continue
    return None


def sample(proc: Path) -> list[int | str]:
    status = dict(line.split(':', 1) for line in (proc / 'status').read_text().splitlines() if ':' in line)
    stat = (proc / 'stat').read_text().rsplit(')', 1)[1].split()
    return [time.monotonic_ns(), proc.name, int(status['VmRSS'].split()[0]),
            int(status['VmHWM'].split()[0]), int(stat[11]) + int(stat[12])]


def validate_renderer(identity: str, functional_only: bool) -> None:
    if not functional_only and any(name in identity.lower() for name in
                                   ('llvmpipe', 'softpipe', 'software rasterizer')):
        raise ValueError('software renderer is functional-only')


def hardware(out: Path, functional_only: bool = False) -> None:
    commands = {'gpu': ['nvidia-smi', '--query-gpu=name,driver_version,memory.total', '--format=csv'],
                'gl': ['glxinfo', '-B'], 'display': ['xrandr', '--current'],
                'cpu': ['lscpu'], 'kernel': ['uname', '-srmo']}
    for name, command in commands.items():
        result = subprocess.run(command, check=True, capture_output=True, text=True, timeout=60)
        (out / (name + '.txt')).write_text(result.stdout)
        if name == 'gl':
            validate_renderer(result.stdout, functional_only)


def evict_inputs(instance: Path, out: Path) -> None:
    paths = list((instance / '.minecraft/mods').glob('*.jar'))
    paths += list((LOCAL / 'prism/libraries').rglob('*.jar'))
    command = json.loads((out / 'command.json').read_text())['argv']
    paths += [Path(path) for path in command[command.index('-cp') + 1].split(os.pathsep)]
    paths = sorted(set(path.resolve() for path in paths))
    evicted = []
    os.sync()
    for path in paths:
        with path.open('rb') as source:
            os.posix_fadvise(source.fileno(), 0, 0, os.POSIX_FADV_DONTNEED)
        evicted.append({'name': path.name, 'bytes': path.stat().st_size})
    (out / 'cache-advice.json').write_text(json.dumps(evicted, indent=2) + '\n')


def validate_display(functional_only: bool) -> None:
    if not os.environ.get('DISPLAY'):
        raise ValueError('benchmark requires DISPLAY')
    if not functional_only:
        if os.environ['DISPLAY'] != ':0':
            raise ValueError('native measurement requires DISPLAY=:0')
        if os.environ.get('LIBGL_ALWAYS_SOFTWARE', '').lower() in ('1', 'true', 'yes'):
            raise ValueError('software rendering is functional-only')


def run(args: argparse.Namespace) -> None:
    validate_renderer_selection(args.instance, args.workload, args.renderer)
    validate_display(args.functional_only)
    out = LOCAL / 'runs' / args.trial
    out.mkdir(parents=True, mode=0o700)
    if args.workload == 'vanilla':
        from vanilla import prepare_vanilla
        prepare_vanilla(out, args.instance, args.candidate)
    elif args.instance == 'ravel':
        prepare_candidate(out)
    else:
        prepare = 'prepare.py' if args.instance == 'original' else 'cleanroom.py'
        subprocess.run(['python3', str(Path(__file__).with_name(prepare)),
                        '--agent', str(LOCAL / 'observer.jar'), '--output', str(out)], check=True)
    command, cwd = prepare_command(args.instance, out, args.candidate, workload=args.workload,
                                   renderer=args.renderer)
    instance = cwd.parent
    game_log = cwd / 'logs/latest.log'
    game_log.unlink(missing_ok=True)
    world = cwd / 'saves/RavelBenchmark'
    if world.exists():
        shutil.rmtree(world)
    if args.cache == 'cold-data-cache':
        evict_inputs(instance, out)
    hardware(out, args.functional_only)
    environment = launcher_environment()
    start = time.monotonic_ns()
    started_ns = time.time_ns()
    with (out / 'launcher.log').open('w') as log, (out / 'process.csv').open('w') as raw:
        process = subprocess.Popen(command, cwd=cwd, env=environment, stdout=log,
                                   stderr=subprocess.STDOUT, start_new_session=True)
        writer = csv.writer(raw)
        writer.writerow(['monotonic_ns', 'pid', 'rss_kib', 'peak_rss_kib', 'cpu_ticks'])
        failure = None
        try:
            monitor(process, writer, out, args.timeout)
        except (OSError, subprocess.SubprocessError, TimeoutError) as error:
            failure = str(error)
        finally:
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGTERM)
                try:
                    process.wait(timeout=30)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait()
    if args.instance == 'cleanroom':
        update_runtime_status(out, process.returncode)
    if game_log.exists():
        shutil.copyfile(game_log, out / 'game.log')
    result = {'trial': args.trial, 'instance': args.instance, 'cache': args.cache,
              'renderer': args.renderer,
              'start_monotonic_ns': start, 'wall_seconds': (time.monotonic_ns()-start)/1e9,
              'launcher_exit': process.returncode, 'complete': (out / 'complete').exists(),
              'monitor_error': failure, 'audio_driver': environment['ALSOFT_DRIVERS'],
              'measurement': 'functional-only' if args.functional_only else 'physical',
              'display': os.environ['DISPLAY']}
    (out / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
    if not result['complete'] or process.returncode != 0 or (out / 'error.txt').exists():
        raise RuntimeError('trial did not complete successfully; inspect private trial logs')
    if args.instance == 'ravel' and args.workload == 'vanilla':
        validate_ravel_runtime(out)
        validate_geometry_export(out, args.renderer, started_ns)
        if args.renderer in ('vanilla', 'resident-world'):
            validate_production_export(out, args.renderer, started_ns)


def monitor(process: subprocess.Popen, writer: Any, out: Path, timeout: float) -> None:
    deadline = time.monotonic() + timeout
    observed_jvm = False
    with (out / 'gpu.csv').open('w') as gpu:
        gpu.write('monotonic_ns,used_vram_mib,gpu_percent\n')
        while time.monotonic() < deadline:
            proc = java_process(out)
            if proc is not None:
                observed_jvm = True
                try:
                    writer.writerow(sample(proc))
                except (FileNotFoundError, ProcessLookupError, KeyError):
                    pass
            elif observed_jvm:
                return
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                break
            result = subprocess.run(['nvidia-smi', '--id=0', '--query-gpu=memory.used,utilization.gpu',
                                     '--format=csv,noheader,nounits'], capture_output=True, text=True,
                                    check=True, timeout=remaining)
            gpu.write(str(time.monotonic_ns()) + ',' + result.stdout.strip() + '\n')
            gpu.flush()
            if (out / 'complete').exists() or (out / 'error.txt').exists():
                return
            if process.poll() is not None:
                return
            time.sleep(1)
    raise TimeoutError('benchmark exceeded explicit trial deadline')


def update_runtime_status(out: Path, launcher_exit: int | None) -> None:
    path = out / 'cleanroom-settings.json'
    if path.exists():
        settings = json.loads(path.read_text())
        settings['status'] = 'complete' if (out / 'complete').exists() else 'runtime-incomplete'
        settings['runtime_observed'] = (out / 'events.csv').exists()
        settings['launcher_exit'] = launcher_exit
        path.write_text(json.dumps(settings, indent=2) + '\n')

def prepare_candidate(out: Path) -> None:
    import prepare
    fixture = LOCAL.parent
    archive, artifacts, pins = prepare.verify_inputs(ROOT, fixture)
    prism = LOCAL / 'prism'
    prism.mkdir(parents=True, exist_ok=True)
    prepare.configure_caches(prism, fixture.resolve().parent / ('prism-' + 'approot-headless'))
    prepare.pin_downloads(LOCAL, prism, pins)
    instance = prepare.prepare_instance(prism, archive, artifacts, 'ravel')
    settings = {'fixture': prepare.FIXTURE, 'instance': 'ravel',
                'minMemoryMiB': 4096, 'maxMemoryMiB': 8192,
                'options': prepare.configure_options(instance / '.minecraft'),
                'artifactCount': len(artifacts),
                'agentSha256': prepare.digest(LOCAL / 'observer.jar')}
    prepare.write_json(out / 'settings.json', settings)



def matrix(args: argparse.Namespace) -> None:
    validate_renderer_selection(args.instance, args.workload, args.renderer)
    if '/' in args.matrix or args.matrix in ('.', '..'):
        raise ValueError('matrix prefix must be a simple identifier')
    paired = args.renderer != 'vanilla'
    caches = ('warm-filesystem',) if args.workload == 'vanilla' else ('cold-data-cache', 'warm-filesystem')
    renderers = ('vanilla', args.renderer) if paired else (args.renderer,)
    schedule = [{'trial': f'{args.matrix}-{cache}-{repetition}'
                 + (f'-{renderer}' if paired else ''), 'cache': cache, 'renderer': renderer}
                for repetition in range(1, 4) for cache in caches for renderer in renderers]
    out = LOCAL / 'runs' / args.matrix
    out.mkdir(parents=True, mode=0o700)
    record = {'order': 'off-on-alternating' if paired else 'single-arm', 'trials': schedule,
              'preflight': 'pending' if paired else 'not-applicable'}
    path = out / 'matrix.json'
    path.write_text(json.dumps(record, indent=2) + '\n')
    base = ['mise', 'exec', '--', 'numen', 'run', '--', 'python3', str(Path(__file__))]
    if paired:
        subprocess.run(base + [args.matrix + '-preflight', '--preflight', '--instance', 'ravel',
                               '--workload', 'vanilla', '--renderer', args.renderer,
                               '--candidate', str(args.candidate.resolve())],
                       check=True)
        record['preflight'] = 'passed'
        path.write_text(json.dumps(record, indent=2) + '\n')
    for trial in schedule:
        command = base + [trial['trial'], '--cache', trial['cache'], '--instance', args.instance,
                          '--timeout', str(args.timeout), '--workload', args.workload,
                          '--renderer', trial['renderer']]
        if args.candidate:
            command += ['--candidate', str(args.candidate.resolve())]
        subprocess.run(command, check=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('trial', nargs='?')
    parser.add_argument('--build', action='store_true')
    parser.add_argument('--preflight', action='store_true',
                        help='run bounded selected-renderer scene and silence checks')
    parser.add_argument('--cache', choices=['cold-data-cache', 'warm-filesystem'], default='warm-filesystem')
    parser.add_argument('--instance', choices=['original', 'cleanroom', 'ravel', 'vanilla'], default='original')
    parser.add_argument('--workload', choices=['sevtech', 'vanilla'], default='sevtech')
    parser.add_argument('--renderer', choices=['vanilla', 'geometry-proof', 'resident-world'], default='vanilla',
                        help='selected renderers are opt-in for the Ravel vanilla workload only')
    parser.add_argument('--functional-only', action='store_true',
                        help='allow a nonphysical display; never include in performance comparisons')
    parser.add_argument('--candidate', type=Path, help='private Ravel direct launch specification JSON')
    parser.add_argument('--timeout', type=int, default=1800)
    parser.add_argument('--matrix', metavar='PREFIX', help='run three separately locked cold/warm pairs')
    args = parser.parse_args()
    if args.instance == 'vanilla':
        args.workload = 'vanilla'
    try:
        validate_renderer_selection(args.instance, args.workload, args.renderer)
    except ValueError as error:
        parser.error(str(error))
    if args.workload == 'vanilla' and args.instance not in ('vanilla', 'ravel'):
        parser.error('vanilla workload requires --instance vanilla or ravel')
    if args.functional_only and args.matrix:
        parser.error('functional-only runs are individual trials, not a performance matrix')
    if (args.instance == 'ravel') != bool(args.candidate):
        parser.error('--candidate is required exactly for --instance ravel')
    if args.preflight:
        run_preflight(args.candidate, args.trial or 'renderer-preflight',
                      renderer='geometry-proof' if args.renderer == 'vanilla' else args.renderer)
    elif args.build:
        build()
    elif args.matrix:
        matrix(args)
    elif args.trial and '/' not in args.trial and args.trial not in ('.', '..'):
        run(args)
    else:
        parser.error('supply --build or a fresh trial identifier')


if __name__ == '__main__':
    main()
