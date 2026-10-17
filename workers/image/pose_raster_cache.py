"""Bounded decoded/scaled PSD PNGs within one worker process.
The caller verifies original and PNG bytes on every lookup. Keys also hash the
supplied bytes, so a changed payload or catalog cannot reuse an earlier raster.
"""
from collections import OrderedDict
import hashlib
import io
from PIL import Image


def checked_size(size):
    if (not isinstance(size, tuple) or len(size) != 2
            or any(type(v) is not int or not 1 <= v <= 32768 for v in size)
            or size[0]*size[1] > 32*1024*1024):
        raise ValueError('Transformed image is too large or invalid')
    return size


class PoseRasterCache:
    def __init__(self, max_bytes=64*1024*1024, max_entries=32):
        self.max_bytes, self.max_entries = max_bytes, max_entries
        self.entries = OrderedDict()
        self.bytes = 0

    def decode(self, data, expected, target):
        expected, target = checked_size(expected), checked_size(target)
        key = (hashlib.sha256(data).digest(), expected, target)
        cached = self.entries.get(key)
        if cached is not None:
            self.entries.move_to_end(key)
            return cached.copy()
        with Image.open(io.BytesIO(data)) as source:
            if source.format != 'PNG' or source.mode != 'RGBA' or source.size != expected:
                raise ValueError('Pose cache dimensions or format mismatch')
            result = source.resize(target, Image.Resampling.LANCZOS)
        size = target[0]*target[1]*4
        if size <= self.max_bytes and self.max_entries > 0:
            while self.entries and (self.bytes+size > self.max_bytes or len(self.entries) >= self.max_entries):
                _, old = self.entries.popitem(last=False)
                self.bytes -= old.width*old.height*4
                old.close()
            self.entries[key] = result.copy()
            self.bytes += size
        return result
