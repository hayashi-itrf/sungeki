"""Consume Rust's display-normalized, hash-verified original video frame."""
from PIL import Image


def decoded_image(data, metadata):
    if not isinstance(metadata, dict) or set(metadata) != {'width', 'height', 'pts'}:
        raise ValueError('Invalid decoded video metadata')
    width, height, pts = (metadata[key] for key in ('width', 'height', 'pts'))
    if (type(width) is not int or type(height) is not int or type(pts) is not int
            or not 1 <= width <= 16384 or not 1 <= height <= 16384
            or not -(2**63) <= pts < 2**63 or width * height * 4 > 64 * 1024 * 1024
            or len(data) != width * height * 4):
        raise ValueError('Invalid decoded video dimensions or bytes')
    return Image.frombytes('RGBA', (width, height), data)
