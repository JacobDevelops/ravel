"""Resolver tests: no network or JVM execution."""
import hashlib
import io
from pathlib import Path
import stat
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
                    ['/actual/java', '-version'], capture_output=True, text=True, check=True)
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


if __name__ == '__main__':
    unittest.main()
