"""Inspect through the production host before constructing a development command."""
import http.client
import json
import os
from pathlib import Path
import subprocess
import stat
import time

ROOT = Path(__file__).resolve().parents[2]


def inspect_candidate(candidate, cancel):
    child = subprocess.Popen([str(ROOT / 'target/release/sungeki-host'),
                              '--check-sbv2-config', str(candidate)],
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    deadline = time.monotonic() + 600
    try:
        while True:
            if cancel.is_set():
                raise InterruptedError('SBV2原本照合を取り消しました')
            if time.monotonic() >= deadline:
                raise TimeoutError('SBV2原本照合がタイムアウトしました')
            try:
                stdout, stderr = child.communicate(timeout=.25)
                if child.returncode:
                    raise RuntimeError(f'SBV2原本照合失敗: {stderr}')
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
    candidate = Path(os.environ['SUNGEKI_SBV2_CONFIG'])
    profile_id = os.environ['SUNGEKI_SBV2_PROFILE']
    if (not candidate.is_absolute() or '..' in candidate.parts or candidate.is_symlink()
            or not stat.S_ISREG(candidate.stat().st_mode) or candidate.stat().st_size > 32 * 1024 * 1024):
        raise RuntimeError('SBV2候補には通常ファイルの絶対パスが必要です')
    before = candidate.read_bytes()
    result = inspect_candidate(candidate, cancel)
    if candidate.read_bytes() != before:
        raise RuntimeError('検査中にSBV2候補が変更されました')
    profile = next((p for p in json.loads(before)['profiles'] if p['id'] == profile_id), None)
    measured = next((p for p in json.loads(result)['profiles'] if p['profile_id'] == profile_id), None)
    if profile is None or measured is None:
        raise RuntimeError('検査済みSBV2設定が見つかりません')
    endpoint = profile['binding']['endpoint'].rstrip('/')
    prefix = 'http://127.0.0.1:'
    if not endpoint.startswith(prefix) or not endpoint[len(prefix):].isdigit():
        raise RuntimeError('SBV2の管理接続先が不正です')
    profile['port'] = int(endpoint[len(prefix):])
    if not 1024 <= profile['port'] <= 65535:
        raise RuntimeError('SBV2の管理ポートが不正です')
    return profile


def command(profile):
    binding = profile['binding']
    return [binding['engine']['executable'], '-I', '-B',
            binding['engine']['files']['next_launcher.py'],
            '--engine-root', binding['working_directory'],
            '--model-root', str(Path(binding['originals']['model']).parent.parent),
            '--work-root', binding['work_root'], '--port', str(profile['port']),
            '--device', 'cpu' if profile['cpu_only'] else 'auto', '--listen-fd', '0']


def healthy(profile):
    connection = http.client.HTTPConnection('127.0.0.1', profile['port'], timeout=.5)
    try:
        connection.request('GET', '/models/info')
        response = connection.getresponse()
        if response.status != 200:
            return False
        data = response.read(4 * 1024 * 1024 + 1)
        if len(data) > 4 * 1024 * 1024:
            return False
        # Compare paths, devices, speakers and styles, not merely HTTP readiness.
        return json.loads(data) == profile['catalog']
    except (OSError, ValueError, http.client.HTTPException):
        return False
    finally:
        connection.close()
