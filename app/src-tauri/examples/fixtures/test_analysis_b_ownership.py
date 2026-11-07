#!/usr/bin/env python3
"""Linux integration check: run after `cargo build --example analysis_b`."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[2]
CLI = ROOT / "target/debug/examples/analysis_b"
ENGINE = ROOT / "binaries/stockfish-x86_64-unknown-linux-gnu"
FIXTURES = ROOT / "examples/fixtures"


@unittest.skipUnless(os.path.exists('/proc/self/task'), 'requires Linux procfs')
class Ownership(unittest.TestCase):
    def test_cancel_and_timeout_during_search_reap_engine(self):
        for cancelled in (True, False):
            with self.subTest(cancelled=cancelled), tempfile.TemporaryDirectory() as tmp:
                manifest = json.loads((FIXTURES / 'analysis-b-manifest.json').read_text())
                manifest.update(screening_depth=40, verification_depth=41,
                                timeout_ms=10000 if cancelled else 2000,
                                cancel_after_ms=2000 if cancelled else None)
                config, report = Path(tmp) / 'config.json', Path(tmp) / 'report.json'
                config.write_text(json.dumps(manifest))
                proc = subprocess.Popen([str(CLI), str(FIXTURES / 'analysis-b-mate.pgn'), str(ENGINE), str(config), str(report)], stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
                children = Path(f'/proc/{proc.pid}/task/{proc.pid}/children')
                engine_pid = None
                deadline = time.monotonic() + 5
                while proc.poll() is None and time.monotonic() < deadline:
                    try:
                        found = children.read_text().split()
                    except FileNotFoundError:
                        break
                    if found:
                        engine_pid = int(found[0])
                        break
                    time.sleep(0.01)
                _, stderr = proc.communicate(timeout=20)
                self.assertIsNotNone(engine_pid, stderr.decode())
                self.assertNotEqual(proc.returncode, 0)
                result = json.loads(report.read_text())
                self.assertEqual(result['status'], 'cancelled' if cancelled else 'failed')
                self.assertEqual(result['error']['code'], 'cancelled' if cancelled else 'engineTimeout')
                self.assertTrue(result['snapshot']['ledger'], 'search never started')
                self.assertFalse(Path(f'/proc/{engine_pid}').exists(), 'engine survived CLI return')


if __name__ == '__main__':
    unittest.main()
