"""Verify play setup sizing, shared board layout, and the game session dock."""
import json
from pathlib import Path

artifacts = Path(__file__).resolve().parent.parent / "artifacts"
for scene in ("play", "play-game", "play-many"):
    for size in ("720x560", "1440x1080"):
        name = f"{scene}-{size}"
        tree = json.loads((artifacts / f"{name}.json").read_text())
        assert not tree["warnings"], (name, tree["warnings"])
        keyed = {node["key"]: node for node in tree["elements"] if node.get("key")}
        width, height = tree["viewport"]
        if scene == "play":
            assert keyed["play-setup-card"]["rect"][2] <= 480.5, name
            keys = ("play-setup-card", "play-color", "play-start")
        else:
            assert sum(key.startswith("square-") for key in keyed) == 64, name
            assert sum(key.startswith("play-session-") for key in keyed) == (15 if scene == "play-many" else 1), name
            assert "board-workspace" in keyed and "move-navigation" in keyed, name
            keys = ("chessboard", "play-add", "play-resign")
            if size == "1440x1080":
                assert "review-inspector" in keyed, name
                assert all(f"move-{ply}" in keyed for ply in range(1, 9)), name
                board_x, _, board_width, _ = keyed["chessboard"]["rect"]
                assert keyed["review-inspector"]["rect"][0] >= board_x + board_width, name
        for key in keys:
            x, y, w, h = keyed[key]["rect"]
            assert 0 <= x and x + w <= width + 0.5, (name, key, "horizontal bounds")
            assert 0 <= y and y + h <= height + 0.5, (name, key, "vertical bounds")
        print(f"{name}: setup, board, move list and session bounds OK")
