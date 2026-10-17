import json
import os
import re
from pathlib import Path
import select
import socket
import struct
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]
HOST = ROOT / 'target/release/sungeki-host'
MCP = ROOT / 'target/release/sungeki-mcp'
PROTOCOL_MINOR = int(re.search(r'PROTOCOL_MINOR: u16 = (\d+)', (ROOT/'crates/protocol/src/lib.rs').read_text()).group(1))


def hello(major=1, minor=61):
    return {'type': 'hello', 'hello': {
        'application': 'sungeki-host', 'protocol_major': major, 'protocol_minor': minor,
        'build_id': 'integration', 'client_name': 'integration', 'capabilities': ['get_capabilities'],
    }}


def connect(directory):
    connection = socket.socket(socket.AF_UNIX)
    connection.settimeout(3)
    try:
        connection.connect(str(directory / 'host.sock'))
        return connection
    except Exception:
        connection.close()
        raise


def read_exact(connection, length):
    data = b''
    while len(data) < length:
        chunk = connection.recv(length - len(data))
        if not chunk:
            raise EOFError('host disconnected')
        data += chunk
    return data


def exchange(connection, body, request_id=1):
    encoded = json.dumps({'request_id': request_id, 'body': body}).encode()
    connection.sendall(struct.pack('>I', len(encoded)) + encoded)
    size = struct.unpack('>I', read_exact(connection, 4))[0]
    assert 0 < size <= 16 * 1024 * 1024
    response = json.loads(read_exact(connection, size))
    assert response['request_id'] == request_id
    return response['body']


class HostProcess:
    def __init__(self, directory, data_dir=None, binary=None, cwd=None):
        self.directory = directory
        self.process = subprocess.Popen([str(binary or HOST), '--runtime-dir', str(directory)], cwd=cwd,
                                        stdout=subprocess.DEVNULL, stderr=subprocess.PIPE,
                                        env=dict(os.environ, SUNGEKI_DATA_DIR=str(data_dir or directory / "data"),
                   SUNGEKI_REVIEW_WORK_ROOT=str((data_dir or directory / "data") / "recovery.sqlite.review-work")))

    def ready(self, seconds=5):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                raise AssertionError(self.process.stderr.read().decode())
            try:
                with connect(self.directory) as connection:
                    return exchange(connection, hello())['host']
            except (FileNotFoundError, ConnectionRefusedError):
                time.sleep(.02)
        raise AssertionError('host did not start')

    def close(self):
        if self.process.poll() is None:
            self.process.terminate()
        self.process.wait(timeout=5)
        self.process.stderr.close()


class McpClient:
    def __init__(self, directory, binary=HOST, data_dir=None, response_timeout=10):
        self.response_timeout = response_timeout
        env = dict(os.environ, SUNGEKI_RUNTIME_DIR=str(directory), SUNGEKI_HOST_BINARY=str(binary),
                   SUNGEKI_DATA_DIR=str(data_dir or directory / "data"),
                   SUNGEKI_REVIEW_WORK_ROOT=str((data_dir or directory / "data") / "recovery.sqlite.review-work"))
        self.process = subprocess.Popen([str(MCP)], cwd='/tmp', env=env,
                                        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                        stderr=subprocess.PIPE, text=True)
        self.sequence = 0
        self.call('initialize', {'protocolVersion': '2025-11-25', 'capabilities': {},
                                'clientInfo': {'name': 'integration', 'version': '1'}})
        self.send({'jsonrpc': '2.0', 'method': 'notifications/initialized'})

    def send(self, value):
        self.process.stdin.write(json.dumps(value) + '\n')
        self.process.stdin.flush()

    def call(self, method, params=None):
        self.sequence += 1
        self.send({'jsonrpc': '2.0', 'id': self.sequence, 'method': method, 'params': params or {}})
        if not select.select([self.process.stdout], [], [], self.response_timeout)[0]:
            raise AssertionError('MCP timed out')
        response = json.loads(self.process.stdout.readline())
        assert response['id'] == self.sequence
        return response

    def capabilities(self):
        return self.call('tools/call', {'name': 'get_capabilities', 'arguments': {}})['result']

    def close(self):
        self.process.stdin.close()
        try:
            self.process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait()
        self.process.stdout.close()
        self.process.stderr.close()
