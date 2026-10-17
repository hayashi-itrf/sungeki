"""Storage path checks only; no engine is started."""
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from supervisor import runtime


class RuntimePaths(unittest.TestCase):
    def test_unspecified_root_does_not_access_default_disk(self):
        with patch.dict(os.environ, {}, clear=True), patch.object(Path, 'is_dir') as disk_check:
            with self.assertRaisesRegex(RuntimeError, 'SUNGEKI_RUNTIME_ROOT'):
                runtime()
            disk_check.assert_not_called()

    def test_invalid_paths(self):
        for value in ('', 'relative/path', '/tmp/next', '/Volumes', '/Volumes/SSD', '/Volumes/SSD/../../tmp/next', '/Volumes/SSD/a/../runtime'):
            with self.subTest(value=value), patch.dict(os.environ, SUNGEKI_RUNTIME_ROOT=value):
                with self.assertRaises(RuntimeError):
                    runtime()

    def test_absent_volume_creates_nothing(self):
        mount = Path('/Volumes') / f'vvm-absent-python-{os.getpid()}'
        self.assertFalse(mount.exists())
        with patch.dict(os.environ, SUNGEKI_RUNTIME_ROOT=str(mount / 'runtime')):
            with self.assertRaises(RuntimeError):
                runtime()
        self.assertFalse(mount.exists())

    @unittest.skipUnless(os.environ.get('SUNGEKI_RUNTIME_TEST_ROOT'), 'explicit external test root required')
    def test_external_paths_and_escape(self):
        root = Path(os.environ['SUNGEKI_RUNTIME_TEST_ROOT'])
        with tempfile.TemporaryDirectory(dir=root) as owned, tempfile.TemporaryDirectory() as outside:
            target = Path(owned) / 'nested' / 'runtime'
            with patch.dict(os.environ, SUNGEKI_RUNTIME_ROOT=str(target)):
                self.assertEqual(runtime(), target.resolve())
            (Path(owned) / 'escape').symlink_to(outside)
            with patch.dict(os.environ, SUNGEKI_RUNTIME_ROOT=str(Path(owned) / 'escape' / 'must-not-create')):
                with self.assertRaises(RuntimeError):
                    runtime()
            self.assertFalse((Path(outside) / 'must-not-create').exists())


if __name__ == '__main__':
    unittest.main()
