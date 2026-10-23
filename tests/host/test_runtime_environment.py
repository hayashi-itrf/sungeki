import json
import os
import sqlite3
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from support import HostProcess, McpClient
from test_mcp_editing import call


KEYS = ('SUNGEKI_RUNTIME_ROOT', 'SUNGEKI_RENDER_FFMPEG', 'SUNGEKI_RENDER_FFPROBE',
        'SUNGEKI_RENDER_FONT', 'SUNGEKI_RENDER_FONT_SHA256')


class RuntimeEnvironmentTest(unittest.TestCase):
    def test_connected_host_selection_is_read_only_and_not_client_environment(self):
        # TMPDIR can point to a short external-SSD test directory.
        with tempfile.TemporaryDirectory(prefix='sg-env-') as directory:
            root = Path(directory)
            data = root / 'host 設定'
            data.mkdir(mode=0o700)
            settings_path = data / 'runtime.json'
            saved = dict(format_version=1, runtime_root='/Volumes/Unavailable/作業',
                         ffmpeg='/missing/saved-ffmpeg', ffprobe='/missing/saved-ffprobe',
                         font='/missing/字幕.ttf', font_sha256='a' * 64,
                         voicevox_engine='/missing/must-not-start-voicevox',
                         vrm_node=None, vrm_render_worker=None, vrm_inspect_worker=None)
            original = (json.dumps(saved, ensure_ascii=False) + '\n').encode()
            settings_path.write_bytes(original)
            host_environment = {key: value for key, value in os.environ.items() if key not in KEYS}
            host_environment.update(SUNGEKI_RENDER_FFMPEG='/missing/host-ffmpeg',
                                    SUNGEKI_RENDER_FFPROBE='', SUNGEKI_RENDER_FONT='/missing/host-font')
            host, client = None, None
            try:
                with patch.dict(os.environ, host_environment, clear=True):
                    host = HostProcess(root, data_dir=data)
                host_info = host.ready()
                with patch.dict(os.environ, {'SUNGEKI_RENDER_FFMPEG': '/client-must-not-win'}):
                    client = McpClient(root, data_dir=root / 'wrong-client-settings')

                def ok(name, **args):
                    result = call(client, name, **args)
                    self.assertFalse(result.get('isError'), result)
                    return result['structuredContent']

                before_sessions = ok('list_sessions')
                def jobs():
                    with sqlite3.connect((data / 'recovery.sqlite').as_uri() + '?mode=ro', uri=True) as db:
                        return db.execute('SELECT id,version,state FROM jobs ORDER BY id').fetchall()
                before_jobs = jobs()
                before_files = sorted(str(p.relative_to(data)) for p in data.rglob('*'))
                before_settings = ok('get_runtime_settings')
                report = ok('get_runtime_environment')['environment']
                self.assertEqual(report['data_directory'], str(data))
                self.assertEqual(report['runtime_settings_path'], str(settings_path))
                self.assertEqual(report['scope'], 'render_paths_and_runtime_root')
                self.assertEqual(report['verification'], 'not_run')
                self.assertEqual(set(report['saved']), {'runtime_root', 'ffmpeg', 'ffprobe', 'font', 'font_sha256'})
                self.assertEqual(report['saved']['ffmpeg'], '/missing/saved-ffmpeg')
                self.assertEqual(report['selected']['ffmpeg'], dict(
                    value='/missing/host-ffmpeg', source='environment', environment_variable='SUNGEKI_RENDER_FFMPEG'))
                self.assertEqual(report['selected']['ffprobe']['value'], '')
                self.assertEqual(report['selected']['ffprobe']['source'], 'environment')
                self.assertEqual(report['selected']['font']['value'], '/missing/host-font')
                self.assertEqual(report['selected']['font_sha256']['value'], 'a' * 64)
                self.assertEqual(report['selected']['font_sha256']['source'], 'saved')
                self.assertEqual(report['selected']['runtime_root']['source'], 'saved')
                self.assertEqual(ok('get_runtime_settings'), before_settings)
                self.assertEqual(ok('list_sessions'), before_sessions)
                self.assertEqual(jobs(), before_jobs)
                self.assertEqual(settings_path.read_bytes(), original)
                self.assertEqual(sorted(str(p.relative_to(data)) for p in data.rglob('*')), before_files)
                self.assertFalse((root / 'wrong-client-settings').exists())
                self.assertEqual(client.capabilities()['structuredContent']['host_instance_id'], host_info['host_instance_id'])
                unknown = client.call('tools/call', {'name': 'get_runtime_environment', 'arguments': {'path': '/other'}})
                self.assertIn('error', unknown)
                tools = client.call('tools/list')['result']['tools']
                tool = next(t for t in tools if t['name'] == 'get_runtime_environment')
                self.assertFalse(tool['inputSchema']['additionalProperties'])
            finally:
                if client:
                    client.close()
                if host:
                    host.close()

    def test_missing_settings_are_reported_without_creating_runtime_file(self):
        with tempfile.TemporaryDirectory(prefix='sg-env-') as directory:
            root = Path(directory)
            env = {key: value for key, value in os.environ.items() if key not in KEYS}
            host, client = None, None
            try:
                with patch.dict(os.environ, env, clear=True):
                    host = HostProcess(root)
                host.ready()
                client = McpClient(root)
                result = call(client, 'get_runtime_environment')
                self.assertFalse(result.get('isError'), result)
                report = result['structuredContent']['environment']
                for selection in report['selected'].values():
                    self.assertEqual(selection['source'], 'unset')
                    self.assertIsNone(selection['value'])
                self.assertFalse((root / 'data/runtime.json').exists())
            finally:
                if client:
                    client.close()
                if host:
                    host.close()
