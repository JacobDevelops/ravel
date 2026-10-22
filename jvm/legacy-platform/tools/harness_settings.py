"""Shared client options and controlled baked-scene placement."""
from pathlib import Path
import re
import sys

SETTINGS = Path(__file__).resolve().parents[1] / 'harness-client.properties'
SCENES = ('opaque', 'cutout-mipped', 'cutout', 'translucent', 'animated', 'still-water', 'flowing-water', 'lava',
          'water-stone', 'lava-cutout-plants')


def read_settings(path: Path) -> dict[str, str]:
    values: dict[str, str] = {}
    for line in path.read_text().splitlines():
        if not line.strip() or line.lstrip().startswith('#'):
            continue
        key, separator, value = line.partition('=')
        if not separator or not key or key in values or not value:
            raise ValueError('Malformed or duplicate harness setting: ' + line)
        values[key] = value
    return values


def validate_scene_settings(path: Path | None = None) -> dict[str, str]:
    values = read_settings(SETTINGS if path is None else path)
    distance = int(values['option.renderDistance'])
    if distance < 1 or not 0 <= int(values['scene.y']) <= 240:
        raise ValueError('Invalid harness render distance or scene height')
    occupied = set()
    for index, name in enumerate(SCENES):
        prefix = f'scene.{index}.'
        chunk = tuple(map(int, values[prefix + 'chunk'].split(',')))
        if len(chunk) != 2 or max(map(abs, chunk)) > distance:
            raise ValueError('Golden scene offset outside render distance: ' + name)
        if chunk == (0, 0) or chunk in occupied or values[prefix + 'name'] != name:
            raise ValueError('Invalid or overlapping golden scene: ' + name)
        occupied.add(chunk)
        blocks = values[prefix + 'blocks'].split(',')
        if len(blocks) != 2 or any(not re.fullmatch(r'minecraft:[a-z_]+ [0-9]+', block) for block in blocks):
            raise ValueError('Invalid golden scene blocks: ' + name)
    return values


def client_options(path: Path | None = None) -> dict[str, str]:
    return {key.removeprefix('option.'): value for key, value in validate_scene_settings(path).items()
            if key.startswith('option.')}


if __name__ == '__main__':
    Path(sys.argv[1]).write_text(''.join(f'{key}:{value}\n' for key, value in client_options().items()))
