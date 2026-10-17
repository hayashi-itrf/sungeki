import hashlib
import struct
import tempfile
import unittest
import uuid
import wave
from pathlib import Path
from support import HostProcess, McpClient
from test_mcp_editing import call

class AudioAnalysisTest(unittest.TestCase):
    def test_verified_waveform_windows_cache_rejections_and_no_edit(self):
        with tempfile.TemporaryDirectory(prefix='vvm-analysis-',dir='/tmp') as temp:
            root=Path(temp); path=root/'stereo.wav'
            frames=[(16384,-8192),(8192,-16384),(24576,-24576),(0,0),(4096,-4096)]
            with wave.open(str(path),'wb') as w:
                w.setparams((2,2,24000,0,'NONE','not compressed'))
                w.writeframes(b''.join(struct.pack('<hh',*f) for f in frames))
            original=path.read_bytes(); fingerprint=hashlib.sha256(original).hexdigest()
            host=HostProcess(root); client=None
            try:
                self.assertGreaterEqual(host.ready()['protocol_minor'],44);client=McpClient(root)
                def ok(name,**args):
                    result=call(client,name,**args)
                    self.assertFalse(result.get('isError'),result)
                    return result['structuredContent']
                initial=ok('create_project',title='波形範囲と原本変更検証')['snapshot']
                asset=dict(id=str(uuid.uuid4()),locator=dict(type='absolute',path=str(path)),name='stereo fixture',source=None,credit='',content_hash=fingerprint,
                    probe=dict(type='audio',info=dict(sha256=fingerprint,duration_us=209,sample_rate=24000,channels=2,frames=5,byte_length=len(original))))
                snapshot=ok('apply_commands',operation_id=str(uuid.uuid4()),commands=[dict(type='register_asset',asset=asset)])['snapshot']
                args=dict(session_id=initial['session_id'],expected_revision=snapshot['revision'],asset_id=asset['id'],base_frames=2,level=0,channel=0,start_bucket=0,count=4096)
                first=ok('analyze_audio',**args)['analysis']
                self.assertFalse(first['cache_hit']);self.assertEqual(first['signal'],'source_pre_gain')
                self.assertEqual(first['peaks'],[[.25,.5],[0,.75],[.125,.125]])
                self.assertEqual((first['start_frame'],first['end_frame']),(0,5))
                self.assertFalse(first['lufs_measured']);self.assertFalse(first['true_peak_measured'])
                right=ok('analyze_audio',**dict(args,channel=1,start_bucket=1,count=1))['analysis']
                self.assertTrue(right['cache_hit']);self.assertEqual(right['peaks'],[[-.75,0]])
                self.assertEqual((right['start_frame'],right['end_frame']),(2,4))
                for changes,code in [({'count':4097},'VALIDATION_FAILED'),({'level':19},'VALIDATION_FAILED'),({'start_bucket':4},'VALIDATION_FAILED'),({'asset_id':'missing'},'NOT_FOUND'),({'expected_revision':0},'CONFLICT')]:
                    rejected=call(client,'analyze_audio',**dict(args,**changes))
                    self.assertTrue(rejected['isError']);self.assertEqual(rejected['structuredContent']['code'],code)
                self.assertEqual(ok('get_project')['snapshot'],snapshot)
                path.write_bytes(original[:-1]+bytes([original[-1]^1]))
                self.assertEqual(call(client,'analyze_audio',**args)['structuredContent']['code'],'EXTERNAL_MODIFICATION')
                path.unlink()
                self.assertEqual(call(client,'analyze_audio',**args)['structuredContent']['code'],'ASSET_MISSING')
                path.write_bytes(original)
                self.assertTrue(ok('analyze_audio',**args)['analysis']['cache_hit'])
                self.assertEqual(ok('get_project')['snapshot'],snapshot)
                self.assertEqual(path.read_bytes(),original)
            finally:
                if client:client.close()
                host.close()
