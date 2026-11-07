import threading
import unittest
from unittest.mock import patch

from continuity_window import Window

FEN = '8/8/8/8/8/8/4K3/7k w - - 0 1'

def checkpoint(value):
    return [{'score': {'type': 'cp', 'value': value}, 'pv': ['e2e3']}]

class WindowTests(unittest.TestCase):
    def test_local_decisions_and_dedup(self):
        w = Window([('a', FEN), ('b', FEN.replace(' w ', ' b ')), ('c', FEN)], 1, 2, 1, None)
        self.assertEqual(w.owner, [0, 1, 0])
        w.baselines = {0: checkpoint(0), 1: checkpoint(-150)}
        self.assertTrue(w.decision(0))
        w.baselines[1] = checkpoint(-10)
        self.assertFalse(w.decision(0))
        w.baselines[1] = None
        self.assertFalse(w.decision(0))

    def test_same_go_promotion_and_limits(self):
        class Engine:
            def __init__(self): self.peak_kb = 0; self.stopped = False; self.closed = False
            def send(self, s):
                if s == 'stop': self.stopped = True
            def ready(self): pass
            def get(self, deadline):
                if self.stopped: return 'bestmove e2e3'
                raise TimeoutError()
            def close(self): self.closed = True
        w = Window([('a', FEN), ('b', FEN.replace(' w ', ' b '))], 0, 1, 1, Engine)
        # Supply immutable neighboring baselines; decision happens while the same go is live.
        w.baselines = {0: checkpoint(0), 1: checkpoint(-150)}
        with patch.object(w, 'decision', return_value=True):
            result = w.run_one(0)
        events = [e for e in w.events if e['kind'] in ('go', 'promote', 'stop')]
        self.assertEqual(len({e['go_id'] for e in events}), 1)
        self.assertEqual(events[-1]['limit_ms'], 1)
        self.assertTrue(result['decision'])

    def test_failure_and_timeout_close_all_engines(self):
        made = []
        class Broken:
            def __init__(self): self.closed = False; made.append(self)
            def send(self, s): pass
            def ready(self): pass
            def get(self, deadline): raise RuntimeError('failure')
            def close(self): self.closed = True
        w = Window([('a', FEN), ('b', FEN.replace(' w ', ' b '))], 1, 2, 1, Broken)
        with self.assertRaises(RuntimeError): w.run()
        self.assertTrue(made and all(e.closed for e in made))
        class Timed(Broken):
            def get(self, deadline): raise TimeoutError()
        w = Window([('a', FEN), ('b', FEN.replace(' w ', ' b '))], 0, 0, 1, Timed)
        clock = iter([0, 0, 0, 11, 11, 11, 11])
        with patch('continuity_window.time.monotonic', side_effect=lambda: next(clock, 11)):
            with self.assertRaises(TimeoutError): w.run_one(0)
        self.assertTrue(made[-1].closed)

    def test_stopped_search_cannot_be_promoted_as_continuous(self):
        class Engine:
            def __init__(self): self.peak_kb = 0; self.reads = 0
            def send(self, command): pass
            def ready(self): pass
            def close(self): pass
            def get(self, deadline):
                self.reads += 1
                return 'info depth 1 multipv 1 score cp 1 pv e2e3' if self.reads == 1 else 'bestmove e2e3'
        w = Window([('a', FEN), ('b', FEN.replace(' w ', ' b '))], 0, 0, 1, Engine)
        with patch.object(w, 'decision', side_effect=[None, True]) as decide:
            result = w.run_one(0)
        self.assertEqual(decide.call_count, 1)
        self.assertIsNone(result['decision'])
        self.assertNotIn(0, w.decisions)
        self.assertFalse(any(e['kind'] == 'promote' for e in w.events))

    def test_window_bounds(self):
        for n in (1, 4):
            with self.assertRaises(ValueError): Window([('x', FEN)] * n, 1, 2, 1, None)

if __name__ == '__main__': unittest.main()
