"""Bounded, source-derived editor preview using the formal PSD pose renderer."""
import argparse
import sys

# Entry points must not modify resources inside a signed application bundle.
sys.dont_write_bytecode = True
import base64
import io
import json
from pathlib import Path

from PIL import Image
from psd_pose import load, probe, render


def preview(source, digest, pose):
    if pose['source_sha256'] != digest:
        raise ValueError('Pose belongs to another original')
    psd, entries, length = load(source, digest)
    original_probe = probe(psd, entries, digest, length)
    image = render(psd, entries, pose['selections'])
    image.thumbnail((1024, 1024), Image.Resampling.LANCZOS)
    encoded = io.BytesIO()
    image.save(encoded, format='PNG')
    data = encoded.getvalue()
    if len(data) > 5 * 1024 * 1024:
        raise ValueError('Preview exceeds output limit')
    return dict(probe=original_probe, width=image.width, height=image.height,
                png_base64=base64.b64encode(data).decode('ascii'))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--sha256', required=True)
    parser.add_argument('--pose', required=True)
    args = parser.parse_args()
    if len(args.pose.encode('utf-8')) > 65536:
        raise ValueError('Pose exceeds input limit')
    print(json.dumps(preview(args.source, args.sha256, json.loads(args.pose))))


if __name__ == '__main__':
    main()
