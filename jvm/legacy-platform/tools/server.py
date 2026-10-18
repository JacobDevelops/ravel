#!/usr/bin/env python3
"""Retain real dedicated-server output and stop through its console after startup."""
from collections.abc import Callable
from pathlib import Path
import os
import re
import subprocess
import sys
import threading
import time


def connect(save: Callable[[], None], client_result: list[int]) -> None:
    environment = os.environ.copy()
    environment['RAVEL_LEGACY_SERVER'] = '127.0.0.1:25586'
    environment['RAVEL_LEGACY_EVIDENCE_SUFFIX'] = '-dedicated'
    environment.pop('RAVEL_LEGACY_CONNECT_CLIENT', None)
    root = Path(__file__).resolve().parents[3]
    try:
        result = subprocess.run(['bash', str(root / 'jvm/legacy-platform/launch.sh'),
                                 'client-agent', 'xvfb'], cwd=root, env=environment, check=False)
        client_result.append(result.returncode)
    finally:
        save()


def main() -> None:
    began = time.monotonic()
    game = Path(sys.argv[1])
    evidence = Path(os.environ.get('RAVEL_LEGACY_EVIDENCE', game.parent / 'evidence-server'))
    evidence.mkdir(parents=True, exist_ok=True)
    complete = evidence / 'server.complete'
    complete.unlink(missing_ok=True)
    process = subprocess.Popen(sys.argv[2:], cwd=game, stdin=subprocess.PIPE,
                               stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    failure = threading.Event()
    connected = os.environ.get('RAVEL_LEGACY_CONNECT_CLIENT') == '1'
    client_result = []
    client_thread = None

    def stop():
        if process.poll() is None:
            failure.set()
            process.kill()

    def save():
        if process.poll() is None:
            process.stdin.write('seed\nsave-all\nstop\n')
            process.stdin.flush()


    deadline = threading.Timer(300, stop)
    deadline.start()
    started = False
    saved = False
    seed = None
    errors = False
    try:
        with (evidence / 'server.log').open('w') as output:
            for line in process.stdout:
                output.write(line)
                output.flush()
                print(line, end='', flush=True)
                if 'Done (' in line and not started:
                    started = True
                    if connected:
                        client_thread = threading.Thread(target=connect, args=(save, client_result),
                                                         name='legacy-connected-client')
                        client_thread.start()
                    else:
                        save()
                if 'Saved the world' in line:
                    saved = True
                if re.search(r'/(ERROR|FATAL)\]|\[STDERR\]:', line):
                    errors = True
                match = re.search(r'Seed: (-?\d+)', line)
                if match:
                    seed = int(match.group(1))
        result = process.wait()
        if client_thread is not None:
            client_thread.join()
    finally:
        deadline.cancel()
        process.stdin.close()
        process.stdout.close()
    if result != 0 or failure.is_set() or errors or not started or not saved or seed != 8675309:
        raise SystemExit('Dedicated server failed startup/save/clean-exit criteria')
    if connected and client_result != [0]:
        raise SystemExit('Dedicated connected-client acceptance failed')
    libraries = (evidence / 'libraries.log').read_text().lower()
    if any(name in libraries for name in ('lwjgl', 'glfw', 'openal', 'jinput')):
        raise SystemExit('Dedicated server loaded a client native library')
    temporary = complete.with_suffix('.tmp')
    temporary.write_text(f'seed={seed}\nstartup=true\nsave=true\nclientNatives=false\nexit=0\n'
                         + 'durationMs=' + str(round((time.monotonic() - began) * 1000)) + '\n')
    temporary.replace(complete)


if __name__ == '__main__':
    main()
