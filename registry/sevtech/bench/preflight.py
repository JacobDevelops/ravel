"""Fail renderer comparison batches before paying for physical trials."""
import json
import os
from pathlib import Path
import signal
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]


def launcher_environment() -> dict[str, str]:
    environment = os.environ.copy()
    for setting in (ROOT / 'jvm/legacy-platform/launcher.env').read_text().splitlines():
        if not setting.strip() or setting.lstrip().startswith('#'):
            continue
        key, value = setting.split('=', 1)
        environment.setdefault(key, value)
    return environment


def validate_preflight(data: dict) -> None:
    if type(data.get('admissions')) is not int or data['admissions'] <= 0:
        raise ValueError('preflight: selected renderer admitted no baked sections')
    expected = {'opaque', 'cutout-mipped', 'cutout', 'translucent', 'animated'}
    if set(data.get('coveredScenes', [])) != expected:
        raise ValueError('preflight: golden scenes are not all loaded and measured')
    if data.get('audioDriver') != 'null':
        raise ValueError('preflight: audio is not silent (ALSOFT_DRIVERS must be null)')


def stop_preflight(process: subprocess.Popen) -> None:
    if process.poll() is not None:
        return
    os.killpg(process.pid, signal.SIGTERM)
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()


def run_preflight(candidate: Path | None = None, trial: str = 'renderer-preflight') -> None:
    if not trial or '/' in trial or trial in {'.', '..'}:
        raise ValueError('preflight: trial must be a single directory name')
    sys.path.insert(0, str(ROOT / 'jvm/legacy-platform/tools'))
    from renderer_proof import validate_scene_settings
    validate_scene_settings()
    environment = launcher_environment()
    if environment['ALSOFT_DRIVERS'] != 'null':
        raise ValueError('preflight: audio is not silent (ALSOFT_DRIVERS must be null)')
    environment.update(RAVEL_LEGACY_PREFLIGHT='1', RAVEL_LEGACY_RENDERER='geometry-proof',
                       RAVEL_LEGACY_EVIDENCE_SUFFIX='-preflight')
    out = ROOT / 'tests/fixtures/local/legacy-platform/evidence-display-agent-preflight'
    command = ['bash', str(ROOT / 'jvm/legacy-platform/launch.sh'), 'client-agent', 'display']
    cwd = ROOT
    if candidate is not None:
        from vanilla import prepare_vanilla, prepare_command
        out = ROOT / 'tests/fixtures/local/sevtech-ages-3.2.3/baseline/runs' / trial
        out.mkdir(parents=True, exist_ok=False)
        prepare_vanilla(out, 'ravel', candidate)
        command, cwd = prepare_command(out, 'ravel', candidate, renderer='geometry-proof')
        observer = '-javaagent:' + str((out.parents[1] / 'observer.jar').resolve()) + '='
        command = [value for value in command if value != '-Dravel.legacy.benchmark=true'
                   and not value.startswith(observer)]
        command[1:1] = ['-Dravel.legacy.preflight=true', '-Dravel.legacy.acceptance=true',
                        '-Dravel.legacy.proof=true']
    else:
        out.mkdir(parents=True, exist_ok=True)
    proof = out / 'preflight.json'
    proof.unlink(missing_ok=True)
    with (out / 'preflight-launcher.log').open('w') as log:
        process = subprocess.Popen(command, cwd=cwd, env=environment, start_new_session=True,
                                   stdout=log, stderr=subprocess.STDOUT)
    try:
        code = process.wait(timeout=60)
    except subprocess.TimeoutExpired as error:
        raise RuntimeError('preflight: launch exceeded 60 seconds; inspect ' + str(out)) from error
    finally:
        stop_preflight(process)
    if code != 0 or not proof.is_file():
        raise RuntimeError('preflight: no fresh successful scene proof; inspect ' + str(out))
    validate_preflight(json.loads(proof.read_text()))
