#!/usr/bin/env python3
"""Asserts the Strobe harness counters for each pipeline stage.

These tests are deterministic: the fake engine never starts Stockfish and the
SQLite cache lives in a directory removed at the end of each run.
"""

import argparse
import os
import re
import subprocess
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
METRIC = re.compile(r"^strobe:metric ([a-z]+)=(\d+)(?: (lower|higher))?$")
HIGHER = {"positions", "moves", "cached", "live", "classifications"}
NAMES = sorted(HIGHER | {"searched", "searches"})


def harness_path() -> Path:
    override = os.environ.get("STROBE_HARNESS")
    if override:
        return Path(override)
    for candidate in (
        ROOT / "target/release/examples/pipeline_strobe",
        ROOT / "target/debug/examples/pipeline_strobe",
    ):
        if candidate.is_file():
            return candidate
    return ROOT / "target/release/examples/pipeline_strobe"


class StrobeHarnessTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.binary = harness_path()
        cls.pgn = ROOT / "perf/data/immortal.pgn"
        if not cls.binary.is_file():
            raise unittest.SkipTest(
                f"harness not built: {cls.binary} (run scripts/check-strobe.sh)"
            )

    def run_scenario(self, scenario: str) -> dict[str, tuple[int, str]]:
        result = subprocess.run(
            [str(self.binary), scenario, str(self.pgn)],
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(result.returncode, 0, f"{scenario}: {result.stderr}")
        metrics = {}
        for line in result.stdout.splitlines():
            match = METRIC.match(line.strip())
            if match:
                name, value, direction = match.groups()
                metrics[name] = (int(value), direction or "lower")
        self.assertEqual(sorted(metrics), NAMES, f"{scenario}: unexpected metric names")
        return metrics

    def test_fixture_has_the_expected_shape(self):
        self.assertTrue(self.pgn.is_file(), f"missing fixture: {self.pgn}")
        result = self.run_scenario("pgn")
        self.assertEqual(result["positions"][0], 46)
        self.assertEqual(result["moves"][0], 45)
        self.assertEqual(result["searches"][0], 0)

    def test_metric_directions_are_stable(self):
        for scenario in ("pgn", "manual", "manual-warm", "live", "automatic"):
            with self.subTest(scenario=scenario):
                metrics = self.run_scenario(scenario)
                for name, (_, direction) in metrics.items():
                    self.assertEqual(direction, "higher" if name in HIGHER else "lower")

    def test_manual_cold_reaches_the_engine(self):
        metrics = self.run_scenario("manual")
        self.assertEqual(metrics["positions"][0], 46)
        self.assertEqual(metrics["moves"][0], 45)
        self.assertEqual(metrics["cached"][0], 0)
        # One terminal position is solved without an engine search.
        self.assertEqual(metrics["searched"][0], 45)
        self.assertEqual(metrics["searches"][0], 45)

    def test_manual_warm_uses_only_the_sqlite_cache(self):
        metrics = self.run_scenario("manual-warm")
        self.assertEqual(metrics["positions"][0], 46)
        self.assertEqual(metrics["cached"][0], 45)
        self.assertEqual(metrics["searched"][0], 0)
        # The first pass still searched 45 positions; the second used the cache.
        self.assertEqual(metrics["searches"][0], 45)

    def test_live_emits_completion_and_move_feedback(self):
        metrics = self.run_scenario("live")
        self.assertEqual(metrics["live"][0], 1)
        self.assertEqual(metrics["classifications"][0], 1)
        # The analysed FEN plus its source FEN are evaluated.
        self.assertEqual(metrics["searches"][0], 2)

    def test_automatic_builds_the_full_review(self):
        metrics = self.run_scenario("automatic")
        self.assertEqual(metrics["positions"][0], 46)
        self.assertEqual(metrics["moves"][0], 45)
        self.assertGreaterEqual(metrics["searched"][0], 45)
        self.assertEqual(metrics["searched"][0], metrics["searches"][0])

    def test_unknown_scenario_fails(self):
        result = subprocess.run(
            [str(self.binary), "nope", str(self.pgn)],
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertNotEqual(result.returncode, 0)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("unittest_args", nargs="*")
    args, rest = parser.parse_known_args()
    sys.argv = [sys.argv[0], *args.unittest_args, *rest]
    unittest.main()
