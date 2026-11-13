#!/usr/bin/env python3
"""Run one fixture scenario in separate latency and allocation processes (Linux)."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]


def binary(instrumented):
    cmd = ['cargo', 'test', '--release', '--lib', '--no-run', '--message-format=json']
    if instrumented:
        cmd += ['--features', 'bench-alloc']
    result = subprocess.run(cmd, cwd=ROOT / 'src-tauri', check=True, text=True, stdout=subprocess.PIPE)
    paths = [m['executable'] for line in result.stdout.splitlines()
             if (m := json.loads(line)).get('reason') == 'compiler-artifact'
             and m.get('profile', {}).get('test') and m.get('executable')
             and m['target']['name'] == 'app_lib']
    if len(paths) != 1:
        raise RuntimeError(f'expected one test executable, got {paths}')
    return paths[0]


def run(executable, args, instrumented):
    with tempfile.TemporaryDirectory() as directory:
        report = Path(directory) / 'report.json'
        env = os.environ.copy()
        env.update(BENCH_SCENARIO=args.scenario, BENCH_ENGINE=args.engine,
                   BENCH_SAMPLES=str(args.samples), BENCH_WARMUPS=str(args.warmups),
                   BENCH_REPORT_PATH=str(report))
        start = time.monotonic()
        process = subprocess.Popen([executable, 'benchmark_complete_pipeline', '--ignored',
                                    '--nocapture', '--test-threads=1'], env=env, stdout=subprocess.DEVNULL)
        rss = None
        while True:
            try:
                for line in Path(f'/proc/{process.pid}/status').read_text().splitlines():
                    if line.startswith('VmHWM:'):
                        rss = max(rss or 0, int(line.split()[1]))
            except (FileNotFoundError, ProcessLookupError):
                pass
            pid, status, usage = os.wait4(process.pid, os.WNOHANG)
            if pid:
                process.returncode = os.waitstatus_to_exitcode(status)
                break
            time.sleep(0.01)
        if process.returncode:
            raise RuntimeError(f'benchmark exited {process.returncode}')
        return {'instrumented': instrumented, 'wall_process_seconds': time.monotonic() - start,
                'cpu_user_seconds_wait4': usage.ru_utime, 'cpu_system_seconds_wait4': usage.ru_stime,
                'process_peak_rss_kib_sampled': rss, 'report': json.loads(report.read_text())}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--scenario', required=True)
    parser.add_argument('--engine', choices=['fake', 'stockfish'], required=True)
    parser.add_argument('--samples', type=int, default=15)
    parser.add_argument('--warmups', type=int, default=3)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--latency-binary', type=Path)
    parser.add_argument('--allocation-binary', type=Path)
    args = parser.parse_args()
    if args.samples < 1 or args.warmups < 0:
        parser.error('samples must be positive and warmups nonnegative')
    if not Path('/proc/self/status').exists():
        parser.error('this runner requires Linux /proc')
    latency = run(str(args.latency_binary.resolve()) if args.latency_binary else binary(False), args, False)
    allocation = run(str(args.allocation_binary.resolve()) if args.allocation_binary else binary(True), args, True)
    if args.engine == 'fake' and any(latency['report']['scenarios'][0][key] != allocation['report']['scenarios'][0][key] for key in ('observed', 'statsSamples')):
        raise RuntimeError('work differs between latency and allocation runs')
    args.output.write_text(json.dumps({'schema': 1, 'scenario': args.scenario, 'engine': args.engine,
        'samples': args.samples, 'warmups': args.warmups, 'latency': latency,
        'allocations': allocation}, indent=2) + '\n')


if __name__ == '__main__':
    main()
