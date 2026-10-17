"""Fake SBV2 HTTP child for MCP checks; no real model or TTS engine."""
import json
from pathlib import Path
import socket
import sys


def configuration(root):
    root = root.resolve()
    for directory in ('engine', 'models/voice', 'text', 'work', 'venv/bin', 'base'):
        (root / directory).mkdir(parents=True)
    for name in ('engine/server_fastapi.py', 'engine/config.py', 'engine/config.yml',
                 'engine/launch.py', 'models/voice/model.safetensors',
                 'models/voice/style_vectors.npy', 'text/encoder'):
        (root / name).write_text('fixture')
    (root / 'venv/pyvenv.cfg').write_text('include-system-site-packages = false\n')
    (root / 'models/voice/config.json').write_text(json.dumps(dict(
        version='2.7.0-JP-Extra', data=dict(sampling_rate=44100, num_styles=1,
                                          spk2id={'Fixture': 0}, style2id={'Neutral': 0}))))
    catalog = {'0': dict(model_path=str(root / 'models/voice/model.safetensors'),
                        config_path=str(root / 'models/voice/config.json'), device='cpu',
                        spk2id={'Fixture': 0}, id2spk={'0': 'Fixture'}, style2id={'Neutral': 0})}
    executable = root / 'base/python-fixture'
    executable.write_text(f'''#!{sys.executable}
import http.server, io, json, os, socket, wave, time
from pathlib import Path
root=Path({str(root)!r})
(root/'pid').write_text(str(os.getpid()))
audio=io.BytesIO()
with wave.open(audio, 'wb') as w:
    w.setnchannels(1); w.setsampwidth(2); w.setframerate(44100)
    w.writeframes(b'\\x00\\x20'*4410)
class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args): pass
    def do_POST(self):
        self.do_GET()
    def do_GET(self):
        if self.path == '/models/info': body=json.dumps({catalog!r}).encode()
        elif self.path.startswith('/voice?'):
            with (root/'requests.jsonl').open('a') as log: log.write(json.dumps(self.path)+'\\n')
            while (root/'hold-voice').exists(): time.sleep(.01)
            body=b'not a WAV file' if (root/'invalid-audio').exists() else audio.getvalue()
        else:
            self.send_error(404); return
        self.send_response(200)
        self.send_header('Content-Length', str(len(body)))
        self.end_headers(); self.wfile.write(body)
server=http.server.HTTPServer(('127.0.0.1', 0), Handler, bind_and_activate=False)
server.socket.close(); server.socket=socket.socket(fileno=os.dup(0))
server.serve_forever()
''')
    executable.chmod(0o700)
    (root / 'venv/bin/python').symlink_to(executable)
    with socket.socket() as listener:
        listener.bind(('127.0.0.1', 0))
        port = listener.getsockname()[1]
    files = {name: str(root / 'engine' / name) for name in ('server_fastapi.py', 'config.py', 'config.yml')}
    files['next_launcher.py'] = str(root / 'engine/launch.py')
    binding = dict(endpoint=f'http://127.0.0.1:{port}', working_directory=str(root / 'engine'),
                   work_root=str(root / 'work'),
                   originals=dict(model=str(root / 'models/voice/model.safetensors'),
                                  config=str(root / 'models/voice/config.json'),
                                  style_vectors=str(root / 'models/voice/style_vectors.npy'),
                                  text_dependencies=dict(encoder=str(root / 'text/encoder'))),
                   engine=dict(executable=str(root / 'venv/bin/python'),
                               executable_target=str(executable), files=files))
    return dict(format_version=1, profiles=[dict(id='fixture', name='模擬SBV2',
                                                cpu_only=True, catalog=catalog, binding=binding)])
