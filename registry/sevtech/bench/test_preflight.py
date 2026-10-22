"""Bounded preflight uses the trial's pinned runtime without the timed observer."""
from contextlib import contextmanager
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
from unittest import TestCase
from unittest.mock import Mock, call, patch

import preflight
import vanilla
sys.path.insert(0, str(preflight.ROOT / 'jvm/legacy-platform/tools'))


class PreflightLaunchTests(TestCase):
    def test_launcher_environment_ignores_blank_and_comment_lines(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            settings = root / 'jvm/legacy-platform/launcher.env'
            settings.parent.mkdir(parents=True)
            settings.write_text(
                '\n   \t\n# Silent audio defaults\n  # comment=value\n'
                'ALSOFT_DRIVERS=null\nRAVEL_SETTING=value=with=equals\n')
            with patch.object(preflight, 'ROOT', root), \
                 patch.dict(preflight.os.environ, {}, clear=True):
                environment = preflight.launcher_environment()
            self.assertEqual(environment, {
                'ALSOFT_DRIVERS': 'null', 'RAVEL_SETTING': 'value=with=equals'})

    def test_launcher_environment_preserves_existing_overrides(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            settings = root / 'jvm/legacy-platform/launcher.env'
            settings.parent.mkdir(parents=True)
            settings.write_text('# Default audio driver\n\nALSOFT_DRIVERS=null\n')
            with patch.object(preflight, 'ROOT', root), \
                 patch.dict(preflight.os.environ, {'ALSOFT_DRIVERS': 'pulse'}, clear=True):
                environment = preflight.launcher_environment()
            self.assertEqual(environment, {'ALSOFT_DRIVERS': 'pulse'})

    def test_candidate_launch_uses_same_runtime_without_observer(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            baseline = root / 'tests/fixtures/local/sevtech-ages-3.2.3/baseline'
            baseline.mkdir(parents=True)
            observer = root / 'retained-observer.jar'
            observer.touch()
            (baseline / 'observer.jar').symlink_to(observer)
            out = baseline / 'runs/check'
            candidate = root / 'candidate.json'
            command = ['java', '-javaagent:pinned-agent.jar', '-Dravel.legacy.benchmark=true',
                       '-javaagent:' + str((baseline / 'observer.jar').resolve()) + '=' + str(out),
                       'PinnedLauncher']
            proof = {'admissions': 1, 'audioDriver': 'null',
                     'coveredScenes': ['opaque', 'cutout-mipped', 'cutout', 'translucent', 'animated']}

            def launch(argv, **kwargs):
                self.assertIn('-javaagent:pinned-agent.jar', argv)
                self.assertIn('-Dravel.legacy.preflight=true', argv)
                self.assertIn('-Dravel.legacy.acceptance=true', argv)
                self.assertIn('-Dravel.legacy.proof=true', argv)
                self.assertNotIn('-Dravel.legacy.benchmark=true', argv)
                self.assertFalse(any('observer.jar' in token for token in argv))
                self.assertTrue(kwargs['start_new_session'])
                (out / 'preflight.json').write_text(json.dumps(proof))
                return Mock(wait=Mock(return_value=0), poll=Mock(return_value=0))

            with patch.object(preflight, 'ROOT', root), \
                 patch.object(preflight, 'launcher_environment', return_value={'ALSOFT_DRIVERS': 'null'}), \
                 patch('renderer_proof.validate_scene_settings'), \
                 patch.object(vanilla, 'prepare_vanilla') as prepare, \
                 patch.object(vanilla, 'prepare_command', return_value=(command, root)), \
                 patch.object(preflight.subprocess, 'Popen', side_effect=launch) as execute:
                preflight.run_preflight(candidate, 'check')
            prepare.assert_called_once_with(out, 'ravel', candidate)

    def test_nonsilent_driver_fails_before_launch(self) -> None:
        with patch.object(preflight, 'launcher_environment', return_value={'ALSOFT_DRIVERS': 'pulse'}), \
             patch('renderer_proof.validate_scene_settings'), \
             patch.object(preflight.subprocess, 'Popen') as execute:
            with self.assertRaisesRegex(ValueError, 'silent'):
                preflight.run_preflight()
        execute.assert_not_called()

    @contextmanager
    def _mock_launch(self, process, proof=None, timestamp=None):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            out = root / 'tests/fixtures/local/legacy-platform/evidence-display-agent-preflight'

            def launch(*args, **kwargs):
                if proof is not None:
                    path = out / 'preflight.json'
                    path.write_text(json.dumps(proof))
                    if timestamp is not None:
                        os.utime(path, ns=(timestamp, timestamp))
                return process

            events = Mock()
            events.attach_mock(process.wait, 'wait')
            with patch.object(preflight, 'ROOT', root), \
                 patch.object(preflight, 'launcher_environment', return_value={'ALSOFT_DRIVERS': 'null'}), \
                 patch('renderer_proof.validate_scene_settings'), \
                 patch.object(preflight.subprocess, 'Popen', side_effect=launch), \
                 patch.object(preflight.os, 'killpg') as killpg:
                events.attach_mock(killpg, 'killpg')
                yield events

    def test_launch_timeout_terminates_process_group_and_waits(self) -> None:
        timeout = subprocess.TimeoutExpired('preflight', 60)
        process = Mock(pid=12345, poll=Mock(return_value=None),
                       wait=Mock(side_effect=[timeout, 0]))
        with self._mock_launch(process) as events:
            with self.assertRaisesRegex(RuntimeError, 'launch exceeded 60 seconds') as raised:
                preflight.run_preflight()
            self.assertIs(raised.exception.__cause__, timeout)
            self.assertEqual(events.mock_calls, [
                call.wait(timeout=60),
                call.killpg(process.pid, signal.SIGTERM),
                call.wait(timeout=5),
            ])

    def test_stubborn_launch_timeout_kills_process_group_and_reaps(self) -> None:
        timeout = subprocess.TimeoutExpired('preflight', 60)
        process = Mock(pid=12345, poll=Mock(return_value=None), wait=Mock(side_effect=[
            timeout, subprocess.TimeoutExpired('preflight', 5), 0,
        ]))
        with self._mock_launch(process) as events:
            with self.assertRaisesRegex(RuntimeError, 'launch exceeded 60 seconds') as raised:
                preflight.run_preflight()
            self.assertIs(raised.exception.__cause__, timeout)
            self.assertEqual(events.mock_calls, [
                call.wait(timeout=60),
                call.killpg(process.pid, signal.SIGTERM),
                call.wait(timeout=5),
                call.killpg(process.pid, signal.SIGKILL),
                call.wait(),
            ])

    def test_keyboard_interrupt_cleans_process_group_before_propagating(self) -> None:
        interrupt = KeyboardInterrupt()
        process = Mock(pid=12345, poll=Mock(return_value=None),
                       wait=Mock(side_effect=[interrupt, 0]))
        with self._mock_launch(process) as events:
            with self.assertRaises(KeyboardInterrupt) as raised:
                preflight.run_preflight()
            self.assertIs(raised.exception, interrupt)
            self.assertEqual(events.mock_calls, [
                call.wait(timeout=60),
                call.killpg(process.pid, signal.SIGTERM),
                call.wait(timeout=5),
            ])

    def test_missing_stale_or_unsuccessful_proof_refuses_launch(self) -> None:
        proof = {'admissions': 1, 'audioDriver': 'null',
                 'coveredScenes': ['opaque', 'cutout-mipped', 'cutout', 'translucent', 'animated']}
        for name, code, evidence, timestamp in [
            ('missing', 0, None, None),
            ('stale', 0, proof, 1),
            ('nonzero', 1, proof, None),
        ]:
            with self.subTest(name=name):
                process = Mock(wait=Mock(return_value=code), poll=Mock(return_value=code))
                with self._mock_launch(process, evidence, timestamp) as events:
                    with self.assertRaisesRegex(RuntimeError, 'no fresh successful scene proof'):
                        preflight.run_preflight()
                    self.assertEqual(events.mock_calls, [call.wait(timeout=60)])

    def test_escaping_trial_fails_before_launch_preparation(self) -> None:
        with patch.object(preflight, 'launcher_environment') as environment, \
             patch('renderer_proof.validate_scene_settings') as validate, \
             patch.object(vanilla, 'prepare_vanilla') as prepare, \
             patch.object(preflight.subprocess, 'Popen') as execute:
            with self.assertRaisesRegex(ValueError, 'single directory name'):
                preflight.run_preflight(Path('candidate.json'), '../escape')
        environment.assert_not_called()
        validate.assert_not_called()
        prepare.assert_not_called()
        execute.assert_not_called()
