#!/usr/bin/env python3
"""Compare preserved release binaries in A/B/B/A order; no compilation during runs."""
import argparse
import hashlib
import json
from pathlib import Path
import statistics
from types import SimpleNamespace

from run import run

SCENARIOS = [
    'search-depth-pv3',
    'game-long-depth-pv3-cold',
    'game-long-depth-pv3-warm',
    'adaptive-long-fast-cold',
    'adaptive-long-fast-warm',
    'adaptive-long-deep-cold',
    'adaptive-long-deep-warm',
]


def distribution(values):
    ordered = sorted(values)
    quartiles = statistics.quantiles(ordered, n=4) if len(ordered) > 1 else ordered * 3
    return {'median': statistics.median(ordered), 'min': ordered[0],
            'max': ordered[-1], 'q1': quartiles[0], 'q3': quartiles[2]}


def summarize(runs):
    scenarios = [r['latency']['report']['scenarios'][0] for r in runs]
    allocations = [sample for r in runs
                   for sample in r['allocations']['report']['scenarios'][0]['allocationSamples']]
    batch = scenarios[0]['batch']
    return {
        'latencyMsPerOperation': distribution([v for s in scenarios for v in s['elapsedMs']]),
        'allocationCallsPerOperation': distribution([a[0] / batch for a in allocations]),
        'allocatedBytesPerOperation': distribution([a[1] / batch for a in allocations]),
        'peakIncrementalLiveBytesPerBatch': distribution([a[2] for a in allocations]),
        'cpuUserSecondsPerProcess': distribution([r['latency']['cpu_user_seconds_wait4'] for r in runs]),
        'cpuSystemSecondsPerProcess': distribution([r['latency']['cpu_system_seconds_wait4'] for r in runs]),
        'sampledRustRssKibPerProcess': [r['latency']['process_peak_rss_kib_sampled'] for r in runs],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for version in ('baseline', 'candidate'):
        for kind in ('normal', 'alloc'):
            parser.add_argument(f'--{version}-{kind}', required=True, type=Path)
    parser.add_argument('--scenario', action='append')
    parser.add_argument('--engine', choices=['fake', 'stockfish'], default='fake')
    parser.add_argument('--samples', type=int, default=15)
    parser.add_argument('--warmups', type=int, default=3)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.samples < 1 or args.warmups < 0:
        parser.error('samples must be positive and warmups nonnegative')
    args.output.mkdir(parents=True, exist_ok=True)
    summary = {'schema': 1, 'order': ['baseline', 'candidate', 'candidate', 'baseline'],
               'samplesPerProcess': args.samples, 'warmupsPerProcess': args.warmups,
               'binaries': {}, 'scenarios': {}}
    for version in ('baseline', 'candidate'):
        for kind in ('normal', 'alloc'):
            path = getattr(args, f'{version}_{kind}').resolve()
            summary['binaries'][f'{version}-{kind}'] = hashlib.sha256(path.read_bytes()).hexdigest()
    for scenario in args.scenario or SCENARIOS:
        collected = {'baseline': [], 'candidate': []}
        expected = None
        config = SimpleNamespace(scenario=scenario, engine=args.engine,
                                 samples=args.samples, warmups=args.warmups)
        for index, version in enumerate(summary['order']):
            print(f'{scenario}: {index + 1}/4 {version}', flush=True)
            result = {'latency': run(str(getattr(args, f'{version}_normal').resolve()), config, False),
                      'allocations': run(str(getattr(args, f'{version}_alloc').resolve()), config, True)}
            (args.output / f'{scenario}-{index + 1}-{version}.json').write_text(
                json.dumps(result, indent=2) + '\n')
            if args.engine == 'fake':
                for key in ('latency', 'allocations'):
                    observed = result[key]['report']['scenarios'][0]
                    work = (observed['observed'], observed['statsSamples'])
                    if expected is None:
                        expected = work
                    if work != expected:
                        raise RuntimeError(f'{scenario}: results or work differ across versions/runs')
            collected[version].append(result)
        entry = {version: summarize(runs) for version, runs in collected.items()}
        entry['equivalentFakeWork'] = args.engine == 'fake'
        entry['medianChangePercent'] = {}
        for metric in ('latencyMsPerOperation', 'allocationCallsPerOperation',
                       'allocatedBytesPerOperation', 'peakIncrementalLiveBytesPerBatch'):
            before = entry['baseline'][metric]['median']
            after = entry['candidate'][metric]['median']
            entry['medianChangePercent'][metric] = (after / before - 1) * 100 if before else None
        summary['scenarios'][scenario] = entry
        (args.output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')


if __name__ == '__main__':
    main()
