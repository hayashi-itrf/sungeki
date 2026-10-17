"""Read-only original inspection through the actual new MCP."""
import hashlib
import tempfile
import unittest
import wave
import uuid
from pathlib import Path
from support import HostProcess, McpClient
from test_mcp_editing import call


class AudioProbeTest(unittest.TestCase):
    def test_probe_metadata_register_and_reject_invalid_originals(self):
        with tempfile.TemporaryDirectory(prefix='vvm-audio-probe-', dir='/tmp') as tmp:
            root=Path(tmp); path=root/'original.wav'
            with wave.open(str(path),'wb') as writer:
                writer.setparams((2,2,44100,0,'NONE','not compressed'))
                writer.writeframes(b'\x00\x20\x00\xe0'*101)
            data=path.read_bytes(); host=HostProcess(root); client=None
            try:
                self.assertGreaterEqual(host.ready()['protocol_minor'],44)
                client=McpClient(root)
                def ok(name,**args):
                    response=call(client,name,**args)
                    self.assertFalse(response.get('isError'),response)
                    return response['structuredContent']
                snapshot=ok('create_project',title='音声原本検査')['snapshot']
                probe=ok('probe_audio',path=str(path))['probe']
                self.assertEqual(probe,dict(sha256=hashlib.sha256(data).hexdigest(),sample_rate=44100,channels=2,frames=101,duration_us=2291,byte_length=len(data)))
                self.assertEqual(ok('get_project')['snapshot'],snapshot)
                self.assertEqual(path.read_bytes(),data)
                invalid=root/'broken.wav';invalid.write_bytes(b'not a WAV')
                link=root/'link.wav';link.symlink_to(path)
                for source in ['relative.wav',str(invalid),str(link),str(root),str(root/'missing.wav')]:
                    response=call(client,'probe_audio',path=source)
                    self.assertTrue(response['isError'],response)
                self.assertEqual(ok('get_project')['snapshot'],snapshot)
                asset_id,track_id,clip_id=[str(uuid.uuid4()) for _ in range(3)]
                registered=ok('apply_commands',operation_id=str(uuid.uuid4()),commands=[
                    dict(type='register_asset',asset=dict(id=asset_id,name='検査済み原本',source=None,credit='',content_hash=probe['sha256'],locator=dict(type='absolute',path=str(path)),probe=dict(type='audio',info=probe))),
                    dict(type='add_track',track=dict(id=track_id,name='BGM',kind='audio',visible=True,muted=False)),
                    dict(type='add_clip',clip=dict(id=clip_id,track_id=track_id,scene_id=None,start_us=0,duration_us=probe['duration_us'],transform=dict(x=0,y=0,width=1,height=1,opacity=1),content=dict(type='audio',asset_id=asset_id,playback=dict(source_start_us=0,source_duration_us=probe['duration_us'],looped=False,gain_db=0,fade_in_us=0,fade_out_us=0))))
                ])['snapshot']
                self.assertEqual(registered['project']['assets'][0]['probe']['info'],probe)
                self.assertEqual(registered['project']['clips'][0]['duration_us'],probe['duration_us'])
                self.assertEqual(path.read_bytes(),data)
            finally:
                if client:client.close()
                host.close()
