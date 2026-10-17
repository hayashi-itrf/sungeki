"""Encode display-normalized original video pixels as a bounded editor PNG."""
import argparse
import sys

# Entry points must not modify resources inside a signed application bundle.
sys.dont_write_bytecode = True
import base64
import io
import json
from pathlib import Path
from PIL import Image
from original_bytes import verified_bytes
from video_frame import decoded_image


def preview(resource):
    image = (decoded_image(verified_bytes(resource['path'], resource['sha256']), resource['decoded_video'])
             if resource else Image.new('RGBA', (1, 1), (0, 0, 0, 0)))
    image.thumbnail((1024, 1024), Image.Resampling.LANCZOS)
    output = io.BytesIO()
    image.save(output, format='PNG')
    data = output.getvalue()
    if len(data) > 5 * 1024 * 1024:
        raise ValueError('Video preview exceeds output limit')
    return dict(width=image.width, height=image.height, png_base64=base64.b64encode(data).decode('ascii'))


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resource', type=Path)
    args = parser.parse_args()
    raw = args.resource.read_bytes()
    if len(raw) > 65536:
        raise ValueError('Video preview resource exceeds input limit')
    print(json.dumps(preview(json.loads(raw))))
