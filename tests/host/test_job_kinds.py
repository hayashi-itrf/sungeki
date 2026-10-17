import json
import sqlite3
import tempfile
import unittest
import uuid
from pathlib import Path
from support import HostProcess, connect, exchange, hello


def call(connection, request):
    result = exchange(connection, dict(type='session', request=request))
    return result.get('result', result)


class JobKindTest(unittest.TestCase):
    def test_invalid_speech_record_fails_closed_without_review_cleanup(self):
        with tempfile.TemporaryDirectory(prefix='vvm-job-kind-', dir='/tmp') as directory:
            root = Path(directory)
            host = HostProcess(root)
            try:
                host.ready()
                with connect(root) as connection:
                    exchange(connection, hello())
                    sid = call(connection, dict(type='create', title='ジョブ種類分離の検証'))['snapshot']['session_id']
                host.close()
                jid = str(uuid.uuid4())
                body = b'not a review payload'
                with sqlite3.connect(root / 'data/recovery.sqlite') as db:
                    db.execute('INSERT INTO jobs(id,session_id,input,version,state,kind) VALUES(?,?,?,0,?,?)',
                               (jid, sid, body, body, 'speech'))
                # Review cleanup must not delete another kind's similarly named output.
                foreign = root / 'data/recovery.sqlite.review-work' / jid
                foreign.mkdir(parents=True)
                (foreign / 'retained').write_bytes(body)
                host = HostProcess(root)
                with self.assertRaisesRegex(AssertionError, '音声ジョブ記録'):
                    host.ready()
                with sqlite3.connect(root / 'data/recovery.sqlite') as db:
                    self.assertEqual(db.execute('SELECT input,version,state,kind FROM jobs WHERE id=?', (jid,)).fetchone(),
                                     (body, 0, body, 'speech'))
                self.assertEqual((foreign / 'retained').read_bytes(), body)
            finally:
                host.close()
