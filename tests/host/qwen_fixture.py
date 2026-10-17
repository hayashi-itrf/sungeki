"""Tiny fake Qwen process for stdio MCP integration; never loads a model."""
import json
from pathlib import Path
import socket
import subprocess
import sys

from support import HOST


def configuration(root):
    root = root.resolve()
    for directory in ('engine/api', 'model', 'voice', 'work', 'venv/bin'):
        (root / directory).mkdir(parents=True)
    for name, content in [('engine/api/main.py', 'fixture'), ('model/config.json', '{}'),
                          ('voice/mapping', 'fixture'), ('launch.py', 'fixture'),
                          ('venv/pyvenv.cfg', 'include-system-site-packages = false\n')]:
        (root / name).write_text(content)
    health = dict(status='healthy', backend=dict(name='official', model_id=str(root / 'model'), ready=True), version='fixture')
    models = dict(data=[dict(id='qwen3-tts')])
    voices = dict(voices=[dict(id='ono_anna', name='ono_anna')], languages=['japanese'])
    script = f'''#!{sys.executable}
import http.server, io, json, os, socket, wave, time
from pathlib import Path
root=Path({str(root)!r})
(root/'pid').write_text(str(os.getpid()))
audio=io.BytesIO()
with wave.open(audio, 'wb') as w:
    w.setnchannels(1); w.setsampwidth(2); w.setframerate(24000)
    w.writeframes(b'\\x00\\x20'*2400)
bodies={dict(zip(['/health', '/v1/models', '/v1/audio/voices'], [health, models, voices]))!r}
class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args): pass
    def reply(self, body):
        self.send_response(200)
        self.send_header('Content-Length', str(len(body)))
        self.end_headers(); self.wfile.write(body)
    def do_GET(self): self.reply(json.dumps(bodies[self.path]).encode())
    def do_POST(self):
        body=json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        with (root/'requests.jsonl').open('a') as log: log.write(json.dumps(body)+'\\n')
        while (root/'hold-voice').exists(): time.sleep(.01)
        self.reply(b'not a WAV file' if (root/'invalid-audio').exists() else audio.getvalue())
server=http.server.HTTPServer(('127.0.0.1', 0), Handler, bind_and_activate=False)
server.socket.close(); server.socket=socket.socket(fileno=os.dup(0))
server.serve_forever()
'''
    executable = root / 'python-fixture'
    executable.write_text(script)
    executable.chmod(0o700)
    (root / 'venv/bin/python').symlink_to(executable)
    with socket.socket() as listener:
        listener.bind(('127.0.0.1', 0))
        port = listener.getsockname()[1]
    engine = dict(zip(['api/main.py', 'next_launcher.py', 'python_executable', 'pyvenv.cfg'],
                      [str(root / p) for p in ['engine/api/main.py', 'launch.py', 'python-fixture', 'venv/pyvenv.cfg']]))
    binding = {f'{name}_root': str(root / name) for name in ('engine', 'model', 'voice', 'work')}
    binding.update(executable=str(root / 'venv/bin/python'), executable_target=str(executable),
                   launcher=str(root / 'launch.py'), originals=dict(model={}, engine=engine,
                   voice=dict(mapping=str(root / 'voice/mapping'))))
    setup = dict(format_version=1, id='fixture', name='模擬Qwen', binding=binding, port=port,
                 catalog=[health, models, voices], sample_rate=24000, channels=1,
                 default_settings=dict(model='qwen3-tts', voice='ono_anna', language='japanese', speed=1, instruct=None))
    source = root / 'setup.json'
    source.write_text(json.dumps(setup))
    result = subprocess.run([str(HOST), '--prepare-qwen-config', str(source)],
                            capture_output=True, text=True, check=True, timeout=10)
    return json.loads(result.stdout)
