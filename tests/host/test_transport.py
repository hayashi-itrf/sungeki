import concurrent.futures
import os
from pathlib import Path
import socket
import struct
import tempfile
import time
import unittest

from support import HOST, HostProcess, McpClient, connect, exchange, hello


class TransportTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='sungeki-test-', dir='/tmp')
        self.directory = Path(self.temporary.name) / 'runtime'
        self.addCleanup(self.temporary.cleanup)

    def host(self):
        process = HostProcess(self.directory)
        self.addCleanup(process.close)
        info = process.ready()
        return process, info

    def mcp(self, binary=HOST):
        client = McpClient(self.directory, binary)
        self.addCleanup(client.close)
        return client

    def test_shutdown_closes_idle_and_partial_request_connections(self):
        host, _ = self.host()
        with connect(self.directory) as idle, connect(self.directory) as partial:
            exchange(idle, hello())
            exchange(partial, hello())
            partial.sendall(struct.pack('>I', 100) + b'{')
            host.process.terminate()
            self.assertEqual(host.process.wait(timeout=3), 0)
            self.assertEqual(idle.recv(1), b'')
            self.assertEqual(partial.recv(1), b'')

    def test_clients_share_host_and_capabilities(self):
        _, info = self.host()
        self.assertEqual(os.stat(self.directory).st_mode & 0o777, 0o700)
        self.assertEqual(os.stat(self.directory / 'host.sock').st_mode & 0o777, 0o600)
        for _ in range(3):
            result = self.mcp().capabilities()
            self.assertFalse(result['isError'])
            self.assertEqual(result['structuredContent'], info)
            self.assertTrue(info['capabilities']['editing'])
            self.assertFalse(any(info['capabilities'][key] for key in ['speech', 'rendering', 'vrm']))

    def test_concurrent_hosts_converge_without_unlinking_live_socket(self):
        primary, before = self.host()
        inode = os.stat(self.directory / 'host.sock').st_ino
        others = [HostProcess(self.directory) for _ in range(8)]
        for process in others:
            self.addCleanup(process.close)
            self.assertEqual(process.process.wait(timeout=5), 0)
        self.assertIsNone(primary.process.poll())
        self.assertEqual(os.stat(self.directory / 'host.sock').st_ino, inode)
        with connect(self.directory) as connection:
            self.assertEqual(exchange(connection, hello())['host'], before)

    def test_major_mismatch_is_explicit_and_closes_connection(self):
        self.host()
        with connect(self.directory) as connection:
            result = exchange(connection, hello(999))
            self.assertEqual(result['error']['code'], 'PROTOCOL_MISMATCH')
            self.assertEqual(connection.recv(1), b'')

    def test_request_before_hello_is_rejected(self):
        self.host()
        with connect(self.directory) as connection:
            result = exchange(connection, {'type': 'get_capabilities'})
            self.assertEqual(result['error']['code'], 'VALIDATION_FAILED')

    def test_oversized_frame_cannot_poison_other_connections(self):
        _, info = self.host()
        with connect(self.directory) as connection:
            connection.sendall(struct.pack('>I', 65537))
            self.assertEqual(connection.recv(1), b'')
        with connect(self.directory) as connection:
            exchange(connection, hello())
            connection.sendall(struct.pack('>I', 16 * 1024 * 1024 + 1))
            self.assertEqual(connection.recv(1), b'')
        with connect(self.directory) as connection:
            rejected = exchange(connection, hello(minor=8))
            self.assertEqual(rejected['error']['code'], 'PROTOCOL_MISMATCH')
        self.assertEqual(self.mcp().capabilities()['structuredContent'], info)

    def test_stale_socket_recovered_only_after_lock(self):
        self.directory.mkdir(mode=0o700)
        stale = socket.socket(socket.AF_UNIX)
        stale.bind(str(self.directory / 'host.sock'))
        stale.close()
        self.host()
        self.assertFalse(self.mcp().capabilities()['isError'])

    def test_missing_sidecar_returns_structured_tool_error(self):
        result = self.mcp(Path('/missing/sungeki-host')).capabilities()
        self.assertTrue(result['isError'])
        self.assertEqual(result['structuredContent']['code'], 'SIDECAR_MISSING')
        self.assertFalse(self.directory.exists())

    def test_insecure_runtime_is_not_repaired_or_used(self):
        self.directory.mkdir(mode=0o755)
        result = self.mcp().capabilities()
        self.assertEqual(result['structuredContent']['code'], 'INVALID_RUNTIME')
        self.assertEqual(os.stat(self.directory).st_mode & 0o777, 0o755)

    def test_regular_file_at_socket_path_is_preserved(self):
        self.directory.mkdir(mode=0o700)
        path = self.directory / 'host.sock'
        path.write_text('do not delete')
        self.assertEqual(self.mcp().capabilities()['structuredContent']['code'], 'INVALID_RUNTIME')
        self.assertEqual(path.read_text(), 'do not delete')

    def test_runtime_symlink_is_rejected(self):
        destination = Path(self.temporary.name) / 'destination'
        destination.mkdir(mode=0o700)
        self.directory.symlink_to(destination)
        self.assertEqual(self.mcp().capabilities()['structuredContent']['code'], 'INVALID_RUNTIME')
        self.assertEqual(list(destination.iterdir()), [])

    def test_reconnect_after_owned_host_restart(self):
        first, before = self.host()
        client = self.mcp()
        self.assertEqual(client.capabilities()['structuredContent'], before)
        first.close()
        _, after = self.host()
        self.assertNotEqual(before['host_instance_id'], after['host_instance_id'])
        self.assertEqual(client.capabilities()['structuredContent'], after)

    def test_auto_start_from_multiple_mcp_clients_and_idle_exit(self):
        # No explicit host exists. All clients start concurrently in an isolated profile.
        clients = [self.mcp() for _ in range(6)]
        with concurrent.futures.ThreadPoolExecutor(max_workers=6) as executor:
            results = list(executor.map(lambda client: client.capabilities(), clients))
        try:
            ids = {result['structuredContent']['host_instance_id'] for result in results}
            self.assertEqual(len(ids), 1)
            self.assertTrue(all(not result['isError'] for result in results))
        finally:
            # The M0 host has no projects/jobs and owns a finite idle lifetime.
            deadline = time.monotonic() + 35
            while (self.directory / 'host.sock').exists() and time.monotonic() < deadline:
                time.sleep(.1)
            self.assertFalse((self.directory / 'host.sock').exists(), 'host did not exit after idle')


if __name__ == '__main__':
    unittest.main(verbosity=2)
