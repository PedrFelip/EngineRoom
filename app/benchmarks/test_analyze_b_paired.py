import unittest

from analyze_b_paired import summarize


def snapshot(unresolved=False, score=None):
    score = score or {'Engine': {'Cp': 25}}
    return {'decisions': [{'ply': 1}, {'ply': 2}],
            'screening': [{'ply': 1, 'best': score, 'best_evidence': 7}],
            'revisions': [], 'unresolved': [{'ply': 1, 'reason': 'test'}] if unresolved else [],
            'evidence': [{'id': 7, 'lines': [{'pv': ['e2e4']}]}]}


class SummaryTest(unittest.TestCase):
    def test_unresolved_stays_in_denominator_and_mates_are_not_centipawns(self):
        report = {'samples': [
            {'kind': 'b', 'elapsedMs': 1, 'result': snapshot(unresolved=True)},
            {'kind': 'reference', 'elapsedMs': 2, 'result': snapshot(score={'Engine': {'Mate': 2}})},
        ]}
        result = summarize(report)
        self.assertEqual(result['totalDecisions'], 2)
        self.assertEqual(result['referenceUnresolved'], 1)
        self.assertEqual(result['samples'][0]['unresolved'], 2)
        self.assertEqual(result['samples'][0]['bestComparable'], 0)
        self.assertIsNone(result['samples'][0]['meanAbsoluteCpError'])


if __name__ == '__main__':
    unittest.main()
