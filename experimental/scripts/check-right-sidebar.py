"""Check resize direction, keyboard adjustment, focus return and panel continuity."""
import json
from pathlib import Path

ARTIFACTS = Path(__file__).resolve().parent.parent / "artifacts"

def walk(node):
    yield node
    for child in node.get("children", []):
        yield from walk(child)

for scene in ("review", "review-light"):
    for checkpoint in ("1.2", "1.6", "2.2"):
        path = ARTIFACTS / f"right-{scene}-{checkpoint}.json"
        tree = json.loads(path.read_text())
        keyed = {e["key"]: e for e in tree["elements"] if "key" in e}
        accessible = {n["key"]: n for n in walk(tree["accessibility"]) if "key" in n}
        toggle = accessible["focus-board"]
        assert toggle["shortcut"] == "Ctrl+Alt+B", path.name
        assert ("expanded" if checkpoint != "1.6" else "collapsed") in toggle["states"], path.name
        assert not tree["warnings"], (path.name, tree["warnings"])
        if checkpoint == "1.6":
            assert "review-inspector" not in keyed, path.name
            assert keyed["focus-board"]["focused"], (path.name, "focus returns to panel toggle")
        else:
            assert abs(keyed["review-inspector"]["rect"][2] - 384) < 0.5, (path.name, "drag grows right panel; Left decreases width by 8")
            assert "candidate-0" in keyed, (path.name, "analysis tab survives toggling")
            assert keyed["review-sidebar-grip"]["focusable"], path.name
            grip = accessible["review-sidebar-grip"]
            assert grip["role"] == "splitter" and grip["range"] == [384, 304, 392, 8], path.name
            assert {"increment", "decrement", "set-value"}.issubset(grip["actions"]), path.name
            assert "review-sidebar-handle" not in keyed, (path.name, "resize area has no visible handle")
            assert "checked" in accessible["candidate-0"]["states"], (path.name, "selected candidate exposed")
            assert "unchecked" in accessible["candidate-1"]["states"], (path.name, "other candidate unselected")
        assert "11 / 16" in {e.get("text") for e in tree["elements"]}, (path.name, "selected move retained")
        print(f"{path.stem}: resize, focus and continuity OK")
