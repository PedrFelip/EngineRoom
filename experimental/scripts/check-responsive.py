"""Check width constraints, board reflow and reachable controls in compact windows."""
import json
from pathlib import Path

ARTIFACTS = Path(__file__).resolve().parent.parent / "artifacts"
paths = [ARTIFACTS / f"{scene}-{size}.json"
         for size in ("720x560", "900x700", "1024x768", "1440x1080")
         for scene in ("home", "review", "settings", "settings-analysis", "manual")]
paths += [ARTIFACTS / f"{name}.json" for name in (
    "settings-light-720x560", "review-narrow-scrolled", "settings-narrow-scrolled",
    "review-collapsed-720x560", "settings-sidebar-wide-720x560")]

def descendants(node):
    yield node
    for child in node.get("children", []):
        yield from descendants(child)

for path in paths:
    tree = json.loads(path.read_text())
    width, height = tree["viewport"]
    elements = tree["elements"]
    keyed = {e["key"]: e for e in elements if "key" in e}
    texts = {e.get("text") for e in elements}
    assert not tree["warnings"], (path.name, tree["warnings"])
    scroll_keys = {"review-scroll", "review-panel-scroll", "board-scroll", "review-settings-scroll", "move-list", "scroll"}
    if "page-content" in keyed:
        scroll_keys.add("page-body")
    scroll_paths = [e["path"] + "/" for e in elements if e.get("key") in scroll_keys]
    for element in elements:
        if element["opacity"] <= 0.01:
            continue
        x, y, w, h = element["rect"]
        assert x >= -0.5 and x + w <= width + 0.5, (path.name, "horizontal overflow", element["path"], element["rect"])
        scrolls = any(element["path"].startswith(prefix) for prefix in scroll_paths)
        assert scrolls or (y >= -0.5 and y + h <= height + 0.5), (path.name, "unscrollable vertical overflow", element["path"], element["rect"])
    if "chessboard" in keyed:
        board = keyed["chessboard"]["rect"]
        body = keyed["page-body"]["rect"]
        inspector = keyed.get("review-inspector", {}).get("rect")
        assert board[2] == board[3] and board[2] % 8 == 0
        assert board[0] >= body[0] and board[0] + board[2] <= body[0] + body[2] + 0.5
        if "review-sidebar-pane" not in keyed:
            assert "review-scroll" in keyed
            assert inspector is None
            assert "compact-review-tabs" in keyed or "focus-board" in keyed
        else:
            assert "review-scroll" not in keyed
            assert inspector[0] >= board[0] + board[2]
            assert inspector[1] < 24, (path.name, "right sidebar uses shell height")
        for key in ("first", "previous", "next", "last", "flip"):
            assert keyed[key]["rect"][3] >= 32, (path.name, key, "target too short")
    if path.stem == "review-narrow-scrolled":
        assert "16 / 16" in texts, "last move remains clickable after nested scrolling"
        x, y, w, h = keyed["move-16"]["rect"]
        assert y >= 0 and y + h <= height, "selected move is on screen"
    if path.stem == "settings-narrow-scrolled":
        switch = next(n for n in descendants(tree["accessibility"]) if n.get("key") == "reduce-motion")
        assert "checked" in switch["states"], "bottom preference remains clickable"
        assert 0 <= switch["bounds"][1] <= height - switch["bounds"][3]
    if path.stem == "review-collapsed-720x560":
        assert keyed["page-body"]["rect"][0] < 55, "sidebar collapsed via keyboard"
    if path.stem == "settings-sidebar-wide-720x560":
        assert keyed["page-body"]["rect"][0] >= 280, "sidebar resized via dragging"
    print(f"{path.stem}: responsive bounds, reflow and controls OK")
