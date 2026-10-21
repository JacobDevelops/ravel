"""Resolver tests: no network or JVM execution."""
import hashlib
import io
from pathlib import Path
import shutil
import stat
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import uuid
import zipfile

import launch


class LaunchTests(unittest.TestCase):
    def test_java_version_requires_the_actual_profile_major(self):
        for major, version in ((8, '1.8.0_504'), (27, '27-ea')):
            result = launch.subprocess.CompletedProcess(
                ['/actual/java', '-version'], 0, '', 'openjdk version "' + version + '"')
            with patch.object(launch.subprocess, 'run', return_value=result) as invoked:
                self.assertIn(version, launch.validate_java('/actual/java', major))
                invoked.assert_called_once_with(
                    ['/actual/java', '-version'], capture_output=True, text=True, check=True, timeout=30)
            with patch.object(launch.subprocess, 'run', return_value=result):
                with self.assertRaisesRegex(ValueError, 'actual Java'):
                    launch.validate_java('/actual/java', 27 if major == 8 else 8)

    def test_download_upgrades_https_and_identifies_client(self):
        content = b'published artifact'
        spec = {'url': 'http://example.invalid/library.jar',
                'sha1': hashlib.sha1(content).hexdigest(), 'size': len(content)}
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            records = {}
            with patch.object(launch.urllib.request, 'urlopen', return_value=io.BytesIO(content)) as opened:
                launch.fetch(root / 'library.jar', spec, root, records)
            request = opened.call_args.args[0]
            self.assertEqual(request.full_url, 'https://example.invalid/library.jar')
            self.assertEqual(request.get_header('User-agent'), 'RavelSevTechBaseline/1.0')
            self.assertEqual(records['library.jar']['url'], request.full_url)
            self.assertEqual(records['library.jar']['sha256'], hashlib.sha256(content).hexdigest())

    def test_private_path_allows_linked_ancestor_not_private_links(self):
        with tempfile.TemporaryDirectory() as raw:
            base = Path(raw)
            actual = base / 'actual'
            actual.mkdir()
            ancestor = base / 'local'
            ancestor.symlink_to(actual, target_is_directory=True)
            root = ancestor / 'baseline'
            root.mkdir()
            target = launch.private_path(root, 'libraries/example.jar')
            self.assertEqual(target, root / 'libraries/example.jar')
            self.assertTrue(target.resolve().is_relative_to(root.resolve()))
            (root / 'libraries').symlink_to(actual, target_is_directory=True)
            with self.assertRaises(ValueError):
                launch.private_path(root, 'libraries/example.jar')
            (root / 'linked.jar').symlink_to(actual / 'example.jar')
            with self.assertRaises(ValueError):
                launch.private_path(root, 'linked.jar')
            with self.assertRaises(ValueError):
                launch.private_path(ancestor, 'example.jar')

    def test_rules_last_match_and_features(self):
        self.assertTrue(launch.allowed({}))
        self.assertFalse(launch.allowed({'rules': []}))
        self.assertTrue(launch.allowed({'rules': [
            {'action': 'allow'}, {'action': 'disallow', 'os': {'name': 'windows'}}]}))
        self.assertFalse(launch.allowed({'rules': [
            {'action': 'allow'}, {'action': 'disallow', 'os': {'name': 'linux'}}]}))
        self.assertFalse(launch.allowed({'rules': [
            {'action': 'allow', 'features': {'is_demo_user': True}}]}))

    def test_merge_replaces_versions_preserves_native_rows(self):
        base = {'order': -2, 'mainClass': 'Main', 'libraries': [
            {'name': 'g:a:1'}, {'name': 'g:b:1'}]}
        patch_component = {'order': 5, 'mainClass': 'Launch', '+tweakers': ['T'],
                           'libraries': [{'name': 'g:a:2'}, {'name': 'g:a:2', 'natives': {'linux': 'n'}}]}
        merged = launch.merge_components([patch_component, base])
        self.assertEqual([r['name'] for r in merged['libraries']], ['g:a:2', 'g:b:1', 'g:a:2'])
        self.assertEqual(merged['mainClass'], 'Launch')
        self.assertEqual(merged['tweakers'], ['T'])

    def test_native_extraction_rejects_traversal_and_links(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            for name in ('../escape', '/absolute', 'bad\\name'):
                archive = root / 'native.jar'
                with zipfile.ZipFile(archive, 'w') as stream:
                    stream.writestr(name, b'x')
                with self.assertRaises(ValueError):
                    launch.extract_natives(archive, root / 'natives')
            with zipfile.ZipFile(archive, 'w') as stream:
                info = zipfile.ZipInfo('linked.so')
                info.external_attr = (stat.S_IFLNK | 0o777) << 16
                stream.writestr(info, 'outside')
            with self.assertRaises(ValueError):
                launch.extract_natives(archive, root / 'natives')

    def test_native_extraction_skips_metadata(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            with zipfile.ZipFile(root / 'native.jar', 'w') as stream:
                stream.writestr('META-INF/signature', b'ignored')
                stream.writestr('liblwjgl64.so', b'native')
            launch.extract_natives(root / 'native.jar', root / 'natives')
            self.assertEqual((root / 'natives/liblwjgl64.so').read_bytes(), b'native')
            self.assertFalse((root / 'natives/META-INF').exists())

    def test_offline_arguments_and_observer(self):
        settings = {'javaPath': '/java8', 'minMemoryMiB': 4096, 'maxMemoryMiB': 8192,
                    'jvmArgs': ['-javaagent:/observer.jar=/trial']}
        profile = {'mainClass': 'net.minecraft.launchwrapper.Launch', 'tweakers': ['FMLTweaker'],
                   'minecraftArguments': '--username ${auth_player_name} --uuid ${auth_uuid} '
                                         '--accessToken ${auth_access_token} --userType ${user_type} '
                                         '--assetsDir ${assets_root} --assetIndex ${assets_index_name}'}
        command = launch.java_command(settings, profile, [Path('/a.jar')], Path('/game'),
                                      Path('/assets'), Path('/natives'), '1.12')
        self.assertEqual(command[0], '/java8')
        self.assertIn('-Xmx8192M', command)
        self.assertIn('-Djava.library.path=/natives', command)
        self.assertEqual(command[command.index('--username') + 1], 'BenchOffline')
        expected = uuid.UUID(bytes=hashlib.md5(b'OfflinePlayer:BenchOffline').digest(), version=3).hex
        self.assertEqual(command[command.index('--uuid') + 1], expected)
        self.assertEqual(command[command.index('--accessToken') + 1], '0')
        self.assertEqual(command[-6:], ['--width', '1280', '--height', '720', '--tweakClass', 'FMLTweaker'])

    def test_fetch_refuses_changed_sha256_pin(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            artifact = root / 'file'
            artifact.write_bytes(b'data')
            spec = {'url': 'https://example.invalid/file', 'sha1': hashlib.sha1(b'data').hexdigest(), 'size': 4}
            records = {'file': {'url': spec['url'], 'sha256': 'wrong'}}
            with self.assertRaisesRegex(ValueError, 'SHA256'):
                launch.fetch(artifact, spec, root, records)

    def test_maven_classifier_path(self):
        self.assertEqual(launch.maven_path('g.example:a:2', 'natives-linux'),
                         'g/example/a/2/a-2-natives-linux.jar')
        self.assertEqual(launch.maven_path('g:a:2:universal'), 'g/a/2/a-2-universal.jar')

    def test_candidate_requires_actual_pinned_classpath(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            jar = root / 'candidate.jar'
            jar.write_bytes(b'jar')
            candidate = {'javaPath': '/jdk27/bin/java', 'classpath': [str(jar)],
                         'sha256': {str(jar): hashlib.sha256(b'jar').hexdigest()},
                         'mainClass': 'real.Main', 'jvmArgs': [], 'gameArgs': ['--gameDir', '{game}']}
            command = launch.candidate_command(candidate, root / 'game', root / 'assets',
                                               root / 'natives', root / 'observer.jar', root / 'trial')
            self.assertIn('real.Main', command)
            self.assertIn(str(root / 'game'), command)
            agents = [arg for arg in command if arg.startswith('-javaagent:')]
            self.assertEqual(agents, ['-javaagent:' + str(root / 'observer.jar') + '=' + str(root / 'trial')])
            self.assertIn('-Xms4096M', command)
            self.assertIn('-Xmx8192M', command)
            self.assertFalse(any(arg.startswith('-XX:') for arg in command))
            for override in ('-Xmx16G', '-Xms1G', '-XX:MaxHeapSize=16G',
                             '-XX:InitialHeapSize=1G', '-XX:MaxRAMPercentage=90',
                             '-XX:InitialRAMFraction=2', '-XX:+AggressiveHeap',
                             '-XX:Flags=custom.flags', '@custom.args'):
                with self.subTest(override=override):
                    candidate['jvmArgs'] = [override]
                    with self.assertRaisesRegex(ValueError, 'heap'):
                        launch.candidate_command(candidate, root / 'game', root / 'assets',
                                                 root / 'natives', root / 'observer.jar', root / 'trial')
            candidate['jvmArgs'] = ['-Xms4096M', '-Xmx8192M']
            exact = launch.candidate_command(candidate, root / 'game', root / 'assets',
                                             root / 'natives', root / 'observer.jar', root / 'trial')
            self.assertEqual(exact.count('-Xms4096M'), 1)
            self.assertEqual(exact.count('-Xmx8192M'), 1)
            candidate['sha256'][str(jar)] = 'wrong'
            with self.assertRaisesRegex(ValueError, 'SHA256'):
                launch.candidate_command(candidate, root / 'game', root / 'assets',
                                         root / 'natives', root / 'observer.jar', root / 'trial')


class VanillaLaunchTests(unittest.TestCase):
    def _host_library(self, path: Path, *, fixture_mods: bool = False,
                      composed: bool = True, legacy: bool = True) -> None:
        symbols = ['Java_dev_ravel_jvm_Hook_ravelInitialize',
                   'Java_dev_ravel_jvm_Hook_ravelInitializeIntegratedServer'] if composed else ['unrelated']
        if composed and legacy:
            symbols.append('Java_dev_ravel_jvm_Hook_legacyBootstrap')
        if fixture_mods:
            symbols.append('Java_dev_ravel_jvm_LegacyPacketLive_receipts')
        source = '\n'.join('void ' + symbol + '(void) {}' for symbol in symbols)
        subprocess.run(['cc', '-shared', '-fPIC', '-x', 'c', '-', '-o', str(path)],
                       input=source, text=True, capture_output=True, check=True, timeout=30)

    def test_vanilla_dispatch_is_explicit_and_preserves_sevtech_default(self):
        import vanilla
        with patch.object(vanilla, 'prepare_command', return_value=(['java'], Path('/game'))) as prepare:
            self.assertEqual(launch.prepare_command('vanilla', '/trial'), (['java'], Path('/game')))
            prepare.assert_called_once_with(Path('/trial'), 'vanilla', None, renderer='vanilla')
            prepare.reset_mock()
            launch.prepare_command('ravel', '/trial', '/candidate', workload='vanilla')
            prepare.assert_called_once_with(Path('/trial'), 'ravel', '/candidate', renderer='vanilla')

    def test_vanilla_candidate_rejects_unpinned_runtime_and_extras(self):
        import vanilla
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            paths = {}
            for name in ('platform', 'agent', 'host', 'screen', 'lwjgl'):
                path = root / (name + '.jar')
                path.write_bytes(name.encode())
                paths[name] = str(path)
            self._host_library(Path(paths['host']))
            data = {'javaPath': '/jdk27/bin/java', 'platform': paths['platform'],
                    'agent': paths['agent'], 'host': paths['host'], 'screen': paths['screen'],
                    'lwjgl': [paths['lwjgl']],
                    'sha256': {value: launch.digest(Path(value)) for value in paths.values()}}
            vanilla.validate_candidate(data)
            data['jvmArgs'] = ['-javaagent:/untracked.jar']
            with self.assertRaisesRegex(ValueError, 'fields'):
                vanilla.validate_candidate(data)
            del data['jvmArgs']
            data['sha256'][paths['host']] = 'wrong'
            with self.assertRaisesRegex(ValueError, 'SHA256'):
                vanilla.validate_candidate(data)

    def test_vanilla_snapshot_retains_original_bytes(self):
        import vanilla
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            source = root / 'platform.jar'
            source.write_bytes(b'first')
            with patch.object(launch, 'LOCAL', root):
                retained = vanilla.snapshot(source)
                source.write_bytes(b'second')
                self.assertEqual(retained.read_bytes(), b'first')
                self.assertNotEqual(retained, vanilla.snapshot(source))

    def _vanilla_candidate(self, root: Path, java: Path) -> Path:
        import json
        candidate = root / 'candidate.json'
        paths = {}
        for name in ('platform.jar', 'agent.jar', 'host.so', 'screen.jar', 'lwjgl-3.3.6.jar',
                     'lwjgl-3.3.6-natives-linux.jar'):
            path = root / name
            path.write_bytes(name.encode())
            if name == 'host.so':
                self._host_library(path)
            paths[name] = path
        lwjgl = [paths[name] for name in ('lwjgl-3.3.6.jar', 'lwjgl-3.3.6-natives-linux.jar')]
        dependency = root / 'jvm/legacy-platform/dependencies.sha256'
        dependency.parent.mkdir(parents=True)
        dependency.write_text(''.join(launch.digest(path) + ' ' + path.name + '\n' for path in lwjgl))
        data = {'javaPath': str(java), 'platform': str(paths['platform.jar']),
                'agent': str(paths['agent.jar']), 'host': str(paths['host.so']),
                'screen': str(paths['screen.jar']), 'lwjgl': list(map(str, lwjgl)),
                'sha256': {str(path): launch.digest(path) for path in paths.values()}}
        candidate.write_text(json.dumps(data))
        return candidate

    def test_vanilla_candidate_attests_actual_retained_host_exports(self) -> None:
        import json
        import vanilla
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            data = json.loads(self._vanilla_candidate(root, root / 'java').read_text())
            host = Path(data['host'])
            with patch.object(launch, 'LOCAL', root):
                retained = vanilla.snapshot(host)
            data['host'] = str(retained)
            data['sha256'][str(retained)] = data['sha256'].pop(str(host))
            host.write_bytes(b'rebuilt source no longer matters')
            self.assertIn(retained, vanilla.validate_candidate(data))
            for content, message in (('fixture', 'fixture-mods'),
                                     ('unrelated', 'composed-host JNI'),
                                     ('modern', 'legacy composed-host JNI'),
                                     ('text', 'ELF')):
                with self.subTest(content=content):
                    if content == 'text':
                        retained.write_bytes(b'not a native host')
                    else:
                        self._host_library(retained, fixture_mods=content == 'fixture',
                                           composed=content != 'unrelated', legacy=content != 'modern')
                    data['sha256'][str(retained)] = launch.digest(retained)
                    with self.assertRaisesRegex(ValueError, message):
                        vanilla.validate_candidate(data)

    def test_vanilla_candidate_creation_attests_host_exports(self) -> None:
        import json
        import vanilla
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            data = json.loads(self._vanilla_candidate(root, root / 'java').read_text())
            work = root / 'tests/fixtures/local/legacy-platform'
            work.mkdir(parents=True)
            for source in [Path(data['platform']), *map(Path, data['lwjgl'])]:
                shutil.copyfile(source, work / source.name)
            agents = root / 'target/ravel-jvm'
            agents.mkdir(parents=True)
            for key, name in (('agent', 'ravel-agent.jar'), ('screen', 'ravel-screen.jar')):
                shutil.copyfile(data[key], agents / name)
            host = Path(data['host'])
            with patch.object(launch, 'LOCAL', root), patch.object(launch, 'ROOT', root), \
                 patch.object(launch, 'validate_java'), patch.object(vanilla, 'validate_agent'):
                candidate = root / 'accepted.json'
                vanilla.write_candidate(candidate, root / 'java', host)
                vanilla.validate_candidate(json.loads(candidate.read_text()))
                self._host_library(host, fixture_mods=True)
                rejected = root / 'rejected.json'
                with self.assertRaisesRegex(ValueError, 'fixture-mods'):
                    vanilla.write_candidate(rejected, root / 'java', host)
                self.assertFalse(rejected.exists())

    def test_geometry_proof_requires_ravel_vanilla_before_resolution(self) -> None:
        import vanilla
        for instance, workload in (('original', 'sevtech'), ('cleanroom', 'sevtech'),
                                   ('ravel', 'sevtech'), ('vanilla', 'vanilla')):
            with self.subTest(instance=instance, workload=workload):
                with self.assertRaisesRegex(ValueError, 'geometry-proof requires'):
                    launch.prepare_command(instance, Path('/unused'), workload=workload,
                                           renderer='geometry-proof')
        with self.assertRaisesRegex(ValueError, 'geometry-proof requires'):
            vanilla.prepare_command(Path('/unused'), 'vanilla', renderer='geometry-proof')
        with self.assertRaisesRegex(ValueError, 'unknown renderer'):
            launch.prepare_command('ravel', Path('/unused'), workload='vanilla', renderer='other')

    def test_renderer_selection_is_forwarded_to_vanilla_resolver(self) -> None:
        import vanilla
        with patch.object(vanilla, 'prepare_command', return_value=(['java'], Path('/game'))) as resolve:
            launch.prepare_command('ravel', Path('/trial'), Path('/candidate'),
                                   workload='vanilla', renderer='geometry-proof')
        resolve.assert_called_once_with(Path('/trial'), 'ravel', Path('/candidate'),
                                        renderer='geometry-proof')

    def test_matched_vanilla_commands_use_official_main_or_real_agent(self):
        import json
        import vanilla
        for instance, renderer in (('vanilla', 'vanilla'), ('ravel', 'vanilla'),
                                   ('ravel', 'geometry-proof')):
            with self.subTest(instance=instance, renderer=renderer), tempfile.TemporaryDirectory() as raw:
                root = Path(raw)
                out = root / 'trial'
                out.mkdir()
                name = 'vanilla-java8' if instance == 'vanilla' else 'vanilla-ravel'
                directory = root / 'prism/instances' / name
                game = directory / '.minecraft'
                game.mkdir(parents=True)
                natives = directory / 'natives-direct'
                natives.mkdir()
                (natives / 'liblwjgl64.so').write_bytes(b'native')
                observer = root / 'observer.jar'
                observer.write_bytes(b'observer')
                java = root / 'java'
                java.write_bytes(b'java')
                client = root / 'client.jar'
                client.write_bytes(b'client')
                library = root / 'authlib.jar'
                library.write_bytes(b'library')
                settings = {'fixture': 'vanilla-1.12.2', 'instance': instance,
                            'javaPath': str(java), 'jvmArgs': ['-javaagent:' + str(observer) + '=' + str(out)],
                            'minMemoryMiB': 4096, 'maxMemoryMiB': 8192,
                            'agentSha256': launch.digest(observer)}
                (out / 'settings.json').write_text(json.dumps(settings))
                official = {'mainClass': 'net.minecraft.client.main.Main',
                            'minecraftArguments': '--gameDir ${game_directory} --username ${auth_player_name}',
                            'assetIndex': {'id': '1.12'}, 'libraries': []}
                candidate = self._vanilla_candidate(root, java) if instance == 'ravel' else None
                candidate_bytes = candidate.read_bytes() if candidate else None
                with patch.object(launch, 'LOCAL', root), patch.object(launch, 'ROOT', root), \
                     patch.object(launch, 'official_metadata', return_value=(official, client)), \
                     patch.object(launch, 'resolve_libraries', return_value=([library], natives)), \
                     patch.object(launch, 'resolve_assets', return_value=root / 'assets'), \
                     patch.object(launch, 'validate_java', return_value='actual version'), \
                     patch.object(vanilla, 'validate_agent'):
                    if renderer == 'vanilla':
                        command, cwd = vanilla.prepare_command(out, instance, candidate)
                    else:
                        command, cwd = vanilla.prepare_command(out, instance, candidate, renderer=renderer)
                self.assertEqual(cwd, game)
                self.assertIn('-Xmx8192M', command)
                self.assertNotIn('--tweakClass', command)
                record = json.loads((out / 'command.json').read_text())
                self.assertTrue(all(len(row['sha256']) == 64 for row in record['provenance']['runtimeInputs']))
                if instance == 'vanilla':
                    self.assertIn('net.minecraft.client.main.Main', command)
                    self.assertNotIn('--module-path', command)
                    self.assertFalse(any(arg.startswith('-Dravel.renderer') for arg in command))
                else:
                    self.assertIn('dev.ravel.legacy.launch.LegacyLauncher', command)
                    self.assertIn('-Dravel.legacy.benchmark=true', command)
                    self.assertEqual(len([arg for arg in command if arg.startswith('-javaagent:')]), 2)
                    self.assertTrue(any(arg.startswith('-Dravel.host=') for arg in command))
                    selection = ['-Dravel.renderer=' + renderer,
                                 '-Dravel.renderer.metrics=' + str(out / 'geometry-upload.json')]
                    self.assertTrue(all(value in command for value in selection))
                    self.assertEqual(record['provenance']['rendererJvmArgs'], selection)
                    self.assertEqual(record['provenance']['renderer'], renderer)
                    self.assertEqual(candidate.read_bytes(), candidate_bytes)
                    child = directory / 'benchmark-child-classpath.txt'
                    self.assertIn(str(library), child.read_text())


if __name__ == '__main__':
    unittest.main()
