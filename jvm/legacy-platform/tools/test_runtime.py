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
import agent
import live
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
    def run_server(self, lines: str, libraries: str = '', result: int = 0,
                   timed_out: bool = False) -> tuple[Path, unittest.mock.Mock]:
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


class AgentTests(unittest.TestCase):
    def test_requires_legacy_marker_named_module_and_runtime_classes(self):
        manifest = ('Ravel-Legacy-Adapter: true\n'
                    'Automatic-Module-Name: dev.ravel.agent\n')
        entries = ('dev/ravel/jvm/Hook.class', 'dev/ravel/jvm/LegacyRuntime.class')
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / 'agent.jar'
            for missing in (None, *manifest.splitlines(), *entries):
                with self.subTest(missing=missing):
                    with zipfile.ZipFile(archive, 'w') as output:
                        output.writestr('META-INF/MANIFEST.MF',
                                        '\n'.join(line for line in manifest.splitlines() if line != missing))
                        for entry in entries:
                            if entry != missing:
                                output.writestr(entry, b'')
                    with patch.object(agent.sys, 'argv', ['agent.py', str(archive)]):
                        if missing is None:
                            agent.main()
                        else:
                            with self.assertRaises(SystemExit):
                                agent.main()


def renderer_proof_fixture():
    return {'beforeReload': 2, 'afterReload': 3, 'afterUnload': 4,
            'reopened': 4, 'finalUnload': 5, 'staleRejected': True,
            'ownerThread': 1, 'workerObserved': True,
            'injectedStaleObservationRejected': True,
            'seams': {name: 1 for name in live.RENDERER_SEAMS},
            'snapshot': {
                'captures': 2, 'seed': 8675309, 'edge': 18,
                'comparison': 'decoded-vs-Java-copied-post-compile-region',
                'light': 'raw-stored-sky-and-block-not-combined-mesh-light',
                'oracle': 'coordinate-only-same-mappings',
                'modelKeys': 'state-derived-not-baked-model-identities',
                'maxConcurrentCaptures': 1, 'timingSampleLimit': 4096,
                'javaBatchBytes': 29160, 'retainedStaleProofBytes': 29160,
                'maxJavaBatchPayloadBytes': 58320,
                'maxNativeCopyBytes': 29160, 'maxNativeScratchBytes': 39712,
                'cells': 11664, 'borderCells': 3472, 'mismatches': 0,
                'nonAirCells': 1, 'adjacentVariation': 1, 'workerCaptures': 1,
                'oracleChecks': 125, 'oracleMismatches': 0, 'oracleBorderChecks': 98,
                'oracleRetryCells': 0, 'oracleRetryCandidates': 0,
                'staleCaptureRejections': 1, 'lastThread': 2, 'lastGeneration': 4,
                'rejected': 0, 'skipped': 1, 'maxBytes': 100, 'totalBytes': 200,
                'p50Nanos': 10, 'p95Nanos': 20, 'p99Nanos': 30,
                'maxNanos': 40, 'totalNanos': 80}}


class RendererProofTests(unittest.TestCase):
    def test_generations_seams_and_freshness(self):
        proof = renderer_proof_fixture()
        proof['snapshot'].update(oracleRetryCells=7, oracleRetryCandidates=2)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'renderer.json'
            path.write_text(json.dumps(proof))
            self.assertEqual(live.renderer_proof(path, 0), proof)
            for change in ({'afterReload': 2}, {'afterUnload': 3}, {'reopened': 3},
                           {'finalUnload': 4}, {'staleRejected': False},
                           {'workerObserved': False}, {'ownerThread': 0},
                           {'injectedStaleObservationRejected': False},
                           {'seams': {'frame': 1}}):
                path.write_text(json.dumps(proof | change))
                with self.assertRaises(RuntimeError):
                    live.renderer_proof(path, 0)
            path.write_text(json.dumps(proof))
            with self.assertRaises(RuntimeError):
                live.renderer_proof(path, float('inf'))

    def test_bounded_vanilla_roundtrip_and_timing(self):
        proof = renderer_proof_fixture()
        snapshot = proof['snapshot']
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'renderer.json'
            for key, invalid in (('captures', 0), ('seed', 1), ('edge', 16),
                                 ('comparison', 'receipt-only'), ('modelKeys', 'baked'),
                                 ('maxConcurrentCaptures', 2), ('timingSampleLimit', 0),
                                 ('javaBatchBytes', 0), ('retainedStaleProofBytes', 0),
                                 ('maxJavaBatchPayloadBytes', 29160),
                                 ('maxNativeCopyBytes', 0), ('maxNativeScratchBytes', 0),
                                 ('cells', 5832), ('borderCells', 0), ('mismatches', 1),
                                 ('nonAirCells', 0), ('adjacentVariation', 0),
                                 ('workerCaptures', 0), ('staleCaptureRejections', 0),
                                 ('oracleChecks', 0), ('oracleChecks', 126), ('oracleMismatches', 1),
                                 ('oracleBorderChecks', 0), ('light', 'combined-mesh-light'),
                                 ('oracleRetryCells', -1), ('oracleRetryCandidates', -1),
                                 ('oracleRetryCells', 1), ('oracleRetryCandidates', 1),
                                 ('oracle', 'independent-mapping-oracle'),
                                 ('lastThread', 0), ('lastGeneration', 0),
                                 ('rejected', -1), ('skipped', -1), ('maxBytes', 0),
                                 ('totalBytes', 1), ('p50Nanos', 0), ('p95Nanos', 9),
                                 ('p99Nanos', 19), ('maxNanos', 29), ('totalNanos', 39)):
                with self.subTest(snapshotField=key):
                    path.write_text(json.dumps(proof | {'snapshot': snapshot | {key: invalid}}))
                    with self.assertRaises(RuntimeError):
                        live.renderer_proof(path, 0)


class LiveTests(unittest.TestCase):

    def test_counts_requires_native_crossing_named_module_and_no_agent_failure(self):
        lines = ('1 hook invocations reached native code',
                 'nativeAccess agent=dev.ravel.agent:true platform=false',
                 '[ravel-legacy] hook legacy.title-init=2')
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / 'client.log'
            log.write_text('\n'.join(lines))
            self.assertEqual(live.counts(log), {'legacy.title-init': 2})
            for text in ('\n'.join(lines[1:]), '\n'.join((lines[0], lines[2])),
                         '\n'.join(lines) + '\n[ravel] failed'):
                with self.subTest(text=text):
                    log.write_text(text)
                    with self.assertRaises(RuntimeError):
                        live.counts(log)

    def test_collect_requires_fresh_logs_and_sums_only_inventory_hooks(self):
        text = ('1 hook invocations reached native code\n'
                'nativeAccess agent=dev.ravel.agent:true platform=false\n'
                '[ravel-legacy] hook legacy.title-init=2\n'
                '[ravel-legacy] hook legacy.unknown=9\n')
        inventory = {'hook': [{'id': 'legacy.title-init'}]}
        with tempfile.TemporaryDirectory() as directory, patch.object(live, 'WORK', Path(directory)):
            for label in ('xvfb-agent', 'xvfb-agent-dedicated', 'server-agent', 'server-agent-restart'):
                log = Path(directory) / f'evidence-{label}'
                log.mkdir()
                (log / ('client.log' if label.startswith('xvfb') else 'server.log')).write_text(text)
            hooks, errors = live.collect(float('inf'), inventory)
            self.assertEqual(hooks, {'legacy.title-init': 0})
            self.assertEqual(len(errors), 4)
            self.assertTrue(all('No fresh process evidence' in error for error in errors))
            self.assertEqual(live.collect(0, inventory), ({'legacy.title-init': 8}, []))


if __name__ == '__main__':
    unittest.main()
