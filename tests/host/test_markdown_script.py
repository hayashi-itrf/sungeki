import tempfile
import unittest
from pathlib import Path
from support import HostProcess, McpClient
from test_mcp_editing import call


class MarkdownScriptTest(unittest.TestCase):
    def test_shared_parser_preserves_session_and_returns_source_line(self):
        with tempfile.TemporaryDirectory(prefix='vvm-md-', dir='/tmp') as directory:
            host, client = HostProcess(Path(directory)), None
            try:
                host.ready()
                client = McpClient(Path(directory))
                result = call(client, 'parse_markdown_script', source='## 導入\n- left: Codex\n  > コーデックス', roles=['left'])
                self.assertFalse(result.get('isError'), result)
                self.assertEqual(result['structuredContent']['scenes'][0]['lines'][0]['spoken_text'], 'コーデックス')
                before = call(client, 'create_project', title='Markdown解析の作品不変確認')['structuredContent']['snapshot']
                result = call(client, 'parse_markdown_script', source='## 導入\n\n- unknown: 本文', roles=['left'])
                self.assertTrue(result.get('isError'), result)
                self.assertEqual(result['structuredContent']['code'], 'VALIDATION_FAILED')
                self.assertEqual(result['structuredContent']['details']['line'], '3')
                result = call(client, 'parse_markdown_script', source='## 導入\n- left: **本文**: 続き\n## 結び\n- right: おしまい', roles=['left', 'right'])
                self.assertFalse(result.get('isError'), result)
                self.assertEqual([s['name'] for s in result['structuredContent']['scenes']], ['導入', '結び'])
                self.assertEqual(result['structuredContent']['scenes'][0]['lines'][0]['subtitle_text'], '**本文**: 続き')
                self.assertEqual(call(client, 'get_project')['structuredContent']['snapshot'], before)
            finally:
                try:
                    if client:
                        client.close()
                finally:
                    host.close()
