#!/usr/bin/env python3
"""Three-position, isolated Stockfish continuity window; no application database."""
import argparse
import concurrent.futures
import copy
import hashlib
import json
import queue
import threading
import time
from pathlib import Path

from continuity_experiment import CORPUS, Engine, PROFILE_FILE, coherent, parse_info


def score_white(checkpoint, fen):
    if not checkpoint:
        return None
    score = checkpoint[0]['score']
    value = score['value'] if score['type'] == 'cp' else (10000 if score['value'] > 0 else -10000)
    return value if fen.split()[1] == 'w' else -value


class Window:
    """Logical work is distinct from a process/go; all shared transitions hold cv."""
    def __init__(self, corpus, triage_ms, extra_ms, multipv, factory, wait_ms=150):
        if not 2 <= len(corpus) <= 3:
            raise ValueError('window must contain 2 or 3 positions')
        self.corpus, self.triage, self.extra, self.multipv = corpus, triage_ms, extra_ms, multipv
        self.factory, self.wait_ms = factory, min(wait_ms, extra_ms)
        self.cv = threading.Condition()
        self.baselines, self.finished, self.decisions = {}, set(), {}
        self.events, self.results = [], {}
        self.cancelled = False
        self.go_counter = 0
        # Identical positions share a single search and baseline.
        self.owner = [next(j for j in range(i + 1) if corpus[j][1] == item[1])
                      for i, item in enumerate(corpus)]

    def event(self, kind, index, go_id, **data):
        with self.cv:
            self.events.append(dict(kind=kind, position=index, go_id=go_id, **data))

    def next_go(self):
        with self.cv:
            self.go_counter += 1
            return f'go-{self.go_counter}'

    def decision(self, i):
        neighbors = [self.owner[j] for j in (i - 1, i + 1) if 0 <= j < len(self.corpus)]
        required = set(neighbors + [self.owner[i]])
        if not required.issubset(self.baselines):
            return None
        own = score_white(self.baselines[self.owner[i]], self.corpus[i][1])
        adjacent = [score_white(self.baselines[n], self.corpus[n][1]) for n in neighbors]
        # Experimental local criterion: >= 100 centipawn swing against any adjacent
        # baseline, both normalized to White. Missing scores never promote.
        return own is not None and any(v is not None and abs(own - v) >= 100 for v in adjacent)

    def run_one(self, i, restart=False, restart_budget=None):
        engine = None
        go_id = self.next_go()
        lines = []
        started = time.monotonic()
        baseline_at = None
        decision = True if restart else None
        stopped = False
        try:
            engine = self.factory()
            engine.send(f'setoption name MultiPV value {self.multipv}'); engine.ready()
            engine.send('position fen ' + self.corpus[i][1])
            engine.send('go infinite'); started = time.monotonic()
            self.event('go', i, go_id, restart=restart)
            while True:
                now = time.monotonic(); elapsed = (now - started) * 1000
                with self.cv:
                    if self.cancelled:
                        raise RuntimeError('window cancelled')
                    if not restart and baseline_at is None and elapsed >= self.triage:
                        baseline_at = elapsed
                        # Include every line already received by the reader at the cutoff.
                        if hasattr(engine, 'lines'):
                            while True:
                                try:
                                    received_at, queued = engine.lines.get_nowait()
                                except queue.Empty:
                                    break
                                if queued is None:
                                    raise RuntimeError('Stockfish exited unexpectedly')
                                parsed = parse_info(queued, go_id, (received_at-started)*1000)
                                if parsed:
                                    lines.append(parsed)
                        snapshot = copy.deepcopy(coherent([line for line in lines if line['wall_ms'] <= self.triage], self.multipv))
                        self.baselines[i] = snapshot
                        self.event('baseline', i, go_id, elapsed_ms=elapsed, lines=snapshot)
                        self.cv.notify_all()
                    if baseline_at is not None and decision is None and not stopped:
                        decision = self.decision(i)
                        if decision is not None:
                            self.decisions[i] = decision
                            self.event('promote' if decision else 'discard', i, go_id,
                                       elapsed_ms=elapsed, wait_ms=elapsed-baseline_at)
                    limit = restart_budget if restart else (self.triage + self.extra if decision else
                            self.triage + self.wait_ms if decision is None else self.triage)
                    if not stopped and elapsed >= limit:
                        engine.send('stop'); stopped = True
                        self.event('stop', i, go_id, elapsed_ms=elapsed, limit_ms=limit)
                if elapsed > limit + 10000:
                    raise TimeoutError('search did not terminate')
                try:
                    line = engine.get(min(started + (limit + 10000) / 1000,
                                          time.monotonic() + .01))
                except TimeoutError:
                    continue
                elapsed = (time.monotonic() - started) * 1000
                if line.startswith('bestmove'):
                    self.event('bestmove', i, go_id, elapsed_ms=elapsed, unexpected=not stopped)
                    break
                received_ms = (getattr(engine, 'last_received_at', time.monotonic()) - started) * 1000
                parsed = parse_info(line, go_id, received_ms)
                if parsed: lines.append(parsed)
            with self.cv:
                self.finished.add(i); self.cv.notify_all()
            return dict(go_id=go_id, elapsed_ms=elapsed, baseline_at_ms=baseline_at,
                        decision=decision, final=copy.deepcopy(coherent(lines, self.multipv)),
                        peak_rss_kb=engine.peak_kb)
        finally:
            if engine is not None:
                engine.close()

    def run(self):
        owners = [i for i, x in enumerate(self.owner) if i == x]
        try:
            with concurrent.futures.ThreadPoolExecutor(max_workers=len(owners)) as pool:
                futures = {pool.submit(self.run_one, i): i for i in owners}
                try:
                    for future in concurrent.futures.as_completed(futures):
                        self.results[futures[future]] = [future.result()]
                except BaseException:
                    with self.cv:
                        self.cancelled = True; self.cv.notify_all()
                    for f in futures: f.cancel()
                    raise
            # A search that stopped awaiting neighbors can still be selected. Restart
            # explicitly with the extra budget; never claim this is the same go.
            for i in owners:
                if i not in self.decisions:
                    decision = self.decision(i)
                    self.decisions[i] = decision
                    self.event('late_promote' if decision else 'late_discard', i,
                               self.results[i][0]['go_id'])
                    if decision:
                        remaining = max(0, self.triage + self.extra - self.results[i][0]['elapsed_ms'])
                        if remaining > 0:
                            self.results[i].append(self.run_one(i, restart=True, restart_budget=remaining))
                        else:
                            self.event('restart_budget_exhausted', i, self.results[i][0]['go_id'])
            return {'owner': self.owner, 'events': self.events, 'results': self.results,
                    'promotions': sum(v is True for v in self.decisions.values()),
                    'restarts': sum(len(v)-1 for v in self.results.values())}
        finally:
            self.cancelled = True


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary', type=Path, default=Path(__file__).resolve().parent.parent / 'src-tauri/binaries/stockfish-x86_64-unknown-linux-gnu')
    p.add_argument('--corpus', type=Path, help='JSON array of 2 or 3 consecutive [name, FEN] positions')
    p.add_argument('--profile', choices=['fast', 'deep'], default='fast')
    p.add_argument('--target', choices=['medium', 'high'], default='medium')
    p.add_argument('--threads-total', type=int, default=3)
    p.add_argument('--hash-total-mb', type=int, default=48)
    p.add_argument('--wait-ms', type=int, default=150)
    p.add_argument('--output', type=Path, required=True)
    a = p.parse_args()
    if a.threads_total < 3 or a.hash_total_mb < 3 or a.wait_ms < 0: p.error('need at least 3 threads and 3 MiB hash; wait must be nonnegative')
    profile = json.loads(PROFILE_FILE.read_text())[a.profile]
    corpus = json.loads(a.corpus.read_text()) if a.corpus else CORPUS
    if not 2 <= len(corpus) <= 3:
        p.error('corpus must contain 2 or 3 positions')
    threads, hash_mb = a.threads_total // len(corpus), a.hash_total_mb // len(corpus)
    binary = a.binary.resolve()
    window = Window(corpus, profile['triageMs'], profile[a.target+'Ms'], profile['triageMultipv'],
                    lambda: Engine(binary, threads, hash_mb), a.wait_ms)
    start = time.monotonic(); result = window.run()
    result.update(binary=str(binary), binary_sha256=hashlib.file_digest(binary.open('rb'), 'sha256').hexdigest(),
                  profile=a.profile, profile_values=profile, target=a.target,
                  nominal_promoted_ms=profile['triageMs'] + profile[a.target+'Ms'],
                  corpus=corpus, threads_per_process=threads, hash_mb_per_process=hash_mb,
                  threads_total_cap=a.threads_total, hash_total_cap_mb=a.hash_total_mb,
                  wait_limit_ms=a.wait_ms, wall_ms=(time.monotonic()-start)*1000)
    a.output.parent.mkdir(parents=True, exist_ok=True)
    a.output.write_text(json.dumps(result, indent=2)+'\n')

if __name__ == '__main__': main()
