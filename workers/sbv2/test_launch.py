"""Fake engine subprocess tests: local test sockets only; no TTS model is loaded."""
import json
from pathlib import Path
import subprocess
import socket
import sys
import tempfile
import unittest
import signal
import select

LAUNCHER = Path(__file__).with_name("launch.py").resolve()


class LauncherTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="next sbv2 launcher ")
        self.addCleanup(self.temp.cleanup)
        self.listener = socket.socket()
        self.addCleanup(self.listener.close)
        self.listener.bind(("127.0.0.1", 0))
        self.listener.listen()
        self.port = self.listener.getsockname()[1]
        self.root = Path(self.temp.name).resolve() / "engine"
        self.root.mkdir()
        self.work = Path(self.temp.name).resolve() / "work"
        self.work.mkdir()
        self.models = Path(self.temp.name).resolve() / "models with spaces"
        self.models.mkdir()
        package = self.root
        for part in ("style_bert_vits2", "nlp", "japanese"):
            package = package / part
            package.mkdir()
            (package / "__init__.py").write_text("")
        (self.root / "style_bert_vits2/constants.py").write_text("DEFAULT_USER_DICT_DIR = None\n")
        for name in ("config.yml", "configs/paths.yml", "dict_data/default.csv"):
            target = self.root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text("original")
        (package / "pyopenjtalk_worker.py").write_text(
            "WORKER_CLIENT = None\n"
            "def initialize_worker():\n"
            "    raise AssertionError('must not connect to or detach shared worker')\n"
        )
        (self.root / "pyopenjtalk.py").write_text(
            "from pathlib import Path\nOPEN_JTALK_DICT_DIR = str(Path(__file__).parent / 'dict_data').encode()\n"
        )
        (self.root / "config.py").write_text(
            "from types import SimpleNamespace\n"
            "def get_config():\n"
            "    return SimpleNamespace(server_config=SimpleNamespace(port=5000))\n"
        )
        (self.root / "uvicorn.py").write_text(
            "def run(app, **kwargs):\n"
            "    import json\n"
            "    print(json.dumps(kwargs))\n"
        )
        (self.root / "server_fastapi.py").write_text(
            "import sys, json, uvicorn, os\n"
            "assert os.environ['HF_HUB_OFFLINE'] == '1'\n"
            "assert os.environ['TRANSFORMERS_OFFLINE'] == '1'\n"
            "from style_bert_vits2.nlp.japanese import pyopenjtalk_worker\n"
            "pyopenjtalk_worker.initialize_worker()\n"
            "assert pyopenjtalk_worker.WORKER_CLIENT is None\n"
            "from style_bert_vits2.constants import DEFAULT_USER_DICT_DIR\n"
            "from pathlib import Path\n"
            "assert 'TRANSFORMERS_CACHE' not in os.environ\n"
            "assert DEFAULT_USER_DICT_DIR.parent == Path.cwd()\n"
            "assert (DEFAULT_USER_DICT_DIR / 'default.csv').read_text() == 'original'\n"
            "(DEFAULT_USER_DICT_DIR / 'user.dic').write_text('compiled')\n"
            "assert Path(os.environ['HF_HOME']).is_relative_to(Path.cwd())\n"
            "from config import get_config\n"
            "print(json.dumps(sys.argv))\n"
            "uvicorn.run(None, host='0.0.0.0', port=get_config().server_config.port, log_level='warning')\n"
        )

    def run_launcher(self, *extra):
        return subprocess.run(
            [sys.executable, "-I", "-B", str(LAUNCHER), "--engine-root", str(self.root),
             "--work-root", str(self.work), "--model-root", str(self.models), "--port", str(self.port),
             "--listen-fd", str(self.listener.fileno()), *extra],
            capture_output=True, text=True, timeout=5, check=False,
            pass_fds=(self.listener.fileno(),),
        )

    def test_local_binding_and_exact_model_argument_without_engine_edits(self):
        original = (self.root / "config.py").read_bytes()
        for device in ("cpu", "auto"):
            result = self.run_launcher("--device", device)
            self.assertEqual(result.returncode, 0, result.stderr)
            argv, options = map(json.loads, result.stdout.splitlines())
            self.assertEqual(argv[:3], [str(self.root / "server_fastapi.py"), "--dir", str(self.models)])
            self.assertEqual(argv[3:], ["--cpu"] if device == "cpu" else [])
            self.assertEqual(options, {"host": "127.0.0.1", "port": self.port, "log_level": "warning", "fd": self.listener.fileno()})
            self.assertEqual((self.root / "config.py").read_bytes(), original)
            self.assertFalse((self.root / "__pycache__").exists())
            self.assertFalse((self.root / "dict_data/user.dic").exists())
            self.assertEqual(list(self.work.iterdir()), [])

    def test_invalid_ports_directories_device_and_missing_entrypoint_fail(self):
        for extra in [("--port", "0"), ("--port", "65536"),
                      ("--model-root", "relative"), ("--device", "gpu")]:
            result = self.run_launcher("--device", "cpu", *extra)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(result.stdout, "")
        (self.root / "server_fastapi.py").unlink()
        self.assertNotEqual(self.run_launcher("--device", "cpu").returncode, 0)

    def test_work_overlap_and_symlink_alias_fail_before_import(self):
        alias = self.work / "alias"
        alias.symlink_to(self.root, target_is_directory=True)
        for target in (self.root, self.models, Path(self.temp.name), alias):
            result = self.run_launcher("--device", "cpu", "--work-root", str(target))
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("separate from originals", result.stderr)
            self.assertEqual(result.stdout, "")

    def test_missing_openjtalk_dictionary_fails_without_download(self):
        (self.root / "pyopenjtalk.py").write_text("OPEN_JTALK_DICT_DIR = b'/missing-next-test-dictionary'\n")
        result = self.run_launcher("--device", "cpu")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("installed OpenJTalk dictionary is missing", result.stderr)
        self.assertEqual(list(self.work.iterdir()), [])

    def test_missing_dictionary_input_does_not_start_or_leave_work(self):
        (self.root / "dict_data/default.csv").unlink()
        result = self.run_launcher("--device", "cpu")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("required engine input missing", result.stderr)
        self.assertEqual(result.stdout, "")
        self.assertEqual(list(self.work.iterdir()), [])

    def test_wrong_descriptor_port_and_datagram_socket_fail_before_import(self):
        for extra in [("--listen-fd", "-1"), ("--listen-fd", "999999"),
                      ("--port", str(1024 if self.port != 1024 else 1025))]:
            result = self.run_launcher("--device", "cpu", *extra)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("invalid listening socket", result.stderr)
            self.assertEqual(result.stdout, "")
        self.listener.close()
        self.listener = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.addCleanup(self.listener.close)
        self.listener.bind(("127.0.0.1", self.port))
        result = self.run_launcher("--device", "cpu")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("invalid listening socket", result.stderr)

    def test_existing_shared_japanese_worker_state_is_not_adopted(self):
        worker = self.root / "style_bert_vits2/nlp/japanese/pyopenjtalk_worker.py"
        worker.write_text("WORKER_CLIENT = object()\n")
        result = self.run_launcher("--device", "cpu")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unsupported shared Japanese worker state", result.stderr)
        self.assertEqual(result.stdout, "")

    def test_termination_cleans_private_work_during_startup_and_server_shutdown(self):
        for phase in ("startup", "server"):
            with self.subTest(phase=phase):
                # Simulate both a pending import and Uvicorn's handler restore/
                # re-raise behavior without loading a model or opening a server.
                (self.root / "server_fastapi.py").write_text(
                    "import signal\nfrom pathlib import Path\n"
                    "Path('partial-cache').write_text('partial')\n"
                    + ("previous = signal.signal(signal.SIGTERM, lambda *_: None)\n"
                       if phase == "server" else "")
                    + "print('READY', flush=True)\nsignal.pause()\n"
                    + ("signal.signal(signal.SIGTERM, previous)\nsignal.raise_signal(signal.SIGTERM)\n"
                       if phase == "server" else "")
                )
                process = subprocess.Popen(
                    [sys.executable, "-I", "-B", str(LAUNCHER), "--engine-root", str(self.root),
                     "--work-root", str(self.work), "--model-root", str(self.models),
                     "--port", str(self.port), "--listen-fd", str(self.listener.fileno()),
                     "--device", "cpu"], stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                    text=True, pass_fds=(self.listener.fileno(),))
                try:
                    self.assertTrue(select.select([process.stdout], [], [], 5)[0])
                    self.assertEqual(process.stdout.readline().strip(), "READY")
                    self.assertEqual(len(list(self.work.iterdir())), 1)
                    process.send_signal(signal.SIGTERM)
                    _, errors = process.communicate(timeout=5)
                    self.assertEqual(process.returncode, 143, errors)
                    self.assertEqual(list(self.work.iterdir()), [])
                    self.assertEqual((self.root / "dict_data/default.csv").read_text(), "original")
                finally:
                    if process.poll() is None:
                        process.kill()
                    process.communicate(timeout=5)


if __name__ == "__main__":
    unittest.main()
