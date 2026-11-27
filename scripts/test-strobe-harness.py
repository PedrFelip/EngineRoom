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
import tempfile
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


# Window-owned sessions harness: searches are 1/2/2 and the exclusive lease is
# always released (acquires == shutdowns, active == 0, permits == 1).
SESSIONS_CASES = {
    "cancel-analysis": {
        "sessions_opened": 1,
        "jobs_enqueued": 1,
        "searches": 1,
        "acquires": 1,
        "shutdowns": 1,
        "live_started": 1,
        "live_completed": 0,
        "latest_completed": 0,
        "stale_events": 0,
        "accepted_events": 2,
        "max_sequence": 2,
        "cancels_acked": 1,
        "closes_acked": 1,
        "auth_rejected": 0,
        "shutdown_rejects": 0,
        "repo_drained": 1,
    },
    "navigation-latest-wins": {
        "sessions_opened": 1,
        "jobs_enqueued": 4,
        "searches": 2,
        "acquires": 2,
        "shutdowns": 2,
        "live_started": 2,
        "live_completed": 1,
        "latest_completed": 1,
        "stale_events": 1,
        "accepted_events": 3,
        "max_sequence": 4,
        "cancels_acked": 0,
        "closes_acked": 1,
        "auth_rejected": 0,
        "shutdown_rejects": 0,
        "repo_drained": 1,
    },
    "session-lifecycle": {
        "sessions_opened": 2,
        "jobs_enqueued": 2,
        "searches": 2,
        "acquires": 2,
        "shutdowns": 2,
        "live_started": 1,
        "live_completed": 1,
        "latest_completed": 1,
        "stale_events": 0,
        "accepted_events": 3,
        "max_sequence": 3,
        "cancels_acked": 0,
        "closes_acked": 3,
        "auth_rejected": 2,
        "shutdown_rejects": 1,
        "repo_drained": 1,
    },
}
SESSIONS_GLOBAL = {
    "fake_engine": 1,
    "peak_active": 1,
    "active": 0,
    "permits": 1,
    "stale_completed": 0,
    "wrong_session": 0,
    "sequence_violations": 0,
}
SESSIONS_HIGHER = {
    "fake_engine",
    "sessions_opened",
    "jobs_enqueued",
    "searches",
    "acquires",
    "shutdowns",
    "permits",
    "live_started",
    "live_completed",
    "latest_completed",
    "stale_events",
    "accepted_events",
    "max_sequence",
    "cancels_acked",
    "closes_acked",
    "auth_rejected",
    "shutdown_rejects",
    "repo_drained",
}
SESSIONS_NAMES = sorted(SESSIONS_CASES["cancel-analysis"] | SESSIONS_GLOBAL)


class StrobeSessionsHarnessTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.binary = example_path("pipeline_strobe_sessions", "STROBE_SESSIONS_HARNESS")
        cls.fixture = ROOT / "perf" / "fixtures" / "short.pgn"
        if not cls.binary.is_file():
            raise unittest.SkipTest(f"sessions harness not built: {cls.binary}")
        if not cls.fixture.is_file():
            raise unittest.SkipTest(f"missing fixture: {cls.fixture}")

    def run_scenario(self, scenario: str, *args: str) -> dict[str, tuple[int, str]]:
        metrics = run_binary(self, self.binary, scenario, *args)
        self.assertEqual(sorted(metrics), SESSIONS_NAMES, f"{scenario}: unexpected metric names")
        return metrics

    def test_session_scenarios_are_deterministic(self):
        for scenario, expected in SESSIONS_CASES.items():
            with self.subTest(scenario=scenario):
                metrics = self.run_scenario(scenario, str(self.fixture))
                for name, value in expected.items():
                    self.assertEqual(metrics[name][0], value, f"{scenario}: {name}")
                for name, value in SESSIONS_GLOBAL.items():
                    self.assertEqual(metrics[name][0], value, f"{scenario}: {name}")
                self.assertEqual(
                    metrics["acquires"][0],
                    metrics["shutdowns"][0],
                    f"{scenario}: every port must be shut down",
                )

    def test_session_metric_directions_are_stable(self):
        for scenario in SESSIONS_CASES:
            with self.subTest(scenario=scenario):
                metrics = self.run_scenario(scenario, str(self.fixture))
                for name, (_, direction) in metrics.items():
                    self.assertEqual(
                        direction,
                        "higher" if name in SESSIONS_HIGHER else "lower",
                        f"{scenario}: {name}",
                    )

    def test_default_pgn_is_usable(self):
        # The fixture is optional: each scenario has a deterministic inline PGN.
        for scenario in SESSIONS_CASES:
            with self.subTest(scenario=scenario):
                metrics = self.run_scenario(scenario)
                for name, value in SESSIONS_GLOBAL.items():
                    self.assertEqual(metrics[name][0], value, f"{scenario}: {name}")

    def test_navigation_rejects_pgn_with_fewer_than_five_fens(self):
        with tempfile.TemporaryDirectory() as folder:
            short = Path(folder) / "short.pgn"
            short.write_text("1. e4 e5 *\n", encoding="utf-8")
            result = subprocess.run(
                [str(self.binary), "navigation-latest-wins", str(short)],
                cwd=ROOT,
                capture_output=True,
                text=True,
                check=False,
            )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("exige pelo menos 5 FENs", result.stderr)

    def test_unknown_scenario_fails(self):
        result = subprocess.run(
            [str(self.binary), "nope", str(self.fixture)],
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
