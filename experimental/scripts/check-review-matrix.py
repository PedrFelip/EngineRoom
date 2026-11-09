"""Render review combinations and retain diagnostics even when a case fails."""
import argparse
import json
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "artifacts" / "review-matrix"


def walk(node):
    yield node
    for child in node.get("children", []):
        yield from walk(child)


def scene_errors(tree, scene, panel_open):
    keys = {e.get("key") for e in tree["elements"]}
    texts = {e.get("text") for e in tree["elements"]}
    required = {
        "review": ["move-list"], "review-light": ["move-list"],
        "demo-analysis": ["candidate-0", "move-feedback"],
        "demo-summary": ["review-summary"],
        "review-light-variation": ["variation-0"],
        "demo-progress": ["analysis-progress", "cancel-analysis"],
        "demo-error": ["analysis-error", "retry-analysis"],
        "preview": ["move-list"],
    }
    errors = []
    if panel_open and ("compact-review-tabs" not in keys or "compact-panel" in keys):
        errors += [f"Scene {scene}: missing {key}" for key in required[scene] if key not in keys]
        if scene == "preview" and "Nenhuma análise concluída nesta sessão." not in texts:
            errors.append("Scene preview: missing preview explanation")
    return errors


def inspect(tree, panel_open, left_open=None):
    errors = []
    elements = tree["elements"]
    keyed = {e["key"]: e for e in elements if "key" in e}
    width, height = tree["viewport"]
    board = keyed.get("chessboard", {}).get("rect")
    inspector = keyed.get("review-inspector", {}).get("rect")
    body = keyed.get("page-body", {}).get("rect")
    if left_open is not None:
        if not body:
            errors.append("Missing page-body for left sidebar visibility check")
        elif (body[0] >= 55) != left_open:
            errors.append(f"Left sidebar visibility: expected {left_open}, body={body}")
    if tree["warnings"]:
        errors.append(f"Rune warnings: {tree['warnings']}")
    compact = "compact-review-tabs" in keyed
    panel_only = "compact-panel" in keyed
    if (not board and not panel_only) or (board and (board[2] != board[3] or board[2] % 8)):
        errors.append(f"Invalid board geometry: {board}")
    if board and sum(key.startswith("square-") for key in keyed) != 64:
        errors.append("Expected 64 board squares")
    if bool(inspector) != (panel_only if compact else panel_open):
        errors.append(f"Panel visibility: expected {panel_open}, got {inspector}")
    if inspector and (inspector[1] < -0.5 or inspector[1]+inspector[3] > height+0.5):
        errors.append(f"Review panel exceeds viewport: {inspector}")
    if compact:
        for key in ("compact-review-tabs", "playback"):
            rect = keyed.get(key, {}).get("rect")
            if not rect or rect[1] < -0.5 or rect[1]+rect[3] > height+0.5:
                errors.append(f"Compact control outside viewport: {key} {rect}")
    if board and inspector:
        overlap = min(board[0] + board[2], inspector[0] + inspector[2]) - max(board[0], inspector[0]) > 0.5 and min(board[1] + board[3], inspector[1] + inspector[3]) - max(board[1], inspector[1]) > 0.5
        if overlap:
            errors.append("Board overlaps review panel")
    for e in elements:
        if e["opacity"] <= 0.01:
            continue
        x, _, w, h = e["rect"]
        if x < -0.5 or x + w > width + 0.5 or w < 0 or h < 0:
            errors.append(f"Horizontal bounds: {e['path']} {e['rect']}")
    nodes = list(walk(tree["accessibility"]))
    accessible = {n["key"]: n for n in nodes if "key" in n}
    for key in ("first", "previous", "next", "last", "flip", "focus-board"):
        e = keyed.get(key)
        node = accessible.get(key, {})
        disabled = "disabled" in node.get("states", [])
        if not e or (not e["focusable"] and not disabled) or e["rect"][3] < 32:
            errors.append(f"Missing or inaccessible control: {key}")
        node = accessible.get(key, {})
        if node.get("role") != "button" or not node.get("name", "").strip() or (not disabled and not {"click", "focus"}.issubset(node.get("actions", []))):
            errors.append(f"Invalid accessibility control: {key}")
    return {
        "errors": errors, "viewport": [width, height], "board": board,
        "panel": inspector, "body": body, "elements": len(elements),
        "accessibility_nodes": len(nodes),
        "focusable": sum(e["focusable"] for e in elements),
        "focused": [e.get("key", e["path"]) for e in elements if e["focused"]],
        "warnings": tree["warnings"],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--quick", action="store_true", help="Minimum and roomy sizes only")
    parser.add_argument("--analyze-only", action="store_true", help="Recheck existing trees without rendering")
    args = parser.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)
    sizes = ("720x560", "1440x1080") if args.quick else ("720x560", "900x700", "1024x768", "1440x1080")
    results = []
    for scene in ("review", "review-light", "demo-analysis", "demo-summary", "review-light-variation", "demo-progress", "demo-error", "preview"):
        for size in sizes:
            for state in ("open", "left-closed", "panel-closed", "both-closed", "panel-reopened"):
                name = f"{scene}-{size}-{state}"
                prefix = OUT / name
                events = []
                if state in ("left-closed", "both-closed"):
                    events.append({"t": 0.1, "type": "key", "name": "b", "cmd": True})
                if state in ("panel-closed", "both-closed", "panel-reopened"):
                    events.append({"t": 0.3, "type": "key", "name": "b", "base": "b", "ctrl": True, "cmd": True, "alt": True})
                if state == "panel-reopened":
                    events.append({"t": 0.6, "type": "tap", "key": "focus-board"})
                script = prefix.with_suffix(".script.json")
                script.write_text(json.dumps({"events": events}, indent=2) + "\n")
                command = [str(ROOT / "target/debug/engineroom-rune"), "--scene", scene, "--size", size, "--script", str(script), "--at", "1", "--fps", "30", "--snap", str(prefix.with_suffix(".png")), "--tree", str(prefix.with_suffix(".json")), "--scale", "1"]
                started = time.monotonic()
                result = {"case": name, "command": command}
                try:
                    code = 0
                    if not args.analyze_only:
                        for suffix in (".json", ".png"):
                            prefix.with_suffix(suffix).unlink(missing_ok=True)
                        prefix.with_suffix(".log").write_text("")
                        run = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, timeout=60)
                        prefix.with_suffix(".log").write_text(run.stdout + run.stderr)
                        code = run.returncode
                    result["exit_code"] = code if not args.analyze_only else None
                    result["analysis_only"] = args.analyze_only
                    if code:
                        result["errors"] = [f"Renderer exited {code}; see {name}.log"]
                    elif not prefix.with_suffix(".png").is_file():
                        result["errors"] = [f"Missing screenshot: {name}.png"]
                    else:
                        tree = json.loads(prefix.with_suffix(".json").read_text())
                        panel_open = state not in ("panel-closed", "both-closed")
                        result.update(inspect(tree, panel_open, state not in ("left-closed", "both-closed")))
                        result["errors"].extend(scene_errors(tree, scene, panel_open))
                except subprocess.TimeoutExpired as error:
                    def decoded(value):
                        return value.decode(errors="replace") if isinstance(value, bytes) else value or ""
                    prefix.with_suffix(".log").write_text(decoded(error.stdout) + decoded(error.stderr) + "\nRenderer timed out after 60 seconds\n")
                    result["errors"] = [str(error)]
                except (OSError, ValueError, KeyError, TypeError, IndexError, AttributeError) as error:
                    result["errors"] = [str(error)]
                result["seconds"] = round(time.monotonic() - started, 3)
                results.append(result)
                prefix.with_suffix(".result.json").write_text(json.dumps(result, indent=2) + "\n")
                print(f"{'FAIL' if result['errors'] else 'PASS'} {name}", flush=True)
    failed = sum(bool(r["errors"]) for r in results)
    (OUT / "report.json").write_text(json.dumps({"cases": len(results), "failed": failed, "results": results}, indent=2) + "\n")
    lines = ["# Headless review matrix", "", f"Cases: {len(results)}; failures: {failed}", "", "| Case | Status | Board | Panel | Elements | AX nodes | Seconds |", "| --- | --- | --- | --- | --- | --- | --- |"]
    for r in results:
        lines.append(f"| [{r['case']}]({r['case']}.png) | {'FAIL' if r['errors'] else 'PASS'} | {r.get('board')} | {r.get('panel')} | {r.get('elements', '—')} | {r.get('accessibility_nodes', '—')} | {r['seconds']} |")
    lines += ["", "## Failures", ""]
    for r in results:
        for error in r["errors"]:
            lines.append(f"- {r['case']}: {error}")
    (OUT / "report.md").write_text("\n".join(lines) + "\n")
    print(f"Report: {OUT / 'report.md'}")
    return int(failed > 0)


if __name__ == "__main__":
    raise SystemExit(main())
