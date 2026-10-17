import base64
import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest
import uuid
from support import HostProcess, connect, exchange, hello, ROOT

class VrmAssetRead(unittest.TestCase):
    def test_registered_read_rejects_changed_source_and_stale_revision(self):
        with tempfile.TemporaryDirectory(prefix='vvm-vrm-read-', dir='/tmp') as tmp:
            root = Path(tmp)
            binary = Path(os.environ.get('NEXT_VRM_TEST_HOST', str(ROOT/'target/release/sungeki-host')))
            host = HostProcess(root, binary=binary)
            try:
                host.ready()
                with connect(root) as c:
                    exchange(c, hello(minor=148))
                    def request(body): return exchange(c, dict(type='session',request=body))
                    response = request(dict(type='create', title='VRM原本範囲読込の検証'))
                    # Wire session response wrapper.
                    snapshot = response['result']['snapshot']
                    project = json.loads((ROOT/'tests/fixtures/vrm-clip-project.json').read_text())
                    asset = project['assets'][0]
                    data = bytes(range(40)); path=root/'registered.vrm';path.write_bytes(data)
                    asset['locator'] = dict(type='absolute',path=str(path))
                    probe=asset['probe']['info'];probe['byte_length']=len(data);probe['sha256']=hashlib.sha256(data).hexdigest();asset['content_hash']=probe['sha256']
                    sid=snapshot['session_id']
                    result=request(dict(type='edit',session_id=sid,edit=dict(operation_id=str(uuid.uuid4()),expected_revision=0,action=dict(type='apply',commands=[dict(type='register_asset',asset=asset)]))))
                    assert result['type']=='session', result
                    args=dict(type='read_vrm_asset',session_id=sid,expected_revision=1,asset_id=asset['id'],offset=5,length=7)
                    result=request(args)['result'];self.assertEqual(base64.b64decode(result['base64']),data[5:12])
                    self.assertEqual(request(dict(args,expected_revision=0))['error']['code'],'CONFLICT')
                    self.assertEqual(request(dict(args,length=4*1024*1024+1))['error']['code'],'VALIDATION_FAILED')
                    path.write_bytes(b'x'*40)
                    self.assertEqual(request(args)['error']['code'],'EXTERNAL_MODIFICATION')
            finally: host.close()
