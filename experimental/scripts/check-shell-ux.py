"""Check collapsed navigation accessibility and bounds in captured headless trees."""
import json
from pathlib import Path

artifacts = Path(__file__).resolve().parent.parent / "artifacts" / "final-ux"


def walk(node):
    yield node
    for child in node.get("children", []):
        yield from walk(child)


for width, height in ((1920, 1080), (900, 700), (720, 560)):
    path = artifacts / f"demo-collapsed-{width}x{height}.json"
    tree = json.loads(path.read_text())
    assert not tree["warnings"], (path, tree["warnings"])
    nodes = {node.get("key"): node for node in walk(tree["accessibility"])}
    for key in ("sidebar-toggle", "rail-home", "rail-history", "rail-session", "rail-settings"):
        node = nodes[key]
        assert node.get("name") and {"click", "focus"} <= set(node.get("actions", [])), (path, key)
        x, y, w, h = node["bounds"]
        assert 0 <= x and 0 <= y and x + w <= width and y + h <= height, (path, key)
    assert "checked" in nodes["rail-session"].get("states", []), path
    for key in ("rail-home", "rail-history", "rail-settings"):
        assert "checked" not in nodes[key].get("states", []), (path, key)
    print(f"{path.name}: navigation accessible and in bounds")
