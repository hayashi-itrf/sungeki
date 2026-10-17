"""Real worker subprocess, synthetic vendor package; no TTS model is loaded."""
import http.client
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
import time
import unittest

LAUNCH = Path(__file__).with_name('launch.py').resolve()
FAKE = '''
import json, os
from pathlib import Path
from types import SimpleNamespace as NS
RuntimeKey = NS
SamplingRequest = NS
class InferenceRuntime:
    @classmethod
    def from_key(cls, key):
        Path('loaded.json').write_text(json.dumps(dict(
            key=vars(key), cwd=os.getcwd(), offline=os.environ['HF_HUB_OFFLINE'],
            cache=os.environ['HF_HOME'], hub=os.environ['HF_HUB_CACHE'], tmp=os.environ['TMPDIR'])))
        return cls()
    model_cfg = NS(use_speaker_condition_resolved=True, use_caption_condition=False)
    codec = NS(sample_rate=48000)
    def unload(self):
        Path('unloaded').touch()
'''


class LaunchTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='next irodori ')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.engine = self.root / 'engine'
        package = self.engine / 'irodori_tts'
        package.mkdir(parents=True)
        (package / '__init__.py').write_text('')
        (package / 'inference_runtime.py').write_text(FAKE)
        (package / 'text_normalization.py').write_text('normalize_text = lambda s: s\n')
        self.model = self.root / 'models'
        self.model.mkdir()
        (self.model / 'model.safetensors').write_bytes(b'fixture')
        (self.model / 'codec.pth').write_bytes(b'fixture')
        (self.model / 'voices.json').write_text(json.dumps([
            dict(id='none', name='参照なし', reference='none', caption=None)]))
        self.work = self.root / 'work'
        self.work.mkdir()
        self.listener = socket.socket()
        self.listener.bind(('127.0.0.1', 0))
        self.listener.listen()
        self.addCleanup(self.listener.close)
        self.port = self.listener.getsockname()[1]
        self.args = [sys.executable, '-I', '-B', str(LAUNCH),
            '--engine-root', str(self.engine), '--checkpoint', str(self.model / 'model.safetensors'),
            '--codec', str(self.model / 'codec.pth'), '--registry', str(self.model / 'voices.json'),
            '--work-root', str(self.work), '--checkpoint-id', 'fixture-model',
            '--listen-fd', '0', '--device', 'cpu']
    def start(self):
        process = subprocess.Popen(self.args, stdin=self.listener, stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE)
        self.addCleanup(self.reap, process)
        return process
    def reap(self, process):
        if process.poll() is None:
            process.terminate()
        try:
            process.communicate(timeout=3)
        except subprocess.TimeoutExpired:
            process.kill()
            process.communicate(timeout=3)
    def request(self, path):
        client = http.client.HTTPConnection('127.0.0.1', self.port, timeout=.2)
        try:
            client.request('GET', path)
            response = client.getresponse()
            return response.status, json.loads(response.read())
        finally:
            client.close()
    def ready(self, process):
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            if process.poll() is not None:
                self.fail(process.communicate()[1].decode())
            try:
                return self.request('/v1/health')
            except (OSError, http.client.HTTPException):
                time.sleep(.03)
        self.fail('worker did not become ready')
    def test_isolated_start_catalog_and_owned_process_stop(self):
        process = self.start()
        self.assertEqual(self.ready(process), (200, {'status': 'ready', 'busy': False}))
        status, catalog = self.request('/v1/catalog')
        self.assertEqual(status, 200)
        self.assertEqual(catalog['model']['checkpoint'], 'fixture-model')
        loaded = json.loads((self.work / 'loaded.json').read_text())
        self.assertEqual(loaded['key']['codec_repo'], str(self.model / 'codec.pth'))
        self.assertEqual(loaded['cwd'], str(self.work))
        self.assertEqual(loaded['offline'], '1')
        self.assertTrue(Path(loaded['cache']).is_relative_to(self.work))
        self.assertTrue(loaded['key']['codec_deterministic_decode'])
        self.assertEqual(list(self.engine.rglob('__pycache__')), [])
        self.reap(process)
        self.assertIsNotNone(process.returncode)
        # Closing the worker duplicate must not close the parent's descriptor.
        self.assertEqual(self.listener.getsockname(), ('127.0.0.1', self.port))
    def test_dependency_cache_is_explicit_and_separate(self):
        cache = self.root / 'dependencies'
        cache.mkdir()
        self.args += ['--dependency-cache', str(cache)]
        process = self.start()
        self.ready(process)
        loaded = json.loads((self.work / 'loaded.json').read_text())
        self.assertEqual(loaded['hub'], str(cache))
        self.assertEqual(list(cache.iterdir()), [])
        self.reap(process)
        self.args[-1] = str(self.work)
        process = self.start()
        _, error = process.communicate(timeout=3)
        self.assertNotEqual(process.returncode, 0)
        self.assertIn(b'cache must be separate', error)
    def test_work_cannot_overlap_originals(self):
        self.args[self.args.index('--work-root') + 1] = str(self.model)
        process = self.start()
        _, error = process.communicate(timeout=3)
        self.assertNotEqual(process.returncode, 0)
        self.assertIn(b'separate from originals', error)
        self.assertFalse((self.model / 'loaded.json').exists())
    def test_cache_symlink_rejected_before_loading(self):
        (self.work / 'cache').symlink_to(self.model, target_is_directory=True)
        process = self.start()
        _, error = process.communicate(timeout=3)
        self.assertNotEqual(process.returncode, 0)
        self.assertIn(b'must not contain symlinks', error)
        self.assertFalse((self.work / 'loaded.json').exists())
    def test_invalid_socket_rejected_before_loading(self):
        process = subprocess.Popen(self.args, stdin=subprocess.DEVNULL,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        self.addCleanup(self.reap, process)
        process.communicate(timeout=3)
        self.assertNotEqual(process.returncode, 0)
        self.assertFalse((self.work / 'loaded.json').exists())


if __name__ == '__main__':
    unittest.main()
