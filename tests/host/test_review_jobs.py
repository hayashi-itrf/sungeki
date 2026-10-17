import json
import sqlite3
import tempfile
import unittest
from pathlib import Path
from support import HostProcess, connect, exchange, hello
from test_sessions import call, edit_request, uid


class ReviewJobTest(unittest.TestCase):
    def test_snapshot_retry_stale_and_durable_report(self):
        with tempfile.TemporaryDirectory(prefix='vvm-review-', dir='/tmp') as directory:
            root = Path(directory)
            host = HostProcess(root)
            try:
                self.assertGreaterEqual(host.ready()['protocol_minor'], 73)
                with connect(root) as connection:
                    exchange(connection, hello(minor=73))
                    before = call(connection, {'type': 'create', 'title': '確認レポートの固定入力検証'})['snapshot']
                    sid, jid = before['session_id'], uid()
                    start = {'type': 'start_review', 'session_id': sid, 'job_id': jid, 'expected_revision': 0}
                    first = call(connection, start)['job']
                    self.assertEqual(first['phase'], 'queued')
                    self.assertFalse(first['stale'])
                    self.assertEqual(call(connection, {'type': 'get', 'session_id': sid})['snapshot'], before)
                    completed = call(connection, {'type': 'get_review_job', 'job_id': jid, 'wait_ms': 2000})['job']
                    self.assertEqual(completed['phase'], 'succeeded')
                    self.assertEqual(completed['report']['source_revision'], 0)
                    self.assertEqual(completed['report']['content_hash'], completed['content_hash'])
                    self.assertEqual(len(completed['report']['asset_fingerprint']), 64)
                    summary = completed['report']['diagnostics']['summary']
                    self.assertEqual(summary, {'passed': 2, 'failed': 0, 'not_run': 4, 'issue_count': 0, 'complete': False})
                    self.assertEqual(call(connection, start)['job'], completed)
                    self.assertEqual(call(connection, {'type': 'cancel_review_job', 'job_id': jid})['job'], completed)
                    call(connection, edit_request(sid, 0, [{'type': 'rename_project', 'title': '診断開始後の編集'}]))
                    stale = call(connection, start)['job']
                    self.assertTrue(stale['stale'])
                    self.assertEqual(stale['report'], completed['report'])
                    self.assertEqual(call(connection, {**start, 'expected_revision': 1})['error']['code'], 'CONFLICT')
                    self.assertEqual(call(connection, {**start, 'job_id': uid()})['error']['code'], 'CONFLICT')
                    self.assertEqual(call(connection, {'type': 'get_review_job', 'job_id': jid, 'wait_ms': 30001})['error']['code'], 'VALIDATION_FAILED')
                    ids = call(connection, {'type': 'list_review_jobs', 'session_id': sid, 'after': None, 'limit': 10})['job_ids']
                    self.assertEqual(ids, [jid])
                    self.assertEqual(call(connection, {'type': 'get_review_job', 'job_id': uid(), 'wait_ms': 0})['error']['code'], 'NOT_FOUND')
                host.close(); host = HostProcess(root); host.ready()
                with connect(root) as connection:
                    exchange(connection, hello(minor=73))
                    self.assertEqual(call(connection, {'type': 'get_review_job', 'job_id': jid, 'wait_ms': 0})['job'], stale)
                with sqlite3.connect(root / 'data/recovery.sqlite') as db:
                    source = json.loads(db.execute('SELECT input FROM jobs WHERE id=?', (jid,)).fetchone()[0])
                    self.assertEqual(source['project'], before['project'])
            finally:
                host.close()

    def test_restart_marks_persisted_unfinished_work_interrupted(self):
        with tempfile.TemporaryDirectory(prefix='vvm-review-recover-', dir='/tmp') as directory:
            root = Path(directory)
            host = HostProcess(root)
            try:
                host.ready()
                with connect(root) as connection:
                    exchange(connection, hello(minor=73))
                    sid = call(connection, {'type': 'create', 'title': 'ジョブ復旧状態の検証'})['snapshot']['session_id']
                    jid = uid()
                    call(connection, {'type': 'start_review', 'session_id': sid, 'job_id': jid, 'expected_revision': 0})
                    call(connection, {'type': 'get_review_job', 'job_id': jid, 'wait_ms': 2000})
                host.close()
                # Deterministic persisted crash-state fixture, not timing-dependent killing of a short review.
                with sqlite3.connect(root / 'data/recovery.sqlite') as db:
                    state = {'phase': 'running', 'progress': {'stage': '構造診断', 'fraction': 0.5}, 'report': None, 'error': None}
                    db.execute('UPDATE jobs SET state=? WHERE id=?', (json.dumps(state).encode(), jid))
                orphan = root / 'data/recovery.sqlite.review-work' / jid
                orphan.mkdir()
                (orphan / 'partial.original').write_bytes(b'crash fixture')
                unrelated = orphan.parent / 'keep-user-file'
                unrelated.write_bytes(b'untouched')
                host = HostProcess(root); host.ready()
                self.assertFalse(orphan.exists())
                self.assertEqual(unrelated.read_bytes(), b'untouched')
                with connect(root) as connection:
                    exchange(connection, hello(minor=73))
                    job = call(connection, {'type': 'get_review_job', 'job_id': jid, 'wait_ms': 2000})['job']
                    self.assertEqual(job['phase'], 'interrupted')
                    self.assertEqual(job['progress'], state['progress'])
                    self.assertIsNone(job['report'])
                    self.assertEqual(call(connection, {'type': 'start_review', 'session_id': sid, 'job_id': jid, 'expected_revision': 0})['job'], job)
            finally:
                host.close()
