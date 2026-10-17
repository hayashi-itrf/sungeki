"""Inspect a hash-pinned PSD and export a full-resolution static pose.
This is source-derived artwork, never the PSD's embedded preview/thumbnail.
Editable PSD Project assets and automatic mouth timing are not connected yet.
"""
import argparse
import hashlib
import importlib.metadata
import io
import json
import os
from pathlib import Path
import sys

# Entry points must not modify resources inside a signed application bundle.
sys.dont_write_bytecode = True

from psd_tools import PSDImage
from psd_tools.constants import ColorMode
from original_bytes import verified_bytes


def load(path, sha256):
    source = verified_bytes(path, sha256)
    psd = PSDImage.open(io.BytesIO(source))
    if psd.depth != 8 or psd.color_mode != ColorMode.RGB:
        raise ValueError('Only 8-bit RGB PSD is supported')
    if not 1 <= psd.width <= 8192 or not 1 <= psd.height <= 8192 or psd.width * psd.height > 32 * 1024 * 1024:
        raise ValueError('Unsupported PSD dimensions')
    entries = {}

    def walk(group, prefix='', depth=0):
        if depth > 32:
            raise ValueError('PSD group nesting is too deep')
        for index, layer in enumerate(group):
            key = f'{prefix}/{index}'
            entries[key] = layer
            if len(entries) > 4096 or len(layer.name.encode('utf-8')) > 4096:
                raise ValueError('PSD layer catalog is too large')
            if layer.width * layer.height > 32 * 1024 * 1024:
                raise ValueError('PSD layer is too large')
            if layer.is_group():
                walk(layer, key, depth+1)
    walk(psd)
    pixels = sum(layer.width * layer.height for layer in entries.values() if not layer.is_group())
    if pixels > 256 * 1024 * 1024:
        raise ValueError('PSD aggregate layer pixels exceed the limit')
    return psd, entries, len(source)


def choose(entries, selections):
    if not isinstance(selections, list) or len(selections) > 256:
        raise ValueError('Invalid pose selections')
    seen = set()
    # Parents are selected before children; input order does not alter a pose.
    for selection in sorted(selections, key=lambda item: str(item.get('group', '')).count('/')):
        if set(selection) != {'group', 'option'}:
            raise ValueError('Unknown selection fields')
        group_key, option_key = selection['group'], selection['option']
        if not isinstance(group_key, str) or not isinstance(option_key, str) or group_key in seen:
            raise ValueError('Invalid or duplicate selection group')
        seen.add(group_key)
        group, option = entries.get(group_key), entries.get(option_key)
        if group is None or not group.is_group() or option is None or option_key.rsplit('/', 1)[0] != group_key:
            raise ValueError('Pose option is not a direct member of the group')
        if not group.is_visible():
            raise ValueError('Pose group is hidden by its parent or saved visibility')
        if not option.name.startswith('*'):
            raise ValueError('Pose options must use the PSDTool * prefix')
        for child in group:
            if child.name.startswith('*'):
                child.visible = child is option


def probe(psd, entries, sha256, byte_length):
    return dict(sha256=sha256, width=psd.width, height=psd.height, byte_length=byte_length,
                layer_layout_version=1, layers=[dict(path=key, name=layer.name,
                kind=layer.kind, visible=layer.visible) for key, layer in entries.items()])


def inspect(psd, entries, sha256, byte_length):
    return dict(probe=probe(psd, entries, sha256, byte_length),
                source_sha256=sha256, source_byte_length=byte_length,
                width=psd.width, height=psd.height, depth=psd.depth,
                color_mode='rgb', pose_layout_version=1,
                psd_tools=importlib.metadata.version('psd-tools'),
                pillow=importlib.metadata.version('Pillow'),
                layers=[dict(path=key, name=layer.name, kind=layer.kind,
                             visible=layer.visible, effective_visible=layer.is_visible(),
                             bounds=list(layer.bbox)) for key, layer in entries.items()])


def render(psd, entries, selections):
    choose(entries, selections)
    for layer in entries.values():
        if layer.is_visible() and layer.kind not in ('pixel', 'group'):
            raise ValueError(f'Unsupported visible layer kind: {layer.kind}')
    result = psd.composite(viewport=(0, 0, psd.width, psd.height),
                           force=True, ignore_preview=True, alpha=0.0, apply_icc=True)
    if result is None or result.size != psd.size:
        raise ValueError('PSD did not produce its full canvas')
    result = result.convert('RGBA')
    if result.getchannel('A').getbbox() is None:
        raise ValueError('Pose is completely transparent')
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--sha256', required=True)
    parser.add_argument('--pose', type=Path)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    selections = []
    if args.pose:
        if args.pose.stat().st_size > 65536:
            raise ValueError('Pose request is too large')
        pose = json.loads(args.pose.read_text())
        if pose['source_sha256'] != args.sha256:
            raise ValueError('Pose preset belongs to a different PSD')
        selections = pose['selections']
    psd, entries, byte_length = load(args.source, args.sha256)
    if args.output:
        result = render(psd, entries, selections)
        encoded = io.BytesIO()
        result.save(encoded, format='PNG')
        data = encoded.getvalue()
        with args.output.open('xb') as target:
            target.write(data)
            target.flush()
            os.fsync(target.fileno())
    elif selections:
        choose(entries, selections)
    report = inspect(psd, entries, args.sha256, byte_length)
    report['selections'] = selections
    if args.output:
        report['output'] = dict(path=str(args.output), sha256=hashlib.sha256(data).hexdigest(),
                                byte_length=len(data), alpha_bounds=list(result.getchannel('A').getbbox()))
    print(json.dumps(report, ensure_ascii=False))


if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        print(json.dumps(dict(error=f'{type(error).__name__}: {error}')), file=sys.stderr)
        sys.exit(1)
