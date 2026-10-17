"""Inspect through the production host before constructing a development command."""
import http.client
import json
import os
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]


def inspect_candidate(candidate, cancel):
    child = subprocess.Popen([str(ROOT / 'target/release/sungeki-host'),
                              '--check-qwen-config', str(candidate)],
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    deadline = time.monotonic() + 600
    try:
        while True:
            if cancel.is_set():
                raise InterruptedError('Qwen原本照合を取り消しました')
            if time.monotonic() >= deadline:
                raise TimeoutError('Qwen原本照合がタイムアウトしました')
            try:
                stdout, stderr = child.communicate(timeout=.25)
                if child.returncode:
                    raise RuntimeError(f'Qwen原本照合失敗: {stderr}')
                return stdout
            except subprocess.TimeoutExpired:
                continue
    finally:
        if child.poll() is None:
            child.terminate()
            try:
                child.wait(timeout=2)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait(timeout=2)
        child.stdout.close()
        child.stderr.close()


def load(cancel):
    candidate = Path(os.environ['SUNGEKI_QWEN_CONFIG'])
    profile_id = os.environ['SUNGEKI_QWEN_PROFILE']
    if not candidate.is_absolute() or candidate.is_symlink():
        raise RuntimeError('Qwen候補には通常ファイルの絶対パスが必要です')
    before = candidate.read_bytes()
    result = inspect_candidate(candidate, cancel)
    if candidate.read_bytes() != before:
        raise RuntimeError('検査中にQwen候補が変更されました')
    profile = next((p for p in json.loads(before)['profiles'] if p['id'] == profile_id), None)
    measured = next((p for p in json.loads(result)['profiles'] if p['profile_id'] == profile_id), None)
    if profile is None or measured is None or measured['identity'] != profile['identity']:
        raise RuntimeError('検査済みQwen設定が見つかりません')
    return profile


def command(profile):
    binding = profile['binding']
    args = [binding['executable'], '-I', '-B', binding['launcher']]
    for name in ('engine', 'model', 'voice', 'work'):
        args += [f'--{name}-root', binding[f'{name}_root']]
    return args + ['--port', str(profile['port']), '--listen-fd', '0']


def healthy(profile):
    connection = http.client.HTTPConnection('127.0.0.1', profile['port'], timeout=.5)
    try:
        connection.request('GET', '/health')
        response = connection.getresponse()
        if response.status != 200:
            return False
        value = json.loads(response.read(65537))
        backend = value.get('backend') if isinstance(value, dict) else None
        if not isinstance(backend, dict):
            return False
        identity = profile['identity']
        return (value.get('status') == 'healthy' and backend.get('ready') is True
                and backend.get('name') == identity['backend']
                and backend.get('model_id') == identity['model_id'])
    except (OSError, ValueError, http.client.HTTPException):
        return False
    finally:
        connection.close()
