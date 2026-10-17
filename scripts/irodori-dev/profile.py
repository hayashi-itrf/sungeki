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
                              '--check-irodori-config', str(candidate)],
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    deadline = time.monotonic() + 600
    try:
        while True:
            if cancel.is_set():
                raise InterruptedError('Irodori原本照合を取り消しました')
            if time.monotonic() >= deadline:
                raise TimeoutError('Irodori原本照合がタイムアウトしました')
            try:
                stdout, stderr = child.communicate(timeout=.25)
                if child.returncode:
                    raise RuntimeError(f'Irodori原本照合失敗: {stderr}')
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
    candidate = Path(os.environ['SUNGEKI_IRODORI_CONFIG'])
    profile_id = os.environ['SUNGEKI_IRODORI_PROFILE']
    if not candidate.is_absolute() or candidate.is_symlink():
        raise RuntimeError('Irodori候補には通常ファイルの絶対パスが必要です')
    before = candidate.read_bytes()
    result = inspect_candidate(candidate, cancel)
    if candidate.read_bytes() != before:
        raise RuntimeError('検査中にIrodori候補が変更されました')
    profile = next((p for p in json.loads(before)['profiles'] if p['id'] == profile_id), None)
    measured = next((p for p in json.loads(result)['profiles'] if p['profile_id'] == profile_id), None)
    if profile is None or measured is None or measured['identity'] != profile['identity']:
        raise RuntimeError('検査済みIrodori設定が見つかりません')
    return profile


def command(profile):
    binding = profile['binding']
    args = [binding['executable'], '-I', '-B', binding['launcher']]
    for name in ('engine_root', 'checkpoint', 'codec', 'registry', 'work_root', 'dependency_cache'):
        if binding.get(name) is not None:
            args += ['--' + name.replace('_', '-'), binding[name]]
    return args + ['--checkpoint-id', profile['identity']['checkpoint'],
                   '--device', binding['device'], '--precision', binding['precision'], '--listen-fd', '0']


def get(profile, path):
    connection = http.client.HTTPConnection('127.0.0.1', profile['port'], timeout=.5)
    try:
        connection.request('GET', path)
        response = connection.getresponse()
        if response.status != 200:
            return None
        raw = response.read(4 * 1024 * 1024 + 1)
        if len(raw) > 4 * 1024 * 1024:
            return None
        return json.loads(raw)
    finally:
        connection.close()


def healthy(profile):
    try:
        state = get(profile, '/v1/health')
        if not isinstance(state, dict) or state.get('status') != 'ready' or state.get('busy') is not False:
            return False
        catalog = get(profile, '/v1/catalog')
        return (isinstance(catalog, dict) and catalog.get('model') == profile['model']
                and sorted(catalog.get('voices', []), key=lambda v: v['id'])
                == sorted(profile['voices'], key=lambda v: v['id']))
    except (OSError, ValueError, KeyError, TypeError, http.client.HTTPException):
        return False
