import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import os
import unittest

spec = importlib.util.spec_from_file_location('continuity', Path(__file__).with_name('continuity_experiment.py'))
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)


class ContinuityTests(unittest.TestCase):
    def line(self, depth, pv, score='cp 10'):
        return c.parse_info(f'info depth {depth} multipv {pv} score {score} nodes 100 time 5 pv e2e4', 'go-1', 5)

    def test_parser_and_bounds(self):
        line = self.line(8, 2, 'mate -3 lowerbound')
        self.assertEqual(line['score'], {'type': 'mate', 'value': -3, 'bound': 'lowerbound'})
        self.assertEqual(line['go_id'], 'go-1')
        self.assertIsNone(c.parse_info('info string hello', 'go-1', 0))
        self.assertIsNone(c.parse_info('info depth x score cp 1 pv e2e4', 'go-1', 0))

    def test_coherence_requires_all_unbounded_same_depth(self):
        lines = [self.line(8, 1), self.line(8, 2), self.line(9, 1), self.line(9, 2, 'cp 3 upperbound')]
        self.assertEqual([x['depth'] for x in c.coherent(lines, 2)], [8, 8])
        self.assertIsNone(c.coherent(lines[:1], 2))

    def test_later_bound_invalidates_previous_exact_slot(self):
        exact = [self.line(8, 1), self.line(8, 2)]
        self.assertIsNone(c.coherent(exact + [self.line(8, 1, 'cp 20 lowerbound')], 2))

    def test_reader_records_arrival_before_delayed_consumption(self):
        import time
        e = c.Engine.__new__(c.Engine)
        e.proc = subprocess.Popen([sys.executable, '-c', "print('readyok', flush=True)"],
                                  stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        e.lines = c.queue.Queue()
        e.peak_kb = None
        e._read()
        consumed_after = time.monotonic()
        self.assertEqual(e.get(time.monotonic() + 1), 'readyok')
        self.assertLessEqual(e.last_received_at, consumed_after)
        e.close()

    def test_cleanup_kills_and_waits_even_when_called_twice(self):
        e = c.Engine.__new__(c.Engine)
        e.proc = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(60)'], stdin=subprocess.PIPE, stdout=subprocess.PIPE)
        e.close(); e.close()
        self.assertIsNotNone(e.proc.returncode)

    def test_fragmented_output_and_missing_bestmove_timeout_cleanup(self):
        with tempfile.TemporaryDirectory() as directory:
            fake = Path(directory) / 'fake'
            fake.write_text("""#!/usr/bin/env python3
import sys, time
for line in sys.stdin:
    line = line.strip()
    if line == 'uci':
        sys.stdout.write('uci'); sys.stdout.flush(); time.sleep(.01)
        print('ok', flush=True)
    elif line == 'isready': print('readyok', flush=True)
    elif line == 'go infinite':
        sys.stdout.write('info depth 1 multipv 1 score cp 2 nodes 1 time 1 pv e2')
        sys.stdout.flush(); time.sleep(.01); print('e4', flush=True)
""")
            os.chmod(fake, 0o755)
            e = c.Engine(fake, 1, 1)
            try:
                with self.assertRaises(TimeoutError):
                    c.search(e, c.CORPUS[0][1], 1, 'go-1', 20, 20, False, grace_ms=100)
            finally:
                e.close()
            self.assertIsNotNone(e.proc.returncode)

    def test_ready_timeout_and_cleanup(self):
        with tempfile.TemporaryDirectory() as directory:
            fake = Path(directory) / 'fake'
            fake.write_text("#!/usr/bin/env python3\nimport time\ntime.sleep(60)\n")
            os.chmod(fake, 0o755)
            e = c.Engine.__new__(c.Engine)
            e.proc = subprocess.Popen([str(fake)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
            e.lines = c.queue.Queue()
            e.peak_kb = None
            try:
                with self.assertRaises(TimeoutError): e.until('readyok', .05)
            finally: e.close()
            self.assertIsNotNone(e.proc.returncode)


if __name__ == '__main__': unittest.main()
