"""Bounded multi-frame transport around the same authoritative rasterizer.
Partial files are private staging only. Success is emitted after every frame.
"""
import argparse
import json
import os
from pathlib import Path
import re
import stat
import sys

# Entry points must not modify resources inside a signed application bundle.
sys.dont_write_bytecode = True
from render_frame import render, FRAME_PNG_COMPRESS_LEVEL


def read_json(path, limit):
    path = Path(path)
    if not path.is_absolute():
        raise ValueError('Batch input requires an absolute path')
    with os.fdopen(os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK), 'rb') as stream:
        info = os.fstat(stream.fileno())
        if not stat.S_ISREG(info.st_mode) or not 0 < info.st_size <= limit:
            raise ValueError('Batch input type or size is invalid')
        data = stream.read(limit + 1)
        if len(data) != info.st_size:
            raise ValueError('Batch input size changed')
    return json.loads(data)


def run(manifest, directory, font, font_hash, pose_cache=None):
    if not directory.is_absolute() or not stat.S_ISDIR(directory.lstat().st_mode):
        raise ValueError('Batch output requires a private directory')
    if not isinstance(manifest, dict) or set(manifest) != {'version', 'frames'} or type(manifest['version']) is not int or manifest['version'] != 1:
        raise ValueError('Unsupported batch version')
    frames = manifest['frames']
    if not isinstance(frames, list) or not 1 <= len(frames) <= 32:
        raise ValueError('Batch must contain 1 to 32 frames')
    names = set()
    for item in frames:
        if not isinstance(item, dict) or set(item) != {'input', 'resources', 'output'} or not isinstance(item['output'], str):
            raise ValueError('Invalid batch frame')
        name = item['output']
        if not re.fullmatch(r'frame-[0-9]+\.png', name) or name in names:
            raise ValueError('Invalid or duplicate output name')
        names.add(name)
        if (directory/name).exists() or (directory/name).is_symlink():
            raise ValueError('Batch output already exists')
    reports = []
    for item in frames:
        packet = read_json(item['input'], 16 * 1024 * 1024)
        resources = read_json(item['resources'], 32 * 1024 * 1024)
        image, report = render(packet, resources, font, font_hash, pose_cache)
        try:
            with (directory/item['output']).open('xb') as stream:
                image.save(stream, format='PNG', compress_level=FRAME_PNG_COMPRESS_LEVEL)
                stream.flush()
                # These are disposable job-local frames, not published output.
                # Closing flushes bytes for the encoder; interrupted jobs never
                # resume from these files. Durable publication belongs to host.
        finally:
            image.close()
        reports.append(dict(output=item['output'], report=report))
    return dict(version=1, frames=reports)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--manifest', type=Path, required=True)
    parser.add_argument('--directory', type=Path, required=True)
    parser.add_argument('--font', type=Path, required=True)
    parser.add_argument('--font-sha256', required=True)
    parser.add_argument('--pose-cache', type=Path)
    args = parser.parse_args()
    result = run(read_json(args.manifest, 1024 * 1024), args.directory, args.font,
                 args.font_sha256, args.pose_cache)
    print(json.dumps(result, ensure_ascii=False))


if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        print(json.dumps(dict(error=f'{type(error).__name__}: {error}')), file=sys.stderr)
        sys.exit(1)
