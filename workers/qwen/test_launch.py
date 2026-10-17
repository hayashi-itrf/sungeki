"""Subprocess fixtures only: no model, external service or real TTS is started."""
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
import unittest

LAUNCHER = Path(__file__).with_name('launch.py').resolve()
CACHE_KEYS = ('XDG_CACHE_HOME', 'HF_HOME', 'HF_HUB_CACHE', 'HF_ASSETS_CACHE',
              'HF_MODULES_CACHE', 'HF_XET_CACHE', 'TORCH_HOME')
LEGACY_KEYS = ('PYTORCH_PRETRAINED_BERT_CACHE', 'PYTORCH_TRANSFORMERS_CACHE',
               'TRANSFORMERS_CACHE')


class LauncherTest(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory(prefix='next qwen launcher ')
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name).resolve()
        self.paths = {name: self.root / name for name in ('engine', 'model', 'voice', 'work')}
        for path in self.paths.values():
            path.mkdir()
        (self.paths['engine'] / 'api').mkdir()
        (self.paths['engine'] / 'api/main.py').write_text('')
        (self.paths['model'] / 'config.json').write_text('{}')
        (self.paths['engine'] / 'uvicorn.py').write_text(
            'def run(app, **kwargs):\n'
            '    import os, json, sys\n'
            '    assert sys.dont_write_bytecode\n'
            f'    assert not any(key in os.environ for key in {LEGACY_KEYS!r})\n'
            "    print(json.dumps(dict(app=app, options=kwargs, cwd=os.getcwd(), env={k: os.environ[k] for k in ('HF_HUB_OFFLINE','TRANSFORMERS_OFFLINE','TTS_BACKEND','TTS_MODEL_ID','VOICE_LIBRARY_DIR','GPU_KEEPALIVE_INTERVAL','ENABLE_VOICE_STUDIO')})))\n"
        )
        self.listener = socket.socket()
        self.addCleanup(self.listener.close)
        self.listener.bind(('127.0.0.1', 0))
        self.listener.listen()

    def run_launcher(self, extra=(), listener=None):
        listener = listener or self.listener
        command = [sys.executable, '-I', str(LAUNCHER)]
        for name, path in self.paths.items():
            command += ['--' + name + '-root', str(path)]
        command += ['--port', str(self.listener.getsockname()[1]), '--listen-fd', str(listener.fileno())]
        return subprocess.run(command + list(extra), pass_fds=(listener.fileno(),),
                              env={**os.environ, 'TTS_BACKEND': 'unexpected', 'RELOAD': 'true',
                                   **{key: str(self.root / 'foreign') for key in CACHE_KEYS + LEGACY_KEYS}},
                              capture_output=True, text=True, timeout=10)

    def test_inherited_socket_offline_environment_and_separate_workdir(self):
        result = self.run_launcher()
        self.assertEqual(result.returncode, 0, result.stderr)
        value = json.loads(result.stdout)
        self.assertEqual(value['cwd'], str(self.paths['work']))
        self.assertEqual(value['app'], 'api.main:app')
        self.assertEqual(value['options'], dict(host='127.0.0.1', port=self.listener.getsockname()[1],
                                              fd=self.listener.fileno(), workers=1, reload=False, lifespan='on'))
        for key, expected in {'HF_HUB_OFFLINE': '1', 'TRANSFORMERS_OFFLINE': '1',
                              'TTS_BACKEND': 'official', 'TTS_MODEL_ID': str(self.paths['model']),
                              'VOICE_LIBRARY_DIR': str(self.paths['voice']),
                              'GPU_KEEPALIVE_INTERVAL': '0', 'ENABLE_VOICE_STUDIO': 'false'}.items():
            self.assertEqual(value['env'][key], expected)
        self.assertFalse(list(self.root.rglob('__pycache__')))
        # Child validation did not close the parent's reservation.
        self.assertEqual(self.listener.getsockname()[0], '127.0.0.1')

    def test_cache_writes_stay_in_next_work_directory(self):
        (self.paths['engine'] / 'uvicorn.py').write_text(
            'def run(*args, **kwargs):\n'
            '    import os\n'
            '    from pathlib import Path\n'
            f'    assert not any(key in os.environ for key in {LEGACY_KEYS!r})\n'
            f'    for key in {CACHE_KEYS!r}:\n'
            "        (Path(os.environ[key]) / (key + '.probe')).write_text('cache')\n"
        )
        result = self.run_launcher()
        self.assertEqual(result.returncode, 0, result.stderr)
        probes = list(self.root.rglob('*.probe'))
        self.assertEqual(len(probes), len(CACHE_KEYS))
        self.assertTrue(all(p.is_relative_to(self.paths['work']) for p in probes))
        self.assertFalse((self.root / 'foreign').exists())

    def test_cache_symlink_escape_is_rejected_before_import(self):
        (self.paths['work'] / 'cache').symlink_to(self.paths['model'], target_is_directory=True)
        result = self.run_launcher()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('cache directories must not contain symlinks', result.stderr)
        self.assertEqual(result.stdout, '')
        self.assertEqual(sorted(p.name for p in self.paths['model'].iterdir()), ['config.json'])

    def test_wrong_socket_and_missing_originals_are_rejected(self):
        with socket.socket() as wrong:
            wrong.bind(('127.0.0.1', 0))
            wrong.listen()
            self.assertNotEqual(self.run_launcher(listener=wrong).returncode, 0)
        (self.paths['model'] / 'config.json').unlink()
        self.assertNotEqual(self.run_launcher().returncode, 0)

    def test_work_inside_original_or_remote_model_is_rejected(self):
        self.assertNotEqual(self.run_launcher(['--work-root', str(self.paths['engine'])]).returncode, 0)
        self.assertNotEqual(self.run_launcher(['--model-root', 'Qwen/remote-model']).returncode, 0)
        self.assertNotEqual(self.run_launcher(['--listen-fd', '-1']).returncode, 0)


if __name__ == '__main__':
    unittest.main()
