#!/usr/bin/env python3
"""Summarize a paired B report without treating unresolved positions as successes."""
import argparse
import json
from pathlib import Path
import statistics


def conclusions(snapshot):
    latest = {entry['ply']: entry for entry in snapshot['screening']}
    for entry in snapshot['revisions']:
        latest[entry['ply']] = entry
    unresolved = {entry['ply'] for entry in snapshot['unresolved']}
    evidence = {item['id']: item for item in snapshot['evidence']}
    result = {}
    for decision in snapshot['decisions']:
        ply = decision['ply']
        conclusion = latest.get(ply)
        item = evidence.get(conclusion.get('best_evidence')) if conclusion else None
        line = item['lines'][0] if item and item['lines'] else None
        score = conclusion.get('best', {}).get('Engine', {}) if conclusion else {}
        result[ply] = {
            'unresolved': ply in unresolved or not conclusion,
            'best': line['pv'][0] if line and line['pv'] else None,
            'cp': score.get('Cp'),
            'score': conclusion.get('best') if conclusion else None,
        }
    return result


def summarize(report):
    reference = next(s for s in report['samples'] if s['kind'] == 'reference')
    target = conclusions(reference['result'])
    output = {'schema': 1, 'referenceElapsedMs': reference['elapsedMs'],
              'totalDecisions': len(target), 'referenceUnresolved': sum(x['unresolved'] for x in target.values()),
              'samples': []}
    for sample in report['samples']:
        if sample['kind'] == 'reference':
            continue
        if sample['kind'] == 'legacy':
            review = sample['result']
            observed = {move['ply']: {'unresolved': False, 'best': move.get('bestUci'),
                        # The legacy parser encodes mates in cp; do not compare large values.
                        'cp': review['positions'][move['ply'] - 1]['cp'] if abs(review['positions'][move['ply'] - 1]['cp']) < 30000 else None}
                        for move in review['moves']}
        else:
            observed = conclusions(sample['result'])
        rows = []
        for ply, expected in target.items():
            actual = observed.get(ply, {'unresolved': True, 'best': None, 'cp': None})
            eligible = not expected['unresolved'] and not actual['unresolved']
            match = actual['best'] == expected['best'] if eligible and actual['best'] and expected['best'] else None
            error = abs(actual['cp'] - expected['cp']) if eligible and actual['cp'] is not None and expected['cp'] is not None else None
            rows.append({'ply': ply, 'referenceUnresolved': expected['unresolved'],
                         'sampleUnresolved': actual['unresolved'], 'referenceBest': expected['best'],
                         'sampleBest': actual['best'], 'referenceScore': expected.get('score'),
                         'sampleCp': actual['cp'], 'bestMatches': match, 'absoluteCpError': error})
        errors = [r['absoluteCpError'] for r in rows if r['absoluteCpError'] is not None]
        output['samples'].append({'kind': sample['kind'], 'elapsedMs': sample['elapsedMs'],
            'unresolved': sum(r['sampleUnresolved'] for r in rows),
            'bestMatches': sum(r['bestMatches'] is True for r in rows),
            'bestComparable': sum(r['bestMatches'] is not None for r in rows),
            'cpComparable': len(errors), 'meanAbsoluteCpError': statistics.mean(errors) if errors else None,
            'decisions': rows})
    return output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('report', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    data = summarize(json.loads(args.report.read_text()))
    with args.output.open('x') as output:
        json.dump(data, output, indent=2)
        output.write('\n')


if __name__ == '__main__':
    main()
