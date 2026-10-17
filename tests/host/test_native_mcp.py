"""Regression for native harness bootstrapping the wrong data directory."""
import sys
import tempfile
import unittest
from pathlib import Path
from support import HostProcess
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'manual'))
from native_mcp import NativeMcpClient


class NativeAttachOnly(unittest.TestCase):
    def test_missing_host_does_not_create_socket_or_data(self):
        with tempfile.TemporaryDirectory(prefix='vvm-native-absent-', dir='/tmp') as tmp:
            root = Path(tmp)
            with self.assertRaises(FileNotFoundError):
                NativeMcpClient(root)
            self.assertEqual(list(root.iterdir()), [])

    def test_existing_host_identity_and_no_replacement_after_exit(self):
        with tempfile.TemporaryDirectory(prefix='vvm-native-attach-', dir='/tmp') as tmp:
            root = Path(tmp)
            host, client = HostProcess(root, root / 'native-data'), None
            try:
                observed = host.ready()
                client = NativeMcpClient(root)
                self.assertEqual(client.observed_host['host_instance_id'], observed['host_instance_id'])
                host.close()
                errors = []
                for _ in range(2):
                    result = client.capabilities()
                    self.assertTrue(result.get('isError'), result)
                    errors.append(result['structuredContent']['code'])
                self.assertIn('SIDECAR_MISSING', errors)
                self.assertFalse((root / 'data').exists())
                self.assertFalse((root / '.native-attach-only-no-host').exists())
            finally:
                if client:
                    client.close()
                host.close()
