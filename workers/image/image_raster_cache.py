"""Bounded worker-local raster reuse; callers still verify source bytes every frame."""
from collections import OrderedDict
import hashlib
import io
from PIL import Image, ImageOps
from pose_raster_cache import checked_size


class ImageRasterCache:
    def __init__(self, max_bytes=64*1024*1024, max_entries=32):
        self.max_bytes, self.max_entries = max_bytes, max_entries
        self.entries = OrderedDict()
        self.bytes = 0

    def decode(self, data, target):
        target = checked_size(target)
        key = (hashlib.sha256(data).digest(), target)
        cached = self.entries.get(key)
        if cached is not None:
            self.entries.move_to_end(key)
            return cached.copy()
        with Image.open(io.BytesIO(data)) as source:
            if source.format not in ('PNG', 'JPEG', 'WEBP') or getattr(source, 'n_frames', 1) != 1:
                raise ValueError('Unsupported image format or animation')
            if source.width * source.height > 32*1024*1024:
                raise ValueError('Image is too large')
            with ImageOps.exif_transpose(source) as oriented:
                with oriented.convert('RGBA') as rgba:
                    result = rgba.resize(target, Image.Resampling.LANCZOS)
        size = target[0]*target[1]*4
        if size <= self.max_bytes and self.max_entries > 0:
            while self.entries and (self.bytes+size > self.max_bytes or len(self.entries) >= self.max_entries):
                _, old = self.entries.popitem(last=False)
                self.bytes -= old.width*old.height*4
                old.close()
            self.entries[key] = result.copy()
            self.bytes += size
        return result


CACHE = ImageRasterCache()
