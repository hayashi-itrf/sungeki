import hashlib
import json
import shutil
import tempfile
import time
import unittest
import uuid
from pathlib import Path
from support import HostProcess, McpClient
from test_mcp_editing import call
from test_image_probe import png

class CollectionHistory(unittest.TestCase):
    def test_history_only_asset_preflight_export_missing_and_restart(self):
        with tempfile.TemporaryDirectory(prefix='vvm-history-', dir='/tmp') as tmp:
            root = Path(tmp).resolve()
            image = root / 'original.png'
            image.write_bytes(png())
            host, client = HostProcess(root), None
            def invoke(tool_name, **args):
                response = call(client, tool_name, **args)
                self.assertFalse(response.get('isError'), response)
                return response['structuredContent']
            def wait(job_id):
                deadline = time.monotonic() + 10
                while time.monotonic() < deadline:
                    job = invoke('get_collection_job', job_id=job_id)['job']
                    if job['phase'] in ('succeeded', 'failed', 'cancelled', 'interrupted'):
                        return job
                    time.sleep(.02)
                self.fail('collection timeout')
            try:
                self.assertGreaterEqual(host.ready()['protocol_minor'], 80)
                client = McpClient(root)
                invoke('create_project', title='保存時点だけの素材を持ち出す試験')
                probe = invoke('probe_image', path=str(image))['probe']
                aid = str(uuid.uuid4())
                asset = dict(id=aid, name='過去の図', source=None, credit='自作図',
                             locator=dict(type='absolute', path=str(image)), content_hash=probe['sha256'],
                             probe=dict(type='image', info=probe))
                snap = invoke('apply_commands', operation_id=str(uuid.uuid4()),
                              commands=[dict(type='register_asset', asset=asset)])['snapshot']
                cid = str(uuid.uuid4())
                invoke('create_checkpoint', checkpoint_id=cid, name='図があった時点', expected_revision=snap['revision'])
                snap = invoke('apply_commands', operation_id=str(uuid.uuid4()),
                              commands=[dict(type='remove_asset', asset_id=aid)])['snapshot']
                common = dict(session_id=snap['session_id'], expected_revision=snap['revision'],
                              destination=str(root / 'package'), include_checkpoints=True)
                dry_id = str(uuid.uuid4())
                invoke('collect_project', **common, job_id=dry_id, dry_run=True)
                dry = wait(dry_id)
                self.assertEqual(dry['phase'], 'succeeded', dry)
                self.assertEqual(dry['checkpoint_count'], 1)
                self.assertTrue(dry['include_checkpoints'])
                self.assertTrue(dry['plan']['sources_ready'])
                self.assertEqual(dry['plan']['source_count'], 1)
                self.assertNotIn('checkpoints', dry['plan']['excluded_dependencies'])
                page = invoke('read_collection_plan', job_id=dry_id, offset=0, limit=10)['page']
                self.assertEqual(page['items'][0]['checkpoint_id'], cid)
                self.assertEqual(page['items'][0]['asset_id'], aid)
                self.assertFalse((root / 'package').exists())
                self.assertEqual(invoke('get_project')['snapshot'], snap)
                # Pre-restoration diagnostics must inspect history-only originals,
                # without restoring the asset or producing a collection directory.
                image.write_bytes(png() + b'grown after checkpoint')
                changed_id = str(uuid.uuid4())
                invoke('collect_project', **common, job_id=changed_id, dry_run=True)
                changed = wait(changed_id)
                self.assertEqual(changed['phase'], 'succeeded')
                self.assertFalse(changed['plan']['sources_ready'])
                changed_page = invoke('read_collection_plan', job_id=changed_id, offset=0, limit=10)['page']
                self.assertEqual(changed_page['items'][0]['status'], 'changed')
                self.assertEqual(changed_page['items'][0]['checkpoint_id'], cid)
                self.assertEqual(invoke('get_project')['snapshot'], snap)
                self.assertFalse((root / 'package').exists())
                image.write_bytes(png())
                actual_id = str(uuid.uuid4())
                invoke('collect_project', **common, job_id=actual_id)
                actual = wait(actual_id)
                self.assertEqual(actual['phase'], 'succeeded', actual)
                self.assertTrue(actual['output']['checkpoints_included'])
                manifest = json.loads((root / 'package/manifest.json').read_text())
                entry = manifest['checkpoints'][0]
                self.assertEqual(entry['origin']['checkpoint_id'], cid)
                data = (root / 'package' / entry['path']).read_bytes()
                self.assertEqual(hashlib.sha256(data).hexdigest(), entry['sha256'])
                portable = json.loads(data)
                material = portable['project']['assets'][0]
                self.assertEqual(hashlib.sha256((root / 'package' / material['locator']['path']).read_bytes()).hexdigest(), probe['sha256'])
                image.unlink()
                missing_id = str(uuid.uuid4())
                invoke('collect_project', **dict(common, destination=str(root / 'missing')), job_id=missing_id, dry_run=True)
                missing = wait(missing_id)
                self.assertFalse(missing['plan']['sources_ready'])
                missing_page = invoke('read_collection_plan', job_id=missing_id, offset=0, limit=10)['page']
                self.assertEqual(missing_page['items'][0]['status'], 'missing')
                self.assertEqual(missing_page['items'][0]['checkpoint_id'], cid)
                self.assertEqual(invoke('get_project')['snapshot'], snap)
                self.assertFalse((root / 'missing').exists())
                invoke('create_checkpoint', checkpoint_id=str(uuid.uuid4()), name='確認後に追加', expected_revision=snap['revision'])
                self.assertTrue(invoke('get_collection_job', job_id=dry_id)['job']['stale'])
                client.close(); client = None
                host.close()
                host = HostProcess(root); host.ready(); client = McpClient(root)
                self.assertEqual(invoke('read_collection_plan', job_id=dry_id, offset=0, limit=10)['page'], page)
                self.assertEqual(invoke('get_collection_job', job_id=actual_id)['job']['output'], actual['output'])
                # Default open does not import or share the source checkpoint list.
                opened = invoke('open_project', path=str(root / 'package/project.vvm.json'))['snapshot']
                self.assertEqual(invoke('list_checkpoints')['checkpoints'], [])
                opened = invoke('apply_commands', operation_id=str(uuid.uuid4()), commands=[dict(type='rename_project', title='未保存の編集内容を保持')])['snapshot']
                imported = invoke('open_project', path=str(root / 'package/project.vvm.json'), import_checkpoints=True)['snapshot']
                self.assertEqual(imported, opened)
                checkpoints = invoke('list_checkpoints')['checkpoints']
                self.assertEqual(len(checkpoints), 1)
                self.assertEqual(checkpoints[0]['id'], entry['id'])
                invoke('open_project', path=str(root / 'package/project.vvm.json'), import_checkpoints=True)
                self.assertEqual(invoke('list_checkpoints')['checkpoints'], checkpoints)
                # A valid but conflicting history in a new document rolls back the new session too.
                other = root / 'conflicting'
                shutil.copytree(root / 'package', other)
                conflicting_path = other / entry['path']
                conflicting = json.loads(conflicting_path.read_text())
                conflicting['name'] = '同じIDの別の保存時点'
                changed_bytes = json.dumps(conflicting, ensure_ascii=False).encode()
                conflicting_path.write_bytes(changed_bytes)
                changed_manifest = json.loads((other / 'manifest.json').read_text())
                changed_manifest['checkpoints'][0]['sha256'] = hashlib.sha256(changed_bytes).hexdigest()
                changed_manifest['checkpoints'][0]['byte_length'] = len(changed_bytes)
                (other / 'manifest.json').write_text(json.dumps(changed_manifest, ensure_ascii=False))
                sessions_before = invoke('list_sessions')
                rejected = call(client, 'open_project', path=str(other / 'project.vvm.json'), import_checkpoints=True)
                self.assertTrue(rejected.get('isError'), rejected)
                self.assertEqual(rejected['structuredContent']['code'], 'CONFLICT')
                self.assertEqual(invoke('list_sessions'), sessions_before)
                restored = invoke('restore_checkpoint', checkpoint_id=entry['id'], operation_id=str(uuid.uuid4()), expected_revision=opened['revision'])['snapshot']
                self.assertEqual(len(restored['project']['assets']), 1)
                self.assertTrue(Path(restored['project']['assets'][0]['locator']['path']).is_relative_to(root / 'package'))
                fresh_id = str(uuid.uuid4())
                invoke('collect_project', session_id=restored['session_id'], expected_revision=restored['revision'],
                       job_id=fresh_id, destination=str(root / 'fresh'), include_checkpoints=True)
                self.assertEqual(wait(fresh_id)['phase'], 'succeeded')
                fresh = invoke('open_project', path=str(root / 'fresh/project.vvm.json'), import_checkpoints=True)['snapshot']
                self.assertNotEqual(fresh['project']['id'], restored['project']['id'])
                self.assertEqual(len(invoke('list_checkpoints')['checkpoints']), 1)


            finally:
                if client:
                    client.close()
                host.close()
