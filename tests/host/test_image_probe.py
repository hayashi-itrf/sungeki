import hashlib
import struct
import tempfile
import unittest
import zlib
from pathlib import Path
from support import HostProcess, McpClient
from test_mcp_editing import call


def png():
    def chunk(kind, payload):
        return struct.pack('>I', len(payload)) + kind + payload + struct.pack('>I', zlib.crc32(kind + payload))
    return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 2, 3, 8, 6, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress((b'\0' + bytes([255, 0, 0, 128]) * 2) * 3))
            + chunk(b'IEND', b''))


class ImageProbeTest(unittest.TestCase):
    def test_mcp_inspects_real_image_without_mutating_or_attaching(self):
        with tempfile.TemporaryDirectory(prefix='vvm-probe-', dir='/tmp') as directory:
            root = Path(directory)
            host = HostProcess(root)
            client = None
            try:
                host.ready()
                client = McpClient(root)
                path = root / '透過素材.wrong-extension'
                data = png()
                path.write_bytes(data)
                result = call(client, 'probe_image', path=str(path))
                self.assertFalse(result.get('isError', False), result)
                self.assertEqual(result['structuredContent'], {'type': 'image_probed', 'probe': {
                    'format': 'png', 'width': 2, 'height': 3, 'orientation': 1, 'has_alpha': True,
                    'byte_length': len(data), 'sha256': hashlib.sha256(data).hexdigest()}})
                self.assertEqual(call(client, 'list_sessions')['structuredContent']['session_ids'], [])
                before = call(client, 'create_project', title='画像検査の不変確認')['structuredContent']['snapshot']
                call(client, 'probe_image', path=str(path))
                after = call(client, 'get_project')['structuredContent']['snapshot']
                self.assertEqual(before, after)
                for value, code in [(str(root / 'missing.png'), 'ASSET_MISSING'),
                                    ('relative.png', 'VALIDATION_FAILED'),
                                    (str(root), 'VALIDATION_FAILED')]:
                    error = call(client, 'probe_image', path=value)
                    self.assertTrue(error['isError'])
                    self.assertEqual(error['structuredContent']['code'], code)
                    self.assertEqual(error['structuredContent']['details']['path'], value)
                path.write_bytes(b'not a supported image')
                error = call(client, 'probe_image', path=str(path))
                self.assertEqual(error['structuredContent']['code'], 'CAPABILITY_UNAVAILABLE')
                self.assertEqual(call(client, 'get_project')['structuredContent']['snapshot'], before)
            finally:
                if client:
                    client.close()
                host.close()
