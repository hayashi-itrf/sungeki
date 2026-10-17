"""Development SBV2 start/wait/stop/status; never adopts an external server."""
import json
import os
from pathlib import Path
import socket
import stat
import subprocess
import sys
import time


def directory():
    root = Path(f'/tmp/sungeki-{os.getuid()}-sbv2-dev')
    root.mkdir(mode=0o700, exist_ok=True)
    info = root.lstat()
    if not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid() or stat.S_IMODE(info.st_mode) != 0o700:
        raise RuntimeError('SBV2管理ディレクトリの所有者・権限が不正です')
    return root


def request(command):
    with socket.socket(socket.AF_UNIX) as connection:
        connection.settimeout(15)
        try:
            connection.connect(str(directory() / 'control.sock'))
        except (FileNotFoundError, ConnectionRefusedError):
            return dict(state='UNAVAILABLE')
        connection.sendall((command + '\n').encode())
        data = bytearray()
        while b'\n' not in data:
            part = connection.recv(4096)
            if not part or len(data) + len(part) > 8192:
                raise RuntimeError('SBV2管理応答が不正です')
            data.extend(part)
        return json.loads(data)


def main():
    if len(sys.argv) != 2 or sys.argv[1] not in ('start', 'wait', 'stop', 'status'):
        raise RuntimeError('Usage: sbv2_ctl.sh {start|wait|stop|status}')
    command = sys.argv[1]
    if command == 'start':
        from supervisor import run
        run(directory())
        return
    if command == 'wait':
        deadline = time.monotonic() + 1200
        while time.monotonic() < deadline:
            result = request('status')
            if result['state'] == 'HEALTHY':
                print(json.dumps(result, ensure_ascii=False))
                return
            if result['state'] in ('STOPPED', 'FAILED'):
                raise RuntimeError(f'SBV2が停止しました: {result}')
            time.sleep(.5)
        raise RuntimeError('SBV2起動待機がタイムアウトしました。所有セッションをstopしてください')
    result = request(command)
    print(json.dumps(result, ensure_ascii=False))
    if result['state'] == 'UNAVAILABLE':
        raise RuntimeError('管理セッション不在です。停止済みとは判定しません')
    if command == 'stop' and result['state'] != 'STOPPED':
        raise RuntimeError('SBV2停止を確認できませんでした')


if __name__ == '__main__':
    try:
        main()
    except (RuntimeError, OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
