"""Check native sidebar semantics and selection across scenes and window sizes."""
import json
from pathlib import Path

artifacts = Path(__file__).resolve().parent.parent / 'artifacts'

def walk(node):
    yield node
    for child in node.get('children', []):
        yield from walk(child)

for scene, selected in [('home-light', 'home-nav'), ('configure', 'home-nav'),
                        ('history', 'history-nav'), ('review-light', 'session-1'),
                        ('settings', 'settings-nav'), ('settings-light', 'settings-nav'), ('demo-collapsed', None)]:
    for size in ('720x560', '1440x1080'):
        path = artifacts / f'sidebar-{scene}-{size}.json'
        tree = json.loads(path.read_text())
        assert not tree['warnings'], (path.name, tree['warnings'])
        nodes = {n.get('key'): n for n in walk(tree['accessibility']) if n.get('key')}
        if scene == 'demo-collapsed':
            assert 'sidebar-list' not in nodes or nodes['sidebar-list']['bounds'][2] == 0, path.name
            continue
        assert nodes['sidebar-list']['name'] == 'Navegação principal'
        for key in ('home-nav', 'history-nav', 'session-1', 'settings-nav'):
            node = nodes[key]
            assert 'click' in node.get('actions', []) and 'focus' in node.get('actions', [])
            assert node['bounds'][3] >= 36
        close = nodes['close-session-1']
        assert 'click' in close.get('actions', []) and 'focus' in close.get('actions', [])
        assert nodes['sidebar-grip']['role'] == 'splitter'
        if selected:
            assert 'selected' in nodes[selected].get('states', []), path.name
        print(f'{path.stem}: sidebar semantics OK')
