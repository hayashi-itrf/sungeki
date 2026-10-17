"""Foreground development owner; production synthesis stays in the common host."""
import fcntl
import json
import os
from pathlib import Path
import runpy
import signal
import socket
import subprocess
import time

from profile import command, healthy, load
from preparation import prepare

# Share the existing development owner's process-group settlement and SSD policy.
_owner = runpy.run_path(str(Path(__file__).resolve().parents[1] / 'voicevox-dev/supervisor.py'))
stop = _owner['stop']
runtime = _owner['runtime']


def run(directory):
    lock = os.open(directory / 'owner.lock', os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        os.close(lock)
        raise RuntimeError('別のSungeki Qwen検証セッションが稼働中です')
    child = listener = reserved = log = None
    endpoint = directory / 'control.sock'
    stopping = False

    def interrupt(_number, _frame):
        nonlocal stopping
        stopping = True

    handlers = {sig: signal.signal(sig, interrupt) for sig in (signal.SIGTERM, signal.SIGINT)}
    try:
        endpoint.unlink(missing_ok=True)
        listener = socket.socket(socket.AF_UNIX)
        listener.bind(str(endpoint))
        os.chmod(endpoint, 0o600)
        listener.listen(4)
        listener.settimeout(.25)
        profile = prepare(listener, load, lambda: stopping)
        if profile is None or stopping:
            return
        # Keep the actual port reserved through spawn, then transfer it as stdin.
        # A probe followed by an independent bind would permit an adoption race.
        reserved = socket.socket()
        reserved.bind(('127.0.0.1', profile['port']))
        reserved.listen()
        root = runtime() / 'qwen-dev'
        root.mkdir(mode=0o700, exist_ok=True)
        log_path = root / f'engine-{time.time_ns()}.log'
        log = log_path.open('xb')
        os.chmod(log_path, 0o600)
        env = dict(os.environ)
        env.pop('PYTHONHOME', None)
        env.pop('PYTHONPATH', None)
        child = subprocess.Popen(command(profile), stdin=reserved, stdout=log, stderr=log,
                                 cwd=profile['binding']['work_root'], env=env,
                                 start_new_session=True, pass_fds=(lock,))
        reserved.close()
        reserved = None
        print(json.dumps(dict(state='STARTING', pid=child.pid, port=profile['port'],
                              log=str(log_path))), flush=True)
        stopped = False
        while not stopping:
            try:
                connection, _ = listener.accept()
            except socket.timeout:
                continue
            with connection:
                connection.settimeout(1)
                try:
                    action = connection.recv(64).decode().strip()
                    if action == 'stop':
                        stop(child)
                        stopped = True
                    if action not in ('status', 'stop'):
                        response = dict(state='INVALID_COMMAND')
                    elif stopped:
                        response = dict(state='STOPPED', returncode=child.returncode)
                    elif child.poll() is not None:
                        response = dict(state='FAILED', returncode=child.returncode)
                    else:
                        ready = healthy(profile)
                        response = dict(state='HEALTHY' if ready and child.poll() is None else 'STARTING',
                                        port=profile['port'])
                    connection.sendall((json.dumps(response) + '\n').encode())
                except (OSError, UnicodeError):
                    pass
    finally:
        try:
            if child is not None:
                stop(child)
        finally:
            if reserved is not None:
                reserved.close()
            if listener is not None:
                listener.close()
                endpoint.unlink(missing_ok=True)
            if log is not None:
                log.close()
            os.close(lock)
            for sig, handler in handlers.items():
                signal.signal(sig, handler)
