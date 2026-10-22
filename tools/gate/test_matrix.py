from contextlib import ExitStack
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch

import matrix


class MatrixTests(unittest.TestCase):
    def test_live_commands_exclude_generators_until_serial_suite(self) -> None:
        commands = matrix.version_commands('live')
        self.assertEqual(len(commands), 3)
        self.assertIn('--exclude', commands[0])
        self.assertIn('--no-default-features', commands[1])
        self.assertNotIn('--exclude', commands[2])
        self.assertIn('ravel-minecraft-generate', commands[2])
        self.assertEqual(matrix.version_commands('generation-only'), [
            ['cargo', 'build', '--locked', '-p', 'ravel-minecraft-adapter']])

    def test_version_records_counts_and_stops_after_failure(self) -> None:
        for code, expected_calls in [(0, 3), (1, 1)]:
            with self.subTest(code=code), tempfile.TemporaryDirectory() as raw, ExitStack() as stack:
                root = Path(raw)
                output = root / 'output'
                output.mkdir()
                stack.enter_context(patch.object(matrix, 'ROOT', root))
                stack.enter_context(patch.object(matrix, 'OUTPUT', output))
                stack.enter_context(patch.dict(matrix.os.environ, {}, clear=True))
                stack.enter_context(patch.object(matrix.os, 'sched_getaffinity', return_value={0, 1, 2, 3}))

                def execute(command: list[str], **kwargs: object) -> Mock:
                    kwargs['stdout'].write('Summary [1s] 7 tests run:\n')
                    self.assertEqual(kwargs['env']['CARGO_BUILD_JOBS'], '2')
                    return Mock(returncode=code)

                run = stack.enter_context(patch.object(matrix.subprocess, 'run', side_effect=execute))
                result = matrix.test_version('test', 'live', 2)
                self.assertEqual(run.call_count, expected_calls)
                self.assertEqual(result['exit'], code)
                self.assertEqual(result['tests'], 7 * expected_calls)
                self.assertEqual(json.loads((output / 'tests-test.json').read_text()), result)

    def test_interrupted_run_does_not_leave_a_stale_result(self) -> None:
        with tempfile.TemporaryDirectory() as raw, ExitStack() as stack:
            root = Path(raw)
            (root / 'tests-test.json').write_text('{"exit": 0}\n')
            stack.enter_context(patch.object(matrix, 'ROOT', root))
            stack.enter_context(patch.object(matrix, 'OUTPUT', root))
            stack.enter_context(patch.object(matrix.subprocess, 'run', side_effect=KeyboardInterrupt))
            with self.assertRaises(KeyboardInterrupt):
                matrix.test_version('test', 'generation-only', 1)
            self.assertFalse((root / 'tests-test.json').exists())

    def test_main_requires_successful_live_coverage(self) -> None:
        for eligibilities, status, succeeds in [
            (['live', 'generation-only', 'not-ready'], 0, True),
            (['generation-only'], 0, False),
            (['live'], 1, False),
        ]:
            with self.subTest(eligibilities=eligibilities, status=status), tempfile.TemporaryDirectory() as raw, ExitStack() as stack:
                root = Path(raw)
                versions = root / 'minecraft/versions'
                versions.mkdir(parents=True)
                for index in range(len(eligibilities)):
                    (versions / str(index)).mkdir()
                (versions / 'not-a-directory').touch()
                stack.enter_context(patch.object(matrix, 'ROOT', root))
                stack.enter_context(patch.object(matrix, 'OUTPUT', root / 'output'))
                stack.enter_context(patch.object(matrix.os, 'chdir'))
                stack.enter_context(patch.object(matrix.Path, 'iterdir', return_value=list(versions.iterdir())))
                capture = stack.enter_context(patch.object(matrix.subprocess, 'check_output', side_effect=eligibilities))
                run = stack.enter_context(patch.object(matrix, 'test_version', return_value={'exit': status}))
                if succeeds:
                    matrix.main()
                else:
                    with self.assertRaisesRegex(SystemExit, 'proved no live version'):
                        matrix.main()
                verdicts = (root / 'output/tested-versions.txt').read_text()
                self.assertNotIn('tested:', verdicts if not succeeds else '')
                if succeeds:
                    self.assertIn('tested: Minecraft 0', verdicts)
                    self.assertIn('built: Minecraft 1', verdicts)
                    self.assertIn('SKIPPED: Minecraft 2', verdicts)
                self.assertEqual(run.call_count, sum(value in ('live', 'generation-only') for value in eligibilities))
                self.assertTrue(all(call.args[0][0] == 'cargo' for call in capture.call_args_list))
