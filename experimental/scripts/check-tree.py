"""Verify exported layouts and observable import/review/settings interactions."""
import json
from pathlib import Path

ARTIFACTS = Path(__file__).resolve().parent.parent / "artifacts"
SCENES = (
    "home", "configure", "manual", "review", "opening", "summary", "history",
    "settings", "settings-light", "settings-analysis", "settings-storage",
    "interaction", "settings-interaction", "import", "compact",
)
BOARD_SCENES = {"review", "opening", "summary", "interaction", "import", "compact"}
for scene in SCENES:
    tree = json.loads((ARTIFACTS / f"{scene}.json").read_text())
    width, height = tree["viewport"]
    elements = tree["elements"]
    keyed = {e["key"]: e for e in elements if "key" in e}
    texts = {e.get("text") for e in elements}
    assert not tree["warnings"], (scene, tree["warnings"])
    for element in elements:
        if element["opacity"] <= 0.01:
            continue
        x, y, w, h = element["rect"]
        assert x >= -0.5 and x + w <= width + 0.5, (scene, element["path"], "horizontal overflow", element["rect"])
        scroll_keys = {"review-scroll", "review-panel-scroll", "board-scroll", "review-settings-scroll", "move-list", "scroll"}
        if "page-content" in keyed:
            scroll_keys.add("page-body")
        scroll_paths = [e["path"] + "/" for e in elements if e.get("key") in scroll_keys]
        scrolls = any(element["path"].startswith(path) for path in scroll_paths)
        assert scrolls or (y >= -0.5 and y + h <= height + 0.5), (
            scene, element["path"], "vertical overflow without a scroll viewport", element["rect"]
        )
    if scene in BOARD_SCENES:
        assert sum(key.startswith("square-") for key in keyed) == 64
        for key in ("first", "previous", "next", "last", "flip"):
            assert keyed[key]["role"] == "button" and keyed[key]["focusable"]
            assert all(size > 0 for size in keyed[key]["rect"][2:]), (scene, key)
    else:
        assert "chessboard" not in keyed, scene
    if scene == "home":
        assert "pgn-editor" in keyed and "continue" in keyed
    if scene == "manual":
        assert keyed["depth"]["role"] == "slider"
    if scene == "history":
        assert "open-demo" in keyed
    if scene.startswith("settings"):
        assert "settings-tabs" in keyed and "settings-close" in keyed
    if scene in {"settings", "settings-light"}:
        assert keyed["sound"]["role"] == "switch"
        assert keyed["volume"]["role"] == "slider"
    if scene == "settings-analysis":
        assert keyed["lines-trigger"]["role"] == "combobox"
        assert keyed["threads"]["role"] == "slider"
    if scene == "settings-interaction":
        assert "5" in texts, "script chose five candidate lines"
        assert "lines-content" not in keyed, "selection closes the popup"
        assert keyed["live-engine"]["role"] == "switch"
    if scene == "interaction":
        assert "review-summary" in keyed and "move-list" not in keyed
        assert keyed["square-a1"]["rect"][0] > keyed["square-h1"]["rect"][0], "board flipped"
        assert "16 / 16" in texts, "last move selected"
    if scene == "import":
        assert "04 / 4" in texts, "typed PGN imported and End reached its final position"
        assert "move-5" not in keyed, "uses the typed game, not the bundled demo"
    print(f"{scene}: {len(elements)} elements, bounds and interactions OK")
