"""Foreground development owner. Not the production host's engine lease manager."""
import fcntl
import http.client
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import time


def runtime():
    value = os.environ.get('SUNGEKI_RUNTIME_ROOT')
    if not value:
        raise RuntimeError('SUNGEKI_RUNTIME_ROOTに外部ディスク内の専用フォルダーを指定してください')
    root = Path(value)
    if not value or not root.is_absolute() or '..' in root.parts:
        raise RuntimeError('Sungeki runtimeには外部ディスク内の絶対パスが必要です')
    root = Path(os.path.normpath(root))
    if len(root.parts) < 4 or root.parts[1] != 'Volumes':
        raise RuntimeError('Sungeki runtimeは /Volumes/ディスク名/専用フォルダー 配下へ指定してください')
    mount = Path('/Volumes') / root.parts[2]
    if not mount.is_dir() or mount.stat().st_dev == mount.parent.stat().st_dev:
        raise RuntimeError('外部ディスクが未接続です')
    actual = root.resolve()
    if actual == mount.resolve() or not actual.is_relative_to(mount.resolve()):
        raise RuntimeError('runtime実体が外部ディスク外です')
    root.mkdir(parents=True, exist_ok=True)
    if not root.resolve().is_relative_to(mount.resolve()):
        raise RuntimeError('runtime実体が外部ディスク外です')
    return root.resolve()


def health(port):
    connection = http.client.HTTPConnection('127.0.0.1', port, timeout=0.5)
    try:
        connection.request('GET', '/version')
        response = connection.getresponse()
        if response.status != 200:
            return False
        version = json.loads(response.read(1025))
        return isinstance(version, str) and 0 < len(version) <= 128
    except (OSError, ValueError, http.client.HTTPException):
        return False
    finally:
        connection.close()


def stop(child):
    # poll(None) leaves an unreaped owned child. If it exits between poll and
    # killpg, its PID cannot be recycled until this parent reaps it.
    if child.poll() is None:
        os.killpg(child.pid, signal.SIGTERM)
        try:
            child.wait(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(child.pid, signal.SIGKILL)
            child.wait(timeout=5)
    # A surviving descendant is not STOPPED. Never kill a recycled group after reaping.
    try:
        os.killpg(child.pid, 0)
    except ProcessLookupError:
        return child.returncode
    raise RuntimeError("所有グループの残留プロセスを確認しました。停止済みとは判定しません")


def run(directory):
    lock_fd = os.open(directory / 'owner.lock', os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
    try:
        fcntl.flock(lock_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        os.close(lock_fd)
        raise RuntimeError('別のSungeki検証セッションがエンジンを使用中です')
    child = None
    listener = None
    log = None
    endpoint = directory / 'control.sock'
    stopping = False
    def interrupt(_number, _frame):
        nonlocal stopping
        stopping = True
    old_handlers = {sig: signal.signal(sig, interrupt) for sig in (signal.SIGTERM, signal.SIGINT)}
    try:
        port = int(os.environ.get('SUNGEKI_VOICEVOX_PORT', '55021'))
        if not 1024 <= port <= 65535:
            raise RuntimeError('開発用ポートは1024〜65535です')
        binary = Path(os.environ.get('SUNGEKI_VOICEVOX_ENGINE', '/Applications/VOICEVOX.app/Contents/Resources/vv-engine/run'))
        if not binary.is_absolute() or not binary.is_file() or not os.access(binary, os.X_OK):
            raise RuntimeError('VOICEVOXの実行可能な絶対パスを指定してください')
        # Never attach to or stop an already listening external engine.
        with socket.socket() as probe:
            probe.bind(('127.0.0.1', port))
        endpoint.unlink(missing_ok=True)  # only under the exclusive owner lock
        listener = socket.socket(socket.AF_UNIX)
        listener.bind(str(endpoint))
        os.chmod(endpoint, 0o600)
        listener.listen(4)
        listener.settimeout(0.25)
        root = runtime() / 'voicevox-dev'
        root.mkdir(mode=0o700, exist_ok=True)
        log_path = root / f'engine-{time.time_ns()}.log'
        log = log_path.open('xb')
        os.chmod(log_path, 0o600)
        scratch = root / 'tmp'
        scratch.mkdir(mode=0o700, exist_ok=True)
        child_env = dict(os.environ, TMPDIR=str(scratch))
        child = subprocess.Popen([str(binary), '--host', '127.0.0.1', '--port', str(port),
                                  '--setting_file', str(root / 'settings.yaml'),
                                  '--preset_file', str(root / 'presets.yaml'), '--disable_mutable_api'],
                                 stdin=subprocess.DEVNULL, stdout=log, stderr=log,
                                 start_new_session=True, pass_fds=(lock_fd,), env=child_env)
        print(json.dumps(dict(state='STARTING', pid=child.pid, port=port, log=str(log_path))), flush=True)
        stopped = False
        while not stopping:
            try:
                connection, _ = listener.accept()
            except socket.timeout:
                continue
            with connection:
                connection.settimeout(1)
                try:
                    command = connection.recv(64).decode().strip()
                    if command == 'stop':
                        stop(child)
                        stopped = True
                    if command not in ('status', 'stop'):
                        response = dict(state='INVALID_COMMAND')
                    elif stopped:
                        response = dict(state='STOPPED', returncode=child.returncode)
                    elif child.poll() is not None:
                        response = dict(state='FAILED', returncode=child.returncode)
                    else:
                        healthy = health(port)
                        response = dict(state='HEALTHY' if healthy and child.poll() is None else 'STARTING', port=port)
                    connection.sendall((json.dumps(response) + '\n').encode())
                except (OSError, UnicodeError):
                    pass
    finally:
        if child is not None:
            stop(child)
        if listener is not None:
            listener.close()
            endpoint.unlink(missing_ok=True)
        if log is not None:
            log.close()
        os.close(lock_fd)
        for sig, handler in old_handlers.items():
            signal.signal(sig, handler)
