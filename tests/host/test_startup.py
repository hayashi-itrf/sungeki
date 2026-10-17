"""Real host readiness while recovery storage is temporarily busy."""
import concurrent.futures
from pathlib import Path
import sqlite3
import tempfile
import time
import unittest

from support import HostProcess, McpClient


class StartupTests(unittest.TestCase):
    def test_recovery_finishes_before_accepting_clients(self):
        with tempfile.TemporaryDirectory(prefix='vvm-ready-', dir='/tmp') as temporary:
            directory = Path(temporary)
            first = HostProcess(directory)
            try:
                first.ready()
            finally:
                first.close()
            database = sqlite3.connect(directory / 'data/recovery.sqlite')
            database.execute('BEGIN IMMEDIATE')
            client = McpClient(directory)
            try:
                with concurrent.futures.ThreadPoolExecutor(max_workers=1) as executor:
                    pending = executor.submit(client.capabilities)
                    # Longer than an ordinary 2s request, within the 5s startup budget.
                    time.sleep(3)
                    database.rollback()
                    result = pending.result(timeout=6)
                self.assertFalse(result['isError'], result)
            finally:
                database.close()
                client.close()
                # Auto-started host exits by its normal idle lifetime.
                deadline = time.monotonic() + 35
                while (directory / 'host.sock').exists() and time.monotonic() < deadline:
                    time.sleep(.1)
                self.assertFalse((directory / 'host.sock').exists())

    def test_startup_timeout_reports_only_known_diagnostic_stage(self):
        # Diagnostic transport fixture, not proof of native OS startup behavior.
        with tempfile.TemporaryDirectory(prefix='vvm-stage-', dir='/tmp') as temporary:
            root = Path(temporary)
            binary = root / 'diagnostic-host'
            binary.write_text('#!/bin/sh\nprintf "VVM_STARTUP:recovery\\n"\nsleep 6\n')
            binary.chmod(0o700)
            client = McpClient(root, binary=binary)
            try:
                result = client.capabilities()
                self.assertTrue(result['isError'], result)
                self.assertIn('保存済みの編集状態を復元', str(result))
            finally:
                client.close()
