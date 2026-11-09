"""Regression tests for the matrix diagnostic collector (no GPU needed)."""
import importlib.util
import contextlib
import io
import json
import subprocess
import tempfile
import unittest
from unittest.mock import patch
from pathlib import Path

spec = importlib.util.spec_from_file_location("matrix", Path(__file__).with_name("check-review-matrix.py"))
matrix = importlib.util.module_from_spec(spec)
spec.loader.exec_module(matrix)


def element(key, rect, focusable=False):
    return {"key": key, "path": key, "rect": rect, "opacity": 1, "focusable": focusable, "focused": False}


def tree():
    elements = [element("chessboard", [240, 100, 400, 400]), element("review-inspector", [660, 100, 320, 500]), element("page-body", [218, 0, 802, 768])]
    elements += [element(f"square-{i}", [240, 100, 50, 50]) for i in range(64)]
    elements += [element(key, [240, 520, 40, 36], True) for key in ("first", "previous", "next", "last", "flip", "focus-board")]
    controls = [{"key": key, "role": "button", "name": key, "actions": ["click", "focus"]} for key in ("first", "previous", "next", "last", "flip", "focus-board")]
    return {"viewport": [1024, 768], "elements": elements, "warnings": [], "accessibility": {"children": controls}}


class DiagnosticsTests(unittest.TestCase):
    def test_successful_exit_cannot_reuse_old_artifacts(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            stale = output / "review-720x560-open"
            stale.with_suffix(".json").write_text(json.dumps(tree()))
            stale.with_suffix(".png").write_bytes(b"stale screenshot")
            with patch.object(matrix, "OUT", output), patch("sys.argv", ["matrix", "--quick"]), patch.object(matrix.subprocess, "run", return_value=subprocess.CompletedProcess("renderer", 0, "", "")), contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(matrix.main(), 1)
            self.assertFalse(stale.with_suffix(".json").exists())
            self.assertFalse(stale.with_suffix(".png").exists())
            self.assertEqual(json.loads((output / "report.json").read_text())["failed"], 80)

    def test_scene_specific_content_is_required(self):
        for scene in ("review", "review-light", "demo-analysis", "demo-summary", "review-light-variation", "demo-progress", "demo-error", "preview"):
            self.assertTrue(matrix.scene_errors(tree(), scene, True), scene)
            self.assertEqual(matrix.scene_errors(tree(), scene, False), [])

    def test_timeout_keeps_partial_output_and_collects_all_cases(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            error = subprocess.TimeoutExpired("renderer", 60, output=b"partial output", stderr=b"partial error")
            with patch.object(matrix, "OUT", output), patch("sys.argv", ["matrix", "--quick"]), patch.object(matrix.subprocess, "run", side_effect=error), contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(matrix.main(), 1)
            report = json.loads((output / "report.json").read_text())
            self.assertEqual(report["failed"], 80)
            self.assertEqual(report["cases"], 80)
            self.assertIn("partial outputpartial error", next(output.glob("*.log")).read_text())
            markdown = (output / "report.md").read_text()
            self.assertEqual(markdown.split("## Failures")[0].count("| FAIL |"), 80)

    def test_malformed_trees_do_not_abort_collection(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            def render(command, **kwargs):
                Path(command[command.index("--tree") + 1]).write_text('{"elements": null}')
                Path(command[command.index("--snap") + 1]).write_bytes(b"screenshot")
                return subprocess.CompletedProcess(command, 0, "", "")
            with patch.object(matrix, "OUT", output), patch("sys.argv", ["matrix", "--quick"]), patch.object(matrix.subprocess, "run", side_effect=render), contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(matrix.main(), 1)
            self.assertEqual(json.loads((output / "report.json").read_text())["failed"], 80)

    def test_missing_page_body_fails_both_sidebar_states(self):
        sample = tree()
        sample["elements"] = [e for e in sample["elements"] if e["key"] != "page-body"]
        for opened in (True, False):
            self.assertIn("Missing page-body for left sidebar visibility check", matrix.inspect(sample, True, opened)["errors"])

    def test_accessibility_requires_named_actionable_buttons(self):
        for field, value in (("role", "label"), ("name", ""), ("actions", ["click"])):
            sample = tree()
            sample["accessibility"]["children"][0][field] = value
            self.assertIn("Invalid accessibility control: first", matrix.inspect(sample, True)["errors"])
        sample = tree()
        sample["accessibility"]["children"] = []
        self.assertIn("Invalid accessibility control: next", matrix.inspect(sample, True)["errors"])

    def test_panel_outside_viewport_fails_even_without_overlap(self):
        sample = tree()
        sample["elements"][1]["rect"] = [660, 507, 320, 560]
        self.assertTrue(any("Review panel exceeds viewport" in e for e in matrix.inspect(sample, True)["errors"]))

    def test_valid_layout(self):
        self.assertEqual(matrix.inspect(tree(), True, True)["errors"], [])

    def test_collects_multiple_failures(self):
        sample = tree()
        sample["warnings"] = ["layout warning"]
        sample["elements"][0]["rect"][2] = 401
        sample["elements"][1]["rect"][0] = 500
        errors = matrix.inspect(sample, False, False)["errors"]
        for message in ("warnings", "geometry", "overlaps", "Panel visibility", "Left sidebar visibility"):
            self.assertTrue(any(message in error for error in errors), (message, errors))

    def test_missing_control_and_overflow(self):
        sample = tree()
        sample["elements"] = [e for e in sample["elements"] if e["key"] != "next"]
        sample["elements"].append(element("overflow", [1020, 0, 40, 40]))
        errors = matrix.inspect(sample, True)["errors"]
        self.assertTrue(any("control: next" in error for error in errors))
        self.assertTrue(any("Horizontal bounds" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
