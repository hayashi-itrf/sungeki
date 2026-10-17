"""Serve control requests while the owned preflight subprocess inspects originals."""
from concurrent.futures import ThreadPoolExecutor
import json
import socket
import threading


def prepare(listener, load, interrupted):
    cancel = threading.Event()
    stopped = False
    with ThreadPoolExecutor(max_workers=1) as pool:
        future = pool.submit(load, cancel)
        try:
            while not interrupted():
                if future.done() and not stopped:
                    return future.result()
                try:
                    connection, _ = listener.accept()
                except socket.timeout:
                    continue
                with connection:
                    connection.settimeout(1)
                    try:
                        action = connection.recv(64).decode().strip()
                        if action == 'stop':
                            cancel.set()
                            # STOPPED is sent only after load has reaped its child.
                            try:
                                future.result(timeout=6)
                            except InterruptedError:
                                pass
                            stopped = True
                        state = ('STOPPED' if stopped else 'STARTING') if action in ('stop', 'status') else 'INVALID_COMMAND'
                        connection.sendall((json.dumps(dict(state=state, stage='original_inspection')) + '\n').encode())
                    except (OSError, UnicodeError):
                        pass
            return None
        finally:
            cancel.set()
