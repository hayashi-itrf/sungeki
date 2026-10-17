"""Every executable entry must keep a signed bundle's resources unchanged."""
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


class BundleEntrypointsTest(unittest.TestCase):
    def test_cli_imports_never_write_bytecode_into_the_bundle(self):
        source = Path(__file__).parent
        entries = ['render_frame.py', 'render_batch.py', 'psd_pose.py',
                   'psd_preview.py', 'video_preview.py', 'probe.py']
        env = {k: v for k, v in os.environ.items()
               if k not in ('PYTHONDONTWRITEBYTECODE', 'PYTHONPYCACHEPREFIX', 'PYTHONPATH')}
        for entry in entries:
            with self.subTest(entry=entry), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                bundle = root / 'image-worker'
                bundle.mkdir()
                for path in source.glob('*.py'):
                    if not path.name.startswith('test_'):
                        shutil.copyfile(path, bundle / path.name)
                def hashes():
                    return {p.name: hashlib.sha256(p.read_bytes()).hexdigest()
                            for p in bundle.iterdir() if p.is_file()}
                before = hashes()
                subprocess.run([sys.executable, str(bundle / entry), '--help'], cwd=root,
                               env=env, check=True, capture_output=True, timeout=30)
                self.assertFalse((bundle / '__pycache__').exists())
                self.assertEqual(before, hashes())
