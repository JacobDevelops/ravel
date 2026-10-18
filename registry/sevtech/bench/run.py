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
from launch import prepare_command

ROOT = Path(__file__).resolve().parents[3]
LOCAL = ROOT / 'tests/fixtures/local/sevtech-ages-3.2.3/baseline'
ASM_URL = 'https://repo.maven.apache.org/maven2/org/ow2/asm/asm/9.6/asm-9.6.jar'
ASM_SHA256 = '3c6fac2424db3d4a853b669f4e3d1d9c3c552235e19a319673f887083c2303a1'


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


def hardware(out: Path) -> None:
    commands = {'gpu': ['nvidia-smi', '--query-gpu=name,driver_version,memory.total', '--format=csv'],
                'gl': ['glxinfo', '-B'], 'cpu': ['lscpu'], 'kernel': ['uname', '-srmo']}
    for name, command in commands.items():
        result = subprocess.run(command, check=True, capture_output=True, text=True, timeout=60)
        (out / (name + '.txt')).write_text(result.stdout)


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


def run(args: argparse.Namespace) -> None:
    if os.environ.get('DISPLAY') != ':0':
        raise ValueError('native measurement requires DISPLAY=:0')
    out = LOCAL / 'runs' / args.trial
    out.mkdir(parents=True, mode=0o700)
    if args.instance == 'ravel':
        prepare_candidate(out)
    else:
        prepare = 'prepare.py' if args.instance == 'original' else 'cleanroom.py'
        subprocess.run(['python3', str(Path(__file__).with_name(prepare)),
                        '--agent', str(LOCAL / 'observer.jar'), '--output', str(out)], check=True)
    command, cwd = prepare_command(args.instance, out, args.candidate)
    instance = LOCAL / 'prism/instances' / args.instance
    game_log = instance / '.minecraft/logs/latest.log'
    game_log.unlink(missing_ok=True)
    world = instance / '.minecraft/saves/RavelBenchmark'
    if world.exists():
        shutil.rmtree(world)
    if args.cache == 'cold-data-cache':
        evict_inputs(instance, out)
    hardware(out)
    start = time.monotonic_ns()
    with (out / 'launcher.log').open('w') as log, (out / 'process.csv').open('w') as raw:
        process = subprocess.Popen(command, cwd=cwd, stdout=log, stderr=subprocess.STDOUT,
                                   start_new_session=True)
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
              'start_monotonic_ns': start, 'wall_seconds': (time.monotonic_ns()-start)/1e9,
              'launcher_exit': process.returncode, 'complete': (out / 'complete').exists(),
              'monitor_error': failure}
    (out / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
    if not result['complete'] or (out / 'error.txt').exists():
        raise RuntimeError('trial did not complete; inspect private trial logs')


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
    if '/' in args.matrix or args.matrix in ('.', '..'):
        raise ValueError('matrix prefix must be a simple identifier')
    for repetition in range(1, 4):
        for cache in ('cold-data-cache', 'warm-filesystem'):
            trial = f'{args.matrix}-{cache}-{repetition}'
            command = ['mise', 'exec', '--', 'numen', 'run', '--', 'python3', str(Path(__file__)),
                       trial, '--cache', cache, '--instance', args.instance,
                       '--timeout', str(args.timeout)]
            if args.candidate:
                command += ['--candidate', str(args.candidate.resolve())]
            subprocess.run(command, check=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('trial', nargs='?')
    parser.add_argument('--build', action='store_true')
    parser.add_argument('--cache', choices=['cold-data-cache', 'warm-filesystem'], default='warm-filesystem')
    parser.add_argument('--instance', choices=['original', 'cleanroom', 'ravel'], default='original')
    parser.add_argument('--candidate', type=Path, help='private Ravel direct launch specification JSON')
    parser.add_argument('--timeout', type=int, default=1800)
    parser.add_argument('--matrix', metavar='PREFIX', help='run three separately locked cold/warm pairs')
    args = parser.parse_args()
    if (args.instance == 'ravel') != bool(args.candidate):
        parser.error('--candidate is required exactly for --instance ravel')
    if args.build:
        build()
    elif args.matrix:
        matrix(args)
    elif args.trial and '/' not in args.trial and args.trial not in ('.', '..'):
        run(args)
    else:
        parser.error('supply --build or a fresh trial identifier')


if __name__ == '__main__':
    main()
