"""Offline acquisition and dedicated-server acceptance regression tests."""
import contextlib
import hashlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import acquire
import server


class AcquireTests(unittest.TestCase):
    def test_acquire_checks_both_hashes_and_reuses_verified_cache(self):
        content = b'official input'
        item = {'sha1': hashlib.sha1(content).hexdigest(), 'url': 'https://example.invalid/input'}
        pin = hashlib.sha256(content).hexdigest()
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            local = root / 'local.jar'
            local.write_bytes(content)
            destination = root / 'work/client.jar'
            with patch.object(acquire, 'WORK', root / 'work'), patch.object(acquire, 'PINS', {'client.jar': pin}):
                self.assertEqual(acquire.acquire(item, destination, local), ('client.jar', pin))
                with patch.object(acquire.urllib.request, 'urlopen', side_effect=AssertionError('cache downloaded')):
                    self.assertEqual(acquire.acquire(item, destination), ('client.jar', pin))
                with patch.dict(acquire.PINS, {'client.jar': 'wrong'}):
                    with self.assertRaisesRegex(RuntimeError, 'Pinned SHA256 mismatch'):
                        acquire.acquire(item, destination)
                destination.unlink()
                local.write_bytes(b'corrupt')
                with self.assertRaisesRegex(RuntimeError, 'Official SHA1 mismatch'):
                    acquire.acquire(item, destination, local)
                self.assertFalse(destination.exists())
                self.assertFalse(destination.with_suffix('.jar.download').exists())

    def test_allowed_excludes_lwjgl2_and_applies_last_matching_linux_rule(self):
        for name in ('org.lwjgl.lwjgl:lwjgl:2', 'net.java.jinput:jinput:2', 'net.java.jutils:jutils:1'):
            self.assertFalse(acquire.allowed({'name': name}))
        library = {'name': 'official:library:1'}
        self.assertTrue(acquire.allowed(library))
        library['rules'] = [{'action': 'allow'}, {'action': 'disallow', 'os': {'name': 'linux'}}]
        self.assertFalse(acquire.allowed(library))
        library['rules'].append({'action': 'allow', 'os': {'name': 'windows'}})
        self.assertFalse(acquire.allowed(library))
        library['rules'].append({'action': 'allow', 'os': {'name': 'linux'}})
        self.assertTrue(acquire.allowed(library))

    def test_main_publishes_classpaths_and_native_files_without_lwjgl2(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            inputs, work = root / 'inputs', root / 'work'
            inputs.mkdir()
            version = {'downloads': {'client': {}, 'server': {}}, 'libraries': [
                {'name': 'official:library:1', 'downloads': {
                    'artifact': {'path': 'official.jar'},
                    'classifiers': {'natives-linux': {'path': 'official-natives-linux.jar'}}}},
                {'name': 'org.lwjgl.lwjgl:lwjgl:2', 'downloads': {'artifact': {'path': 'forbidden.jar'}}}]}
            manifest = json.dumps(version).encode()
            (inputs / 'version.json').write_bytes(manifest)
            jobs = []

            def materialize(item, destination, local=None):
                jobs.append(destination)
                destination.parent.mkdir(parents=True, exist_ok=True)
                if destination.name.endswith('-natives-linux.jar'):
                    with zipfile.ZipFile(destination, 'w') as archive:
                        archive.writestr('nested/libofficial.so', b'native')
                        archive.writestr('META-INF/manifest', b'ignored')
                else:
                    destination.write_bytes(b'jar')

            with patch.object(acquire, 'WORK', work), patch.object(acquire, 'INPUT', inputs), \
                    patch.object(acquire, 'MANIFEST_SHA256', hashlib.sha256(manifest).hexdigest()), \
                    patch.object(acquire, 'acquire', side_effect=materialize), \
                    patch.object(acquire.sys, 'argv', ['acquire.py']), contextlib.redirect_stdout(io.StringIO()):
                acquire.main()
            self.assertEqual((work / 'version.json').read_bytes(), manifest)
            self.assertEqual((work / 'natives/libofficial.so').read_bytes(), b'native')
            self.assertEqual((work / 'server-classpath.txt').read_text(), str(work / 'server.jar') + '\n')
            self.assertEqual((work / 'client-classpath.txt').read_text().splitlines(), [
                str(work / name) for name in ('client.jar', 'libraries/official.jar', 'libraries/official-natives-linux.jar')])
            self.assertNotIn(work / 'libraries/forbidden.jar', jobs)

    def test_manifest_digest_failure_never_publishes_download(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            with patch.object(acquire, 'WORK', root / 'work'), patch.object(acquire, 'INPUT', root / 'inputs'), \
                    patch.object(acquire.urllib.request, 'urlopen', return_value=io.BytesIO(b'corrupt')):
                with self.assertRaisesRegex(RuntimeError, 'version manifest SHA256 mismatch'):
                    acquire.main()
            self.assertFalse((root / 'work/version.json').exists())

    def test_downloaded_manifest_is_published_and_warm_cache_is_not_rewritten(self):
        content = json.dumps({'downloads': {'client': {}, 'server': {}}, 'libraries': []}).encode()
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            work = root / 'work'
            manifest = work / 'version.json'
            with patch.object(acquire, 'WORK', work), patch.object(acquire, 'INPUT', root / 'inputs'), \
                    patch.object(acquire, 'MANIFEST_SHA256', hashlib.sha256(content).hexdigest()), \
                    patch.object(acquire, 'acquire'), patch.object(acquire.sys, 'argv', ['acquire.py']), \
                    contextlib.redirect_stdout(io.StringIO()):
                with patch.object(acquire.urllib.request, 'urlopen', return_value=io.BytesIO(content)) as opened:
                    acquire.main()
                opened.assert_called_once_with(acquire.MANIFEST_URL, timeout=90)
                self.assertEqual(manifest.read_bytes(), content)
                with patch.object(acquire.urllib.request, 'urlopen', side_effect=AssertionError('cache downloaded')), \
                        patch.object(acquire, 'atomic_write', wraps=acquire.atomic_write) as published:
                    acquire.main()
                self.assertNotIn(manifest, [call.args[0] for call in published.call_args_list])
                self.assertEqual(manifest.read_bytes(), content)

    def test_atomic_publication_keeps_previous_file_when_rename_fails(self):
        with tempfile.TemporaryDirectory() as raw:
            destination = Path(raw) / 'classpath.txt'
            destination.write_bytes(b'previous')
            with patch.object(Path, 'replace', side_effect=OSError('rename failed')):
                with self.assertRaisesRegex(OSError, 'rename failed'):
                    acquire.atomic_write(destination, b'new')
            self.assertEqual(destination.read_bytes(), b'previous')
            self.assertEqual(list(destination.parent.iterdir()), [destination])

    def test_asset_jobs_derive_official_urls_and_content_addressed_paths(self):
        with tempfile.TemporaryDirectory() as raw:
            work = Path(raw)
            index = {'id': 'legacy'}
            content_hash = '1234567890abcdef'
            def materialize(item, destination):
                self.assertEqual(item, index)
                destination.parent.mkdir(parents=True)
                destination.write_text(json.dumps({'objects': {'sound': {'hash': content_hash}}}))
            with patch.object(acquire, 'WORK', work), patch.object(acquire, 'acquire', side_effect=materialize):
                jobs = acquire.asset_jobs({'assetIndex': index})
            self.assertEqual(jobs, [(
                {'sha1': content_hash, 'url': 'https://resources.download.minecraft.net/12/' + content_hash},
                work / 'assets/objects/12' / content_hash, None)])



class ServerTests(unittest.TestCase):
    def run_server(self, lines, libraries='', result=0, timed_out=False):
        root = self.enterContext(tempfile.TemporaryDirectory())
        game = Path(root) / 'game'
        game.mkdir()
        evidence = game.parent / 'evidence-server'
        self.evidence = evidence
        evidence.mkdir()
        (evidence / 'libraries.log').write_text(libraries)
        (evidence / 'server.complete').write_text('stale')
        process = unittest.mock.Mock()
        process.stdout = io.StringIO(lines)
        process.stdin = unittest.mock.Mock()
        process.poll.return_value = None
        process.wait.return_value = result
        timer = unittest.mock.Mock()
        if timed_out:
            timer.start.side_effect = lambda: timer.callback()
        def make_timer(seconds, callback):
            self.assertEqual(seconds, 300)
            timer.callback = callback
            return timer
        with patch.object(server.subprocess, 'Popen', return_value=process), \
                patch.object(server.threading, 'Timer', side_effect=make_timer), \
                patch.object(server.sys, 'argv', ['server.py', str(game), 'java', 'Main']), \
                contextlib.redirect_stdout(io.StringIO()):
            try:
                server.main()
            finally:
                self.assertTrue(timer.cancel.called)
                self.assertTrue(process.stdin.close.called)
                self.assertTrue(process.stdout.closed)
        return evidence, process

    def test_success_sends_console_commands_once_and_writes_completion(self):
        evidence, process = self.run_server('Done (1s)!\nDone (duplicate)!\nSeed: 8675309\nSaved the world\n')
        process.stdin.write.assert_called_once_with('seed\nsave-all\nstop\n')
        text = (evidence / 'server.complete').read_text()
        self.assertTrue(text.startswith('seed=8675309\nstartup=true\nsave=true\nclientNatives=false\nexit=0\n'))
        self.assertRegex(text, r'durationMs=\d+\n$')

    def test_rejects_missing_milestones_errors_bad_seed_and_nonzero_exit(self):
        good = 'Done (1s)!\nSeed: 8675309\nSaved the world\n'
        for lines, code, timeout in ((good.replace('Done (1s)!', ''), 0, False),
                                     (good.replace('Saved the world', ''), 0, False),
                                     (good.replace('8675309', '-1'), 0, False),
                                     (good + '[Server/ERROR]: failure\n', 0, False),
                                     (good + '[STDERR]: failure\n', 0, False),
                                     (good, 1, False), (good, 0, True)):
            with self.subTest(lines=lines, code=code, timeout=timeout):
                with self.assertRaisesRegex(SystemExit, 'startup/save/clean-exit'):
                    self.run_server(lines, result=code, timed_out=timeout)
                self.assertFalse((self.evidence / 'server.complete').exists())

    def test_rejects_each_client_native_library(self):
        for library in ('LWJGL', 'glfw', 'OpenAL', 'jinput'):
            with self.subTest(library=library):
                with self.assertRaisesRegex(SystemExit, 'client native library'):
                    self.run_server('Done (1s)!\nSeed: 8675309\nSaved the world\n', libraries=library)
                self.assertFalse((self.evidence / 'server.complete').exists())


if __name__ == '__main__':
    unittest.main()
