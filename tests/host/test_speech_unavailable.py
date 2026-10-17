import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
from support import HostProcess, McpClient
from test_mcp_editing import call

class SpeechUnavailableTest(unittest.TestCase):
    def test_missing_audio_storage_does_not_prevent_host_start_or_editing(self):
        with tempfile.TemporaryDirectory(prefix='vvm-no-speech-store-', dir='/tmp') as directory:
            root = Path(directory)
            with patch.dict(os.environ, SUNGEKI_RUNTIME_ROOT=str(root / 'missing-audio-storage')):
                host = HostProcess(root)
                client = None
                try:
                    host.ready()
                    client = McpClient(root)
                    result = call(client, 'create_project', title='音声保存先がない場合の編集検証')
                    self.assertFalse(result.get('isError'), result)
                    session = result['structuredContent']['snapshot']['session_id']
                    listed = call(client, 'list_speech_jobs', session_id=session, limit=12)
                    self.assertFalse(listed.get('isError'), listed)
                    self.assertEqual(listed['structuredContent']['job_ids'], [])
                    self.assertFalse((root / 'missing-audio-storage').exists())
                finally:
                    if client: client.close()
                    host.close()
