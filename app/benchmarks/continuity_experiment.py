#!/usr/bin/env python3
"""Isolated UCI continuity experiment; Python standard library only."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import queue
import subprocess
import threading
import time

HERE = Path(__file__).resolve().parent
PROFILE_FILE = HERE.parent / 'src-tauri/src/review/profiles.json'
# First game in games.pgn: Larsen–Spassky, 1970, plies 0, 1 and 2.
CORPUS = [
    ('start', 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1'),
    ('1.b3', 'rnbqkbnr/pppppppp/8/8/8/1P6/P1PPPPPP/RNBQKBNR b KQkq - 0 1'),
    ('1...e5', 'rnbqkbnr/pppp1ppp/8/4p3/8/1P6/P1PPPPPP/RNBQKBNR w KQkq e6 0 2'),
]


def parse_info(line, go_id, wall_ms):
    parts = line.split()
    if not parts or parts[0] != 'info' or 'pv' not in parts or 'score' not in parts:
        return None
    try:
        def number(key, default=None):
            return int(parts[parts.index(key) + 1]) if key in parts else default
        s = parts.index('score')
        if parts[s + 1] not in ('cp', 'mate'):
            return None
        return {'go_id': go_id, 'wall_ms': wall_ms, 'depth': number('depth'),
                'seldepth': number('seldepth'), 'multipv': number('multipv', 1),
                'nodes': number('nodes'), 'time_ms': number('time'),
                'score': {'type': parts[s + 1], 'value': int(parts[s + 2]),
                          'bound': next((x for x in parts[s + 3:parts.index('pv')] if x in ('lowerbound', 'upperbound')), None)},
                'pv': parts[parts.index('pv') + 1:], 'raw': line}
    except (ValueError, IndexError):
        return None


def coherent(lines, multipv):
    """Latest complete unbounded depth, preserving each emitted line's identity."""
    depths = {}
    for line in lines:
        if line['depth'] is not None:
            depths.setdefault(line['depth'], {})[line['multipv']] = line
    for depth in sorted(depths, reverse=True):
        if all(i in depths[depth] and depths[depth][i]['pv'] and depths[depth][i]['score']['bound'] is None
               for i in range(1, multipv + 1)):
            return [dict(depths[depth][i]) for i in range(1, multipv + 1)]
    return None


class Engine:
    def __init__(self, binary, threads, hash_mb):
        self.proc = subprocess.Popen([str(binary)], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                     stderr=subprocess.DEVNULL, text=True, bufsize=1)
        self.lines = queue.Queue()
        self.peak_kb = None
        threading.Thread(target=self._read, daemon=True).start()
        try:
            self.send('uci'); self.until('uciok', 10)
            self.send(f'setoption name Threads value {threads}')
            self.send(f'setoption name Hash value {hash_mb}')
            self.ready()
        except BaseException:
            self.close(); raise

    def _read(self):
        for line in self.proc.stdout:
            self.lines.put((time.monotonic(), line.rstrip()))
        self.lines.put((time.monotonic(), None))

    def send(self, command):
        self.proc.stdin.write(command + '\n'); self.proc.stdin.flush()

    def sample_ram(self):
        try:
            for line in Path(f'/proc/{self.proc.pid}/status').read_text().splitlines():
                if line.startswith('VmHWM:'):
                    self.peak_kb = max(self.peak_kb or 0, int(line.split()[1]))
        except (OSError, ValueError):
            pass

    def get(self, deadline):
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise TimeoutError('Stockfish response deadline exceeded')
        try:
            received_at, line = self.lines.get(timeout=remaining)
        except queue.Empty as exc:
            raise TimeoutError('Stockfish response deadline exceeded') from exc
        if line is None:
            raise RuntimeError('Stockfish exited unexpectedly')
        self.last_received_at = received_at
        self.sample_ram()
        return line

    def until(self, prefix, timeout):
        deadline = time.monotonic() + timeout
        lines = []
        while True:
            line = self.get(deadline); lines.append(line)
            if line.startswith(prefix):
                return lines

    def ready(self):
        self.send('isready'); self.until('readyok', 10)

    def close(self):
        if self.proc.poll() is None:
            self.proc.kill()
        self.proc.wait()
        if self.proc.stdin: self.proc.stdin.close()
        if self.proc.stdout: self.proc.stdout.close()


def search(engine, fen, multipv, go_id, first_ms, total_ms, continuous, grace_ms=10000):
    engine.send(f'setoption name MultiPV value {multipv}'); engine.ready()
    engine.send('position fen ' + fen)
    started = time.monotonic()
    engine.send('go infinite')
    lines = []
    checkpoint = None
    stopped = False
    # Checkpoints are snapshots of complete depths received by the deadline; no waiting
    # for a partial cycle after that deadline.
    while True:
        now = time.monotonic()
        elapsed = (now - started) * 1000
        if checkpoint is None and elapsed >= first_ms:
            # Drain output already timestamped by the reader before taking the snapshot.
            while True:
                try:
                    received_at, queued = engine.lines.get_nowait()
                except queue.Empty:
                    break
                if queued is None:
                    raise RuntimeError('Stockfish exited unexpectedly')
                parsed = parse_info(queued, go_id, (received_at - started) * 1000)
                if parsed:
                    lines.append(parsed)
            checkpoint = {'at_wall_ms': elapsed, 'deadline_ms': first_ms,
                          'lines': coherent([x for x in lines if x['wall_ms'] <= first_ms], multipv)}
            if not continuous:
                engine.send('stop'); stopped = True
        if continuous and elapsed >= total_ms and not stopped:
            engine.send('stop'); stopped = True
        if elapsed > max(total_ms, first_ms) + grace_ms:
            raise TimeoutError('search did not terminate')
        deadline = started + (max(total_ms, first_ms) + grace_ms) / 1000
        next_boundary = (first_ms if checkpoint is None else total_ms if continuous and not stopped else None)
        if next_boundary is not None:
            deadline = min(deadline, started + next_boundary / 1000)
        try:
            line = engine.get(deadline)
        except TimeoutError:
            if time.monotonic() < started + (max(total_ms, first_ms) + grace_ms) / 1000:
                continue
            raise
        elapsed = (time.monotonic() - started) * 1000
        if line.startswith('bestmove'):
            if not stopped:
                raise RuntimeError('unexpected bestmove before stop')
            return {'go_id': go_id, 'wall_ms': elapsed, 'checkpoint': checkpoint,
                    'final': coherent(lines, multipv), 'bestmove': line, 'info': lines}
        parsed = parse_info(line, go_id, (engine.last_received_at - started) * 1000)
        if parsed: lines.append(parsed)


def run_case(binary, threads, hash_mb, fen, mode, triage_ms, total_ms, triage_pv, final_pv):
    engines = []
    start = time.monotonic()
    startup_ms = 0
    teardown_ms = 0
    try:
        t = time.monotonic()
        e = Engine(binary, threads, hash_mb); engines.append(e)
        startup_ms += (time.monotonic() - t) * 1000
        if mode == 'continuous':
            first = search(e, fen, triage_pv, 'go-1', triage_ms, total_ms, True)
            stages = [first]
        else:
            first = search(e, fen, triage_pv, 'go-1', triage_ms, triage_ms, False)
            if mode == 'cold':
                t = time.monotonic(); e.close(); teardown_ms += (time.monotonic() - t) * 1000
                t = time.monotonic(); e = Engine(binary, threads, hash_mb); engines.append(e)
                startup_ms += (time.monotonic() - t) * 1000
            second = search(e, fen, final_pv, 'go-2', max(1, total_ms - triage_ms),
                            max(1, total_ms - triage_ms), False)
            stages = [first, second]
        peak = max((x.peak_kb or 0 for x in engines), default=0) or None
    finally:
        t = time.monotonic()
        for engine in engines:
            engine.close()
        teardown_ms += (time.monotonic() - t) * 1000
    return {'mode': mode, 'wall_ms': (time.monotonic() - start) * 1000,
            'startup_ms': startup_ms, 'teardown_ms': teardown_ms,
            'peak_rss_kb_max_process': peak, 'stages': stages}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary', type=Path, default=HERE.parent / 'src-tauri/binaries/stockfish-x86_64-unknown-linux-gnu')
    p.add_argument('--profile', choices=['fast', 'deep'], default='fast')
    p.add_argument('--target', choices=['medium', 'high'], default='medium')
    p.add_argument('--repetitions', type=int, default=2)
    p.add_argument('--threads', type=int, default=1)
    p.add_argument('--hash-mb', type=int, default=16)
    p.add_argument('--output', type=Path, required=True)
    args = p.parse_args()
    if min(args.repetitions, args.threads, args.hash_mb) < 1: p.error('resources and repetitions must be positive')
    binary = args.binary.resolve()
    profiles = json.loads(PROFILE_FILE.read_text()); profile = profiles[args.profile]
    triage = profile['triageMs']
    total = triage + profile[args.target + 'Ms']
    triage_pv = profile['triageMultipv']
    current_pv = profile['mediumMultipv'] if args.target == 'medium' else profile['refinementMultipv']
    probe = Engine(binary, args.threads, args.hash_mb)
    try:
        probe.send('uci'); identification = probe.until('uciok', 10)
    finally:
        probe.close()
    result = {'binary': str(binary), 'binary_sha256': hashlib.file_digest(binary.open('rb'), 'sha256').hexdigest(),
              'uci_identification': [x for x in identification if x.startswith('id ')],
              'profile_file': str(PROFILE_FILE.relative_to(HERE.parent)), 'profile': args.profile,
              'profile_values': profile, 'target': args.target, 'threads_per_process': args.threads,
              'hash_mb_per_process': args.hash_mb, 'repetitions': args.repetitions,
              'corpus_source': 'benchmarks/games.pgn, first game, initial position and plies 1–2',
              'concurrency': 1, 'runs': []}
    scenarios = [('equal_multipv', triage_pv), ('current_multipv_restart_only', current_pv)]
    for rep in range(args.repetitions):
        for index, (name, fen) in enumerate(CORPUS):
            for scenario, final_pv in scenarios:
                modes = ['continuous', 'warm', 'cold'] if scenario == 'equal_multipv' else ['warm', 'cold']
                shift = (rep + index) % len(modes)
                modes = modes[shift:] + modes[:shift]
                if rep % 2: modes.reverse()
                for mode in modes:
                    print(f'{rep + 1}/{args.repetitions} {name} {scenario} {mode}', flush=True)
                    case = run_case(binary, args.threads, args.hash_mb, fen, mode, triage, total, triage_pv, final_pv)
                    case.update({'repetition': rep + 1, 'position': name, 'fen': fen, 'scenario': scenario,
                                 'triage_multipv': triage_pv, 'final_multipv': final_pv})
                    result['runs'].append(case)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + '\n')

if __name__ == '__main__':
    main()
