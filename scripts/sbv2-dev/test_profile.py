"""Read-only profile/control checks, no actual TTS process."""
import json
import os
from pathlib import Path
import tempfile
import threading
import unittest
from unittest.mock import patch, MagicMock
import profile as adapter


class ProfileTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='next-sbv2-profile-')
        self.addCleanup(self.temp.cleanup)
        self.path = Path(self.temp.name) / 'candidate.json'
        self.profile = dict(id='voice', cpu_only=True, catalog={'0': {'style2id': {'Neutral': 0}}},
                            binding=dict(endpoint='http://127.0.0.1:18881/',
                                         working_directory='/engine with spaces', work_root='/work',
                                         originals=dict(model='/models/voice/model.safetensors'),
                                         engine=dict(executable='/venv/bin/python',
                                                     files={'next_launcher.py': '/next/launch.py'})))
        self.path.write_text(json.dumps(dict(profiles=[self.profile])))
        self.environment = patch.dict(os.environ, SUNGEKI_SBV2_CONFIG=str(self.path),
                                      SUNGEKI_SBV2_PROFILE='voice')
        self.environment.start()
        self.addCleanup(self.environment.stop)

    def test_checked_profile_builds_exact_separate_arguments(self):
        with patch.object(adapter, 'inspect_candidate', return_value=json.dumps(dict(profiles=[dict(profile_id='voice')]))):
            profile = adapter.load(threading.Event())
        self.assertEqual(adapter.command(profile), [
            '/venv/bin/python', '-I', '-B', '/next/launch.py',
            '--engine-root', '/engine with spaces', '--model-root', '/models',
            '--work-root', '/work', '--port', '18881', '--device', 'cpu', '--listen-fd', '0'])
        profile['cpu_only'] = False
        self.assertIn('auto', adapter.command(profile))

    def test_candidate_changed_during_preflight_is_rejected(self):
        def inspect(*_):
            self.path.write_text('{}')
            return '{}'
        with patch.object(adapter, 'inspect_candidate', side_effect=inspect):
            with self.assertRaisesRegex(RuntimeError, '変更'):
                adapter.load(threading.Event())

    def test_unchecked_profile_or_link_is_rejected(self):
        with patch.object(adapter, 'inspect_candidate', return_value='{"profiles": []}'):
            with self.assertRaisesRegex(RuntimeError, '見つかりません'):
                adapter.load(threading.Event())
        link = self.path.with_name('link.json')
        link.symlink_to(self.path)
        with patch.dict(os.environ, SUNGEKI_SBV2_CONFIG=str(link)):
            with self.assertRaises(RuntimeError):
                adapter.load(threading.Event())

    def test_health_requires_exact_catalog_and_bounded_response(self):
        profile = dict(port=18881, catalog=self.profile['catalog'])
        connection = MagicMock()
        response = connection.getresponse.return_value
        response.status = 200
        with patch.object(adapter.http.client, 'HTTPConnection', return_value=connection):
            response.read.return_value = json.dumps(profile['catalog']).encode()
            self.assertTrue(adapter.healthy(profile))
            connection.request.assert_called_with('GET', '/models/info')
            for bad in [b'{}', b'not json', b'x' * (4 * 1024 * 1024 + 1)]:
                response.read.return_value = bad
                self.assertFalse(adapter.healthy(profile))
            response.status = 500
            self.assertFalse(adapter.healthy(profile))
        self.assertEqual(connection.close.call_count, 5)


if __name__ == '__main__':
    unittest.main()
