"""Synthetic engine and explicit manifests for the real stdio MCP path.

No TTS weights or vendor code are used. This is not a production setup tool.
"""
import json
import socket
import sys
import subprocess
from support import HOST


def configuration(root):
    root = root.resolve()
    for directory in ('engine/irodori_tts', 'model', 'codec', 'voice', 'work', 'venv/bin', 'worker'):
        (root / directory).mkdir(parents=True)
    for name in ('engine/irodori_tts/inference_runtime.py', 'model/checkpoint', 'codec/codec'):
        (root / name).write_text('fixture')
    (root / 'venv/pyvenv.cfg').write_text('include-system-site-packages = false\n')
    voices = [dict(id='none', name='参照なし', reference='none', caption=None)]
    (root / 'voice/registry').write_text(json.dumps(voices))
    model = dict(checkpoint='fixture-model', speaker_conditioning=True,
                 caption_conditioning=False, sample_rate=24000, channels=1)
    script = f'''#!{sys.executable}
import http.server, io, json, os, socket, wave, time
from pathlib import Path
root=Path({str(root)!r})
(root/'pid').write_text(str(os.getpid()))
audio=io.BytesIO()
with wave.open(audio, 'wb') as w:
    w.setnchannels(1); w.setsampwidth(2); w.setframerate(24000)
    w.writeframes(b'\\x00\\x20'*2400)
bodies={{'/v1/health':{{'status':'ready','busy':False}},'/v1/catalog':{dict(model=model, voices=voices)!r}}}
class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args): pass
    def reply(self, body, kind):
        self.send_response(200)
        self.send_header('Content-Type', kind)
        self.send_header('Content-Length', str(len(body)))
        self.end_headers(); self.wfile.write(body)
    def do_GET(self): self.reply(json.dumps(bodies[self.path]).encode(), 'application/json')
    def do_POST(self):
        body=json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        with (root/'requests.jsonl').open('a') as log: log.write(json.dumps(body)+'\\n')
        while (root/'hold-voice').exists(): time.sleep(.01)
        self.reply(b'not a WAV file' if (root/'invalid-audio').exists() else audio.getvalue(), 'audio/wav')
server=http.server.HTTPServer(('127.0.0.1', 0), Handler, bind_and_activate=False)
server.socket.close(); server.socket=socket.socket(fileno=os.dup(0))
server.serve_forever()
'''
    executable = root / 'python-fixture'
    executable.write_text(script)
    executable.chmod(0o700)
    (root / 'venv/bin/python').symlink_to(executable)
    engine = {key: str(root / path) for key, path in [
        ('irodori_tts/inference_runtime.py', 'engine/irodori_tts/inference_runtime.py'),
        ('python_executable', 'python-fixture'), ('pyvenv.cfg', 'venv/pyvenv.cfg')]}
    for name in ('launch.py', 'environment.py', 'bridge.py', 'audio.py', 'server.py'):
        path = root / 'worker' / name
        path.write_text('fixture')
        engine['next/' + name] = str(path)
    originals = dict(model={'checkpoint': str(root / 'model/checkpoint')},
        codec={'codec': str(root / 'codec/codec')}, engine=engine,
        voice={'registry': str(root / 'voice/registry')})
    with socket.socket() as listener:
        listener.bind(('127.0.0.1', 0))
        port = listener.getsockname()[1]
    binding = dict(engine_root=str(root / 'engine'), checkpoint=str(root / 'model/checkpoint'),
        codec=str(root / 'codec/codec'), registry=str(root / 'voice/registry'),
        work_root=str(root / 'work'), executable=str(root / 'venv/bin/python'),
        executable_target=str(executable), launcher=str(root / 'worker/launch.py'),
        device='cpu', precision='fp32', originals=originals)
    setup = dict(format_version=1, id='fixture', name='模擬Irodori',
        binding=binding, port=port, model=model,
        default_settings=dict(voice_id='none', caption=None, cfg_scale=3., num_steps=40, seed=42))
    path = root / 'setup.json'
    path.write_text(json.dumps(setup))
    result = subprocess.run([str(HOST), '--prepare-irodori-config', str(path)],
                            capture_output=True, text=True, check=True, timeout=10)
    candidate = json.loads(result.stdout)
    output = root / 'candidate.json'
    output.write_text(json.dumps(candidate))
    checked = subprocess.run([str(HOST), '--check-irodori-config', str(output)],
                             capture_output=True, text=True, check=True, timeout=10)
    report = json.loads(checked.stdout)
    assert report['synthesis_checked'] is False
    assert report['dependency_completeness_checked'] is False
    assert report['profiles'][0]['identity'] == candidate['profiles'][0]['identity']
    assert not (root / 'pid').exists()
    return candidate
