"""Development-only engine session: start (foreground), wait, stop, status.
No PID files or port-owner kill. The foreground supervisor owns its Popen child.
"""
import json
import os
from pathlib import Path
import socket
import stat
import sys
import time


def directory(engine):
    root = Path(f'/tmp/sungeki-{os.getuid()}-{engine}-dev')
    try:
        root.mkdir(mode=0o700)
    except FileExistsError:
        pass
    info = root.lstat()
    if not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid() or stat.S_IMODE(info.st_mode) != 0o700:
        raise RuntimeError('検証用制御ディレクトリの所有者/権限が不正です')
    return root


def request(command, engine):
    path = directory(engine) / 'control.sock'
    with socket.socket(socket.AF_UNIX) as connection:
        connection.settimeout(15)
        try:
            connection.connect(str(path))
        except (FileNotFoundError, ConnectionRefusedError):
            return dict(state='UNAVAILABLE')
        connection.sendall((command + '\n').encode())
        chunks = bytearray()
        while b'\n' not in chunks:
            part = connection.recv(4096)
            if not part:
                raise RuntimeError('管理セッションからの応答がありません')
            chunks.extend(part)
            if len(chunks) > 8192:
                raise RuntimeError('管理応答が大きすぎます')
        return json.loads(chunks)


def main():
    if len(sys.argv) != 3 or sys.argv[1] not in ('aivis', 'coeiroink') or sys.argv[2] not in ('start', 'wait', 'stop', 'status'):
        raise RuntimeError('Usage: {aivis|coeiroink}_ctl.sh {start|wait|stop|status}')
    engine, command = sys.argv[1:]
    if command == 'start':
        from supervisor import run
        run(directory(engine), engine)
        return
    if command == 'wait':
        deadline = time.monotonic() + 600
        while time.monotonic() < deadline:
            result = request('status', engine)
            if result['state'] == 'HEALTHY':
                print(json.dumps(result, ensure_ascii=False))
                return
            if result['state'] in ('STOPPED', 'FAILED'):
                raise RuntimeError(f'エンジンが停止しました: {result}')
            time.sleep(0.25)
        raise RuntimeError('エンジン起動を600秒以内に確認できませんでした')
    result = request(command, engine)
    print(json.dumps(result, ensure_ascii=False))
    if result['state'] == 'UNAVAILABLE':
        raise RuntimeError('所有中の管理セッションがありません。停止済みとは判定しません')
    if command == 'stop' and result['state'] != 'STOPPED':
        raise RuntimeError('所有プロセスの停止を確認できませんでした')


if __name__ == '__main__':
    try:
        main()
    except (RuntimeError, OSError, ValueError) as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
