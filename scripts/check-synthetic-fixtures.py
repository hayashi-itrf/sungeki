"""Verify that the three small test media contain only the documented synthetic data.
Requires the image worker's Pillow dependency. Does not change existing fixtures.
"""
from pathlib import Path
import struct
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]


def main():
    images = [
        ('tests/fixtures/build-source.png', (32, 32), (180, 190, 200, 255)),
        ('crates/review/tests/data/transparent.png', (1, 1), (0, 0, 0, 0)),
    ]
    for relative, size, color in images:
        with Image.open(ROOT / relative) as image:
            assert image.size == size, relative
            assert image.convert('RGBA').getcolors() == [(size[0] * size[1], color)], relative
    # IEEE float WAV, mono, 48000 Hz, five mathematical samples; no recorded voice.
    samples = struct.pack('<5f', -1.25, -.5, 0, .5, 1.25)
    fmt = struct.pack('<HHIIHHH', 3, 1, 48000, 192000, 4, 32, 0)
    chunks = b'fmt ' + struct.pack('<I', len(fmt)) + fmt
    chunks += b'fact' + struct.pack('<II', 4, 5)
    chunks += b'data' + struct.pack('<I', len(samples)) + samples
    expected = b'RIFF' + struct.pack('<I', len(chunks) + 4) + b'WAVE' + chunks
    assert (ROOT / 'tests/fixtures/irodori-float.wav').read_bytes() == expected
    print('Two synthetic images and five-sample float WAV verified')


if __name__ == '__main__':
    main()
