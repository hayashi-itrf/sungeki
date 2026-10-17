import json
from pathlib import Path
import socket
import struct
import tempfile
import threading
import time
import unittest

from support import HOST, McpClient, read_exact


class ClientFailureTests(unittest.TestCase):
    def test_pronunciation_requires_minor_twenty_one_without_replacing_host(self):
        reply = {'type': 'hello', 'host': {
            'application': 'sungeki-host', 'protocol_major': 1, 'protocol_minor': 20,
            'build_id': 'review', 'host_instance_id': 'existing-host',
            'capabilities': dict.fromkeys(['editing', 'persistence', 'speech', 'rendering', 'vrm'], False),
        }}
        thread, failures = self.fake_host(reply)
        inode = (self.directory/'host.sock').stat().st_ino
        client = McpClient(self.directory, Path('/missing/must-not-start'))
        self.addCleanup(client.close)
        result = client.call('tools/call', {'name': 'preview_pronunciation', 'arguments': {
            'session_id': 'explicit', 'expected_revision': 0, 'subtitle_text': 'ABC'}})['result']
        thread.join(timeout=5)
        self.assertEqual(failures, [])
        self.assertEqual(result['structuredContent']['code'], 'PROTOCOL_MISMATCH')
        self.assertEqual((self.directory/'host.sock').stat().st_ino, inode)

    def test_speech_commands_require_minor_twenty_two_without_replacing_host(self):
        reply = {'type': 'hello', 'host': {
            'application': 'sungeki-host', 'protocol_major': 1, 'protocol_minor': 21,
            'build_id': 'pronunciation', 'host_instance_id': 'existing-host',
            'capabilities': dict.fromkeys(['editing', 'persistence', 'speech', 'rendering', 'vrm'], False),
        }}
        thread, failures = self.fake_host(reply)
        inode = (self.directory/'host.sock').stat().st_ino
        client = McpClient(self.directory, Path('/missing/must-not-start'))
        self.addCleanup(client.close)
        result = client.call('tools/call', {'name': 'apply_commands', 'arguments': {
            'session_id': 'explicit', 'expected_revision': 0,
            'operation_id': '00000000-0000-4000-8000-000000000001',
            'commands': [{'type': 'remove_character', 'character_id': 'character'}]}})['result']
        thread.join(timeout=5)
        self.assertEqual(failures, [])
        self.assertEqual(result['structuredContent']['code'], 'PROTOCOL_MISMATCH')
        self.assertEqual((self.directory/'host.sock').stat().st_ino, inode)

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='sungeki-failure-', dir='/tmp')
        self.directory = Path(self.temporary.name)
        self.addCleanup(self.temporary.cleanup)

    def fake_host(self, reply=None, delay=0):
        listener = socket.socket(socket.AF_UNIX)
        listener.bind(str(self.directory / 'host.sock'))
        listener.listen(1)
        listener.settimeout(5)
        self.addCleanup(listener.close)
        failures = []

        def serve():
            try:
                connection, _ = listener.accept()
                with connection:
                    connection.settimeout(5)
                    size = struct.unpack('>I', read_exact(connection, 4))[0]
                    request = json.loads(read_exact(connection, size))
                    time.sleep(delay)
                    if reply is not None:
                        value = {'request_id': request['request_id'], 'body': reply}
                        encoded = json.dumps(value).encode()
                        connection.sendall(struct.pack('>I', len(encoded)) + encoded)
            except Exception as error:
                failures.append(error)

        thread = threading.Thread(target=serve)
        thread.start()
        self.addCleanup(thread.join, 6)
        return thread, failures

    def test_client_does_not_replace_or_start_over_incompatible_host(self):
        before = {'type': 'hello', 'host': {
            'application': 'sungeki-host', 'protocol_major': 999, 'protocol_minor': 0,
            'build_id': 'foreign', 'host_instance_id': 'foreign-host',
            'capabilities': dict.fromkeys(['editing', 'persistence', 'speech', 'rendering', 'vrm'], False),
        }}
        thread, failures = self.fake_host(before)
        inode = (self.directory / 'host.sock').stat().st_ino
        client = McpClient(self.directory, Path('/missing/should-never-launch'))
        self.addCleanup(client.close)
        result = client.capabilities()
        thread.join(timeout=5)
        self.assertEqual(failures, [])
        self.assertTrue(result['isError'])
        self.assertEqual(result['structuredContent']['code'], 'PROTOCOL_MISMATCH')
        self.assertEqual((self.directory / 'host.sock').stat().st_ino, inode)
        self.assertFalse((self.directory / 'host.lock').exists())

    def test_silent_host_has_finite_timeout(self):
        thread, failures = self.fake_host(delay=3)
        client = McpClient(self.directory)
        self.addCleanup(client.close)
        started = time.monotonic()
        result = client.capabilities()
        elapsed = time.monotonic() - started
        self.assertLess(elapsed, 4)
        self.assertEqual(result['structuredContent']['code'], 'HOST_UNAVAILABLE')
        thread.join(timeout=5)
        self.assertEqual(failures, [])

    def test_missing_runtime_parent_has_no_fallback(self):
        directory = self.directory / 'not-mounted' / 'runtime'
        client = McpClient(directory, HOST)
        self.addCleanup(client.close)
        result = client.capabilities()
        self.assertTrue(result['isError'])
        self.assertEqual(result['structuredContent']['code'], 'HOST_UNAVAILABLE')
        self.assertFalse(directory.exists())

    def test_checkpoint_requires_supported_minor_without_replacing_host(self):
        reply = {'type': 'hello', 'host': {
            'application': 'sungeki-host', 'protocol_major': 1, 'protocol_minor': 2,
            'build_id': 'events', 'host_instance_id': 'existing-host',
            'capabilities': dict.fromkeys(['editing', 'persistence', 'speech', 'rendering', 'vrm'], False),
        }}
        thread, failures = self.fake_host(reply)
        inode = (self.directory/'host.sock').stat().st_ino
        client = McpClient(self.directory, Path('/missing/must-not-start'))
        self.addCleanup(client.close)
        result = client.call('tools/call', {'name': 'list_checkpoints', 'arguments': {'session_id': 'explicit'}})['result']
        thread.join(timeout=5)
        self.assertEqual(failures, [])
        self.assertEqual(result['structuredContent']['code'], 'PROTOCOL_MISMATCH')
        self.assertEqual((self.directory/'host.sock').stat().st_ino, inode)

    def test_delete_requires_minor_five_without_replacing_host(self):
        reply = {'type': 'hello', 'host': {
            'application': 'sungeki-host', 'protocol_major': 1, 'protocol_minor': 4,
            'build_id': 'compare', 'host_instance_id': 'existing-host',
            'capabilities': dict.fromkeys(['editing', 'persistence', 'speech', 'rendering', 'vrm'], False),
        }}
        thread, failures = self.fake_host(reply)
        inode = (self.directory/'host.sock').stat().st_ino
        client = McpClient(self.directory, Path('/missing/must-not-start'))
        self.addCleanup(client.close)
        result = client.call('tools/call', {'name': 'delete_checkpoint', 'arguments': {
            'session_id': 'explicit', 'checkpoint_id': '00000000-0000-4000-8000-000000000001'}})['result']
        thread.join(timeout=5)
        self.assertEqual(failures, [])
        self.assertEqual(result['structuredContent']['code'], 'PROTOCOL_MISMATCH')
        self.assertEqual((self.directory/'host.sock').stat().st_ino, inode)

    def test_scene_commands_require_minor_six_without_replacing_host(self):
        reply = {'type': 'hello', 'host': {
            'application': 'sungeki-host', 'protocol_major': 1, 'protocol_minor': 5,
            'build_id': 'checkpoints', 'host_instance_id': 'existing-host',
            'capabilities': dict.fromkeys(['editing', 'persistence', 'speech', 'rendering', 'vrm'], False),
        }}
        thread, failures = self.fake_host(reply)
        inode = (self.directory/'host.sock').stat().st_ino
        client = McpClient(self.directory, Path('/missing/must-not-start'))
        self.addCleanup(client.close)
        result = client.call('tools/call', {'name': 'apply_commands', 'arguments': {
            'session_id': 'explicit', 'operation_id': '00000000-0000-4000-8000-000000000001',
            'expected_revision': 0, 'commands': [{'type': 'layout_scenes', 'scene_ids': [], 'start_us': 0, 'gap_us': 0}]}})['result']
        thread.join(timeout=5)
        self.assertEqual(failures, [])
        self.assertEqual(result['structuredContent']['code'], 'PROTOCOL_MISMATCH')
        self.assertEqual((self.directory/'host.sock').stat().st_ino, inode)

    def test_preview_requires_minor_seven_without_replacing_host(self):
        reply = {'type': 'hello', 'host': {
            'application': 'sungeki-host', 'protocol_major': 1, 'protocol_minor': 6,
            'build_id': 'scenes', 'host_instance_id': 'existing-host',
            'capabilities': dict.fromkeys(['editing', 'persistence', 'speech', 'rendering', 'vrm'], False),
        }}
        thread, failures = self.fake_host(reply)
        inode = (self.directory/'host.sock').stat().st_ino
        client = McpClient(self.directory, Path('/missing/must-not-start'))
        self.addCleanup(client.close)
        result = client.call('tools/call', {'name': 'apply_commands', 'arguments': {
            'session_id': 'explicit', 'expected_revision': 0, 'dry_run': True,
            'commands': [{'type': 'rename_project', 'title': 'Preview'}]}})['result']
        thread.join(timeout=5)
        self.assertEqual(failures, [])
        self.assertEqual(result['structuredContent']['code'], 'PROTOCOL_MISMATCH')
        self.assertEqual((self.directory/'host.sock').stat().st_ino, inode)

    def test_project_views_require_minor_eight_without_replacing_host(self):
        reply = {'type': 'hello', 'host': {
            'application': 'sungeki-host', 'protocol_major': 1, 'protocol_minor': 7,
            'build_id': 'preview', 'host_instance_id': 'existing-host',
            'capabilities': dict.fromkeys(['editing', 'persistence', 'speech', 'rendering', 'vrm'], False),
        }}
        thread, failures = self.fake_host(reply)
        inode = (self.directory/'host.sock').stat().st_ino
        client = McpClient(self.directory, Path('/missing/must-not-start'))
        self.addCleanup(client.close)
        result = client.call('tools/call', {'name': 'get_project', 'arguments': {
            'session_id': 'explicit', 'view': 'summary'}})['result']
        thread.join(timeout=5)
        self.assertEqual(failures, [])
        self.assertEqual(result['structuredContent']['code'], 'PROTOCOL_MISMATCH')
        self.assertEqual((self.directory/'host.sock').stat().st_ino, inode)

    def test_full_document_requires_minor_nine_without_replacing_host(self):
        reply = {'type': 'hello', 'host': {
            'application': 'sungeki-host', 'protocol_major': 1, 'protocol_minor': 8,
            'build_id': 'views', 'host_instance_id': 'existing-host',
            'capabilities': dict.fromkeys(['editing', 'persistence', 'speech', 'rendering', 'vrm'], False),
        }}
        thread, failures = self.fake_host(reply)
        inode = (self.directory/'host.sock').stat().st_ino
        client = McpClient(self.directory, Path('/missing/must-not-start'))
        self.addCleanup(client.close)
        result = client.call('tools/call', {'name': 'get_project', 'arguments': {'session_id': 'explicit'}})['result']
        thread.join(timeout=5)
        self.assertEqual(failures, [])
        self.assertEqual(result['structuredContent']['code'], 'PROTOCOL_MISMATCH')
        self.assertEqual((self.directory/'host.sock').stat().st_ino, inode)

    def test_image_probe_requires_minor_ten_without_replacing_host(self):
        reply = {'type': 'hello', 'host': {
            'application': 'sungeki-host', 'protocol_major': 1, 'protocol_minor': 9,
            'build_id': 'document-transfer', 'host_instance_id': 'existing-host',
            'capabilities': dict.fromkeys(['editing', 'persistence', 'speech', 'rendering', 'vrm'], False),
        }}
        thread, failures = self.fake_host(reply)
        inode = (self.directory/'host.sock').stat().st_ino
        client = McpClient(self.directory, Path('/missing/must-not-start'))
        self.addCleanup(client.close)
        result = client.call('tools/call', {'name': 'probe_image', 'arguments': {'path': '/tmp/image.png'}})['result']
        thread.join(timeout=5)
        self.assertEqual(failures, [])
        self.assertEqual(result['structuredContent']['code'], 'PROTOCOL_MISMATCH')
        self.assertEqual((self.directory/'host.sock').stat().st_ino, inode)

    def test_voice_catalog_requires_minor_twenty_three_without_replacing_host(self):
        reply = {'type': 'hello', 'host': {
            'application': 'sungeki-host', 'protocol_major': 1, 'protocol_minor': 22,
            'build_id': 'speech-model', 'host_instance_id': 'existing-host',
            'capabilities': dict.fromkeys(['editing', 'persistence', 'speech', 'rendering', 'vrm'], False),
        }}
        thread, failures = self.fake_host(reply)
        inode = (self.directory/'host.sock').stat().st_ino
        client = McpClient(self.directory, Path('/missing/must-not-start'))
        self.addCleanup(client.close)
        result = client.call('tools/call', {'name': 'discover_voicevox', 'arguments': {
            'base_url': 'http://127.0.0.1:50021'}})['result']
        thread.join(timeout=5)
        self.assertEqual(failures, [])
        self.assertEqual(result['structuredContent']['code'], 'PROTOCOL_MISMATCH')
        self.assertEqual((self.directory/'host.sock').stat().st_ino, inode)

    def test_speech_jobs_require_minor_twenty_five_without_replacing_host(self):
        reply = {'type': 'hello', 'host': {
            'application': 'sungeki-host', 'protocol_major': 1, 'protocol_minor': 24,
            'build_id': 'voice-details', 'host_instance_id': 'existing-host',
            'capabilities': dict.fromkeys(['editing', 'persistence', 'speech', 'rendering', 'vrm'], False),
        }}
        thread, failures = self.fake_host(reply)
        inode = (self.directory/'host.sock').stat().st_ino
        client = McpClient(self.directory, Path('/missing/must-not-start'))
        self.addCleanup(client.close)
        result = client.call('tools/call', {'name': 'get_speech_job', 'arguments': {'job_id': 'unknown'}})['result']
        thread.join(timeout=5)
        self.assertEqual(failures, [])
        self.assertEqual(result['structuredContent']['code'], 'PROTOCOL_MISMATCH')
        self.assertEqual((self.directory/'host.sock').stat().st_ino, inode)
