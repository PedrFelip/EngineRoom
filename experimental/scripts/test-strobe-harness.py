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
METRIC = re.compile(r"^strobe:metric ([a-z_]+)=(\d+)(?: (lower|higher))?$")
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


def example_path(name: str, override_env: str) -> Path:
    """Resolve a built example, with an environment override for custom paths."""
    override = os.environ.get(override_env)
    if override:
        return Path(override)
    for candidate in (
        ROOT / f"target/release/examples/{name}",
        ROOT / f"target/debug/examples/{name}",
    ):
        if candidate.is_file():
            return candidate
    return ROOT / f"target/release/examples/{name}"


def parse_metrics(stdout: str) -> dict[str, tuple[int, str]]:
    """Read `strobe:metric` rows; an absent direction is `lower`."""
    metrics: dict[str, tuple[int, str]] = {}
    for line in stdout.splitlines():
        match = METRIC.match(line.strip())
        if match:
            name, value, direction = match.groups()
            metrics[name] = (int(value), direction or "lower")
    return metrics


def run_binary(test: unittest.TestCase, binary: Path, *args: str) -> dict[str, tuple[int, str]]:
    result = subprocess.run(
        [str(binary), *args],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    test.assertEqual(result.returncode, 0, f"{binary.name} {args}: {result.stderr}")
    return parse_metrics(result.stdout)


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


# Partial-cache / resume harness. Interruptions at 2, 15 and 2 searches give
# resumes of 5, 30 and 4; `long` also crosses the eight-entry periodic flush.
CACHE_CASES = {
    "short": {
        "fixture_positions": 8,
        "fixture_moves": 7,
        "fixture_terminals": 1,
        "fixture_search_positions": 7,
        "interrupt_after": 2,
        "interrupt_go": 3,
        "interrupt_completed": 2,
        "interrupt_progress_searched": 2,
        "partial_cache_entries": 2,
        "resume_cache_hits": 2,
        "resume_searched": 5,
        "resume_go": 5,
        "result_positions": 8,
        "result_moves": 7,
    },
    "long": {
        "fixture_positions": 45,
        "fixture_moves": 44,
        "fixture_terminals": 0,
        "fixture_search_positions": 45,
        "interrupt_after": 15,
        "interrupt_go": 16,
        "interrupt_completed": 15,
        "interrupt_progress_searched": 15,
        "partial_cache_entries": 15,
        "resume_cache_hits": 15,
        "resume_searched": 30,
        "resume_go": 30,
        "result_positions": 45,
        "result_moves": 44,
    },
    "fen": {
        "fixture_positions": 6,
        "fixture_moves": 5,
        "fixture_terminals": 0,
        "fixture_search_positions": 6,
        "interrupt_after": 2,
        "interrupt_go": 3,
        "interrupt_completed": 2,
        "interrupt_progress_searched": 2,
        "partial_cache_entries": 2,
        "resume_cache_hits": 2,
        "resume_searched": 4,
        "resume_go": 4,
        "result_positions": 6,
        "result_moves": 5,
    },
}
CACHE_GLOBAL = {
    "interrupt_progress_cached": 0,
    "ports_acquired": 2,
    "ports_terminated": 2,
    "ports_active": 0,
    "ports_peak_active": 1,
    "teardown_ok": 1,
    "deterministic_ok": 1,
    "fake_engine": 1,
}
CACHE_HIGHER = {
    "fixture_positions",
    "fixture_moves",
    "partial_cache_entries",
    "resume_cache_hits",
    "result_positions",
    "result_moves",
    "deterministic_ok",
    "teardown_ok",
    "fake_engine",
}
CACHE_NAMES = sorted(CACHE_CASES["short"] | CACHE_GLOBAL)


class StrobeCacheHarnessTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.binary = example_path("pipeline_strobe_cache", "STROBE_CACHE_HARNESS")
        cls.fixtures = ROOT / "perf" / "fixtures"
        if not cls.binary.is_file():
            raise unittest.SkipTest(f"cache harness not built: {cls.binary}")
        missing = [s for s in CACHE_CASES if not (cls.fixtures / f"{s}.pgn").is_file()]
        if missing:
            raise unittest.SkipTest(f"missing cache fixtures: {missing}")

    def run_scenario(self, scenario: str, *extra: str) -> dict[str, tuple[int, str]]:
        metrics = run_binary(
            self,
            self.binary,
            scenario,
            str(self.fixtures / f"{scenario}.pgn"),
            *extra,
        )
        self.assertEqual(sorted(metrics), CACHE_NAMES, f"{scenario}: unexpected metric names")
        return metrics

    def test_cache_scenarios_are_deterministic(self):
        for scenario, expected in CACHE_CASES.items():
            with self.subTest(scenario=scenario):
                metrics = self.run_scenario(scenario)
                for name, value in expected.items():
                    self.assertEqual(metrics[name][0], value, f"{scenario}: {name}")
                for name, value in CACHE_GLOBAL.items():
                    self.assertEqual(metrics[name][0], value, f"{scenario}: {name}")

    def test_cache_metric_directions_are_stable(self):
        for scenario in CACHE_CASES:
            with self.subTest(scenario=scenario):
                metrics = self.run_scenario(scenario)
                for name, (_, direction) in metrics.items():
                    self.assertEqual(
                        direction,
                        "higher" if name in CACHE_HIGHER else "lower",
                        f"{scenario}: {name}",
                    )

    def test_long_interrupt_at_flush_boundary(self):
        # Eight completed positions exercise the periodic flush as well as the
        # failure flush; the resumed pass must still read all eight from SQLite.
        metrics = self.run_scenario("long", "8")
        self.assertEqual(metrics["interrupt_completed"][0], 8)
        self.assertEqual(metrics["partial_cache_entries"][0], 8)
        self.assertEqual(metrics["resume_cache_hits"][0], 8)
        self.assertEqual(metrics["resume_searched"][0], 37)
        self.assertEqual(metrics["ports_active"][0], 0)

    def test_invalid_scenario_and_fixture_fail(self):
        result = subprocess.run(
            [str(self.binary), "nope", str(self.fixtures / "short.pgn")],
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertNotEqual(result.returncode, 0)
        result = subprocess.run(
            [str(self.binary), "short", str(self.fixtures / "missing.pgn")],
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
