"""Offline setup rejects invalid declarations and changed originals without spawning."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

from irodori_fixture import configuration
from support import HOST


class IrodoriSetupTest(unittest.TestCase):
    def test_invalid_setup_and_changed_original_are_rejected_offline(self):
        with tempfile.TemporaryDirectory(prefix='irodori-setup-', dir='/tmp') as directory:
            root = Path(directory).resolve()
            configuration(root)
            path = root / 'setup.json'
            original = json.loads(path.read_text())
            for field, value in [('format_version', 2), ('unexpected', True),
                                 ('default_settings', dict(original['default_settings'], voice_id='missing'))]:
                with self.subTest(field=field):
                    path.write_text(json.dumps(dict(original, **{field: value})))
                    result = subprocess.run([str(HOST), '--prepare-irodori-config', str(path)],
                                            capture_output=True, text=True, timeout=10)
                    self.assertNotEqual(result.returncode, 0)
                    self.assertEqual(result.stdout, '')
                    self.assertFalse((root / 'pid').exists())
            (root / 'model/checkpoint').write_text('changed original')
            result = subprocess.run([str(HOST), '--check-irodori-config', str(root / 'candidate.json')],
                                    capture_output=True, text=True, timeout=10)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(result.stdout, '')
            self.assertFalse((root / 'pid').exists())


if __name__ == '__main__':
    unittest.main()
