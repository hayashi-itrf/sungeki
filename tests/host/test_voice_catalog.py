import json
import tempfile
import threading
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from support import HostProcess, McpClient
from test_mcp_editing import call


class VoiceCatalogTest(unittest.TestCase):
    def test_mcp_reads_engine_and_catalog_without_mutating_project(self):
        requests = []
        engine_id = '074fc39e-678b-4c13-8916-ffca8d505d1d'

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass

            def do_GET(self):
                requests.append(self.path)
                values = {
                    '/engine_manifest': dict(uuid=engine_id, name='検証エンジン', terms_of_service='エンジン利用条件', default_sampling_rate=24000),
                    f'/speaker_info?core_version=0.16.0&speaker_uuid={engine_id}&resource_format=url': dict(policy='話者利用条件'),
                    '/version': '1.0', '/core_versions': ['0.16.0'],
                    '/speakers?core_version=0.16.0': [dict(speaker_uuid=engine_id,
                        name='検証話者', version='1', styles=[dict(id=42, name='ノーマル')])],
                }
                body = json.dumps(values[self.path]).encode()
                self.send_response(200)
                self.send_header('Content-Length', str(len(body)))
                self.end_headers()
                self.wfile.write(body)

        server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            with tempfile.TemporaryDirectory(prefix='vvm-voice-list-', dir='/tmp') as directory:
                root = Path(directory)
                host = HostProcess(root)
                client = None
                try:
                    self.assertGreaterEqual(host.ready()['protocol_minor'], 44)
                    client = McpClient(root)

                    def ok(tool, **args):
                        result = call(client, tool, **args)
                        self.assertFalse(result.get('isError'), result)
                        return result['structuredContent']

                    before = ok('create_project', title='話者一覧の読取検証')['snapshot']
                    base_url = f'http://127.0.0.1:{server.server_port}'
                    engine = ok('discover_voicevox', base_url=base_url)['engine']
                    self.assertEqual(engine['engine_id'], engine_id)
                    catalog = ok('get_voicevox_catalog', base_url=base_url,
                                 expected_engine=engine, core_version='0.16.0')
                    self.assertEqual(catalog['speakers'][0]['styles'],
                                     [dict(id=42, name='ノーマル', kind='talk')])
                    self.assertEqual(len(requests), 10)
                    details = ok('get_voicevox_speaker', base_url=base_url, expected_engine=engine, core_version='0.16.0', speaker_uuid=engine_id)['details']
                    self.assertEqual(details['speaker_policy'], '話者利用条件')
                    self.assertEqual(details['engine_terms'], 'エンジン利用条件')
                    self.assertEqual(details['default_sampling_rate'], 24000)
                    stale = dict(engine, version='old')
                    result = call(client, 'get_voicevox_catalog', base_url=base_url,
                                  expected_engine=stale, core_version='0.16.0')
                    self.assertTrue(result['isError'])
                    self.assertIn('ENGINE_UNAVAILABLE', json.dumps(result))
                    self.assertEqual(len(requests), 29)
                    result = call(client, 'discover_voicevox', base_url='https://example.com')
                    self.assertTrue(result['isError'])
                    self.assertIn('VALIDATION_FAILED', json.dumps(result))
                    self.assertEqual(ok('get_project')['snapshot'], before)
                    self.assertFalse(ok('get_capabilities')['capabilities']['speech'])
                finally:
                    if client:
                        client.close()
                    host.close()
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=2)
