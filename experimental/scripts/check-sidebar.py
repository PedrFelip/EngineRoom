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
        assert not any(e.get('text') == 'ESPAÇO DE TRABALHO' for e in tree['elements']), path.name
        nodes = {n.get('key'): n for n in walk(tree['accessibility']) if n.get('key')}
        if scene == 'demo-collapsed':
            assert 'sidebar-list' not in nodes or nodes['sidebar-list']['bounds'][2] == 0, path.name
            assert 'sidebar-logo' not in nodes, path.name
            assert 'click' in nodes['sidebar-toggle']['actions'], path.name
            continue
        assert nodes['sidebar-list']['name'] == 'Navegação principal'
        logo = nodes['sidebar-logo']
        assert (logo['role'], logo['name']) == ('image', 'EngineRoom'), path.name
        x, _, width, height = logo['bounds']
        assert width > 0 and height > 0 and x + width <= nodes['sidebar-toggle']['bounds'][0], path.name
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

# Named folders and the move menu are presentation-only sidebar affordances.
for scene in ('sessions-colors', 'sessions-grouped', 'sessions-grouped-light', 'sessions-menu'):
    path = artifacts / f'{scene}.json'
    tree = json.loads(path.read_text())
    assert not tree['warnings'], (path.name, tree['warnings'])
    all_nodes = list(walk(tree['accessibility']))
    assert not any(node.get('name') == 'SEM PASTA' for node in all_nodes), path.name
    nodes = {n.get('key'): n for n in all_nodes if n.get('key')}
    if scene != 'sessions-menu':
        assert 'session-1' in nodes and 'close-session-1' in nodes, path.name
        assert 'context-menu' in nodes['session-1'].get('actions', []), path.name
        assert 'Pasta: ' in nodes['session-1'].get('name', ''), path.name
    if scene in ('sessions-grouped', 'sessions-grouped-light'):
        groups = ('session-folder-fixture-blue', 'session-folder-fixture-amber', 'session-unfiled') if scene == 'sessions-grouped' else ('session-folder-fixture-green', 'session-folder-fixture-violet')
        for key in groups:
            assert key in nodes, (path.name, key)
        names = ('Aberturas', 'Torneios') if scene == 'sessions-grouped' else ('Finais', 'Estudos')
        for name in names:
            assert any(n.get('name') == name for n in all_nodes), (path.name, name)
    if scene == 'sessions-menu':
        element_keys = {e.get('key') for e in tree['elements']}
        assert 'session-1' in element_keys and 'close-session-1' in element_keys, path.name
        assert 'session-menu-1-menu' in element_keys, path.name
        menu_labels = ('Aberturas', 'Sem pasta', 'Nova pasta…', 'Fechar sessão')
        for name in menu_labels:
            assert any(n.get('name') == name for n in all_nodes), (path.name, name)
        label_x = {
            e['text']: e['rect'][0] for e in tree['elements']
            if e.get('text') in menu_labels and e['path'].endswith('/content/label')
        }
        assert len(label_x) == len(menu_labels) and len(set(label_x.values())) == 1, label_x
        checked = [n for n in all_nodes if 'checked' in n.get('states', [])]
        assert any(n.get('name') == 'Aberturas' for n in checked), path.name
    print(f'{scene}: presentation semantics OK')

editor = json.loads((artifacts / 'folder-editor.json').read_text())
assert not editor['warnings'], editor['warnings']
keys = {node.get('key') for node in walk(editor['accessibility'])}
assert 'folder-name' in keys and 'save-folder' in keys
print('folder-editor: semantics OK')

folder_menu = json.loads((artifacts / 'folder-menu.json').read_text())
assert not folder_menu['warnings'], folder_menu['warnings']
menu_nodes = list(walk(folder_menu['accessibility']))
for name in ('Editar pasta…', 'Excluir pasta'):
    assert any(node.get('name') == name for node in menu_nodes), name
print('folder-menu: semantics OK')

drag = json.loads((artifacts / 'sessions-drag.json').read_text())
assert not drag['warnings'], drag['warnings']
assert any(element.get('key') == 'session-drag-preview' for element in drag['elements'])
assert any(node.get('name') == 'Solte aqui para deixar fora de pastas' for node in walk(drag['accessibility']))
print('sessions-drag: destination and preview OK')
