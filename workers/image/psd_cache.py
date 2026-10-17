"""Job-local, bounded full-resolution PSD pose cache; never a preview proxy.
The caller creates a private empty directory for each render. Captured originals
are verified on every lookup. Incomplete/corrupt entries fail the job.
"""
import hashlib
import io
import json
import os
from pathlib import Path
import stat

from PIL import Image
from original_bytes import verified_bytes, MAX_BYTES
from pose_raster_cache import PoseRasterCache, checked_size

_RASTERS = PoseRasterCache()

MAX_ENTRIES = 64
MAX_CACHE_BYTES = 256 * 1024 * 1024


def write_new(path, data):
    with os.fdopen(os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), 'wb') as stream:
        stream.write(data)
        stream.flush()
        os.fsync(stream.fileno())


def pose_image(resource, pose, directory=None, target=None):
    if target is not None:
        target = checked_size(target)
    if pose['source_sha256'] != resource['sha256']:
        raise ValueError('PSD pose belongs to another original')
    # A cache hit never hides a missing or changed captured original.
    verified_bytes(resource['path'], resource['sha256'])
    key = hashlib.sha256(json.dumps(['psd-full-pose-v1', resource['sha256'],
        resource['probe'], pose], sort_keys=True, separators=(',', ':')).encode()).hexdigest()
    png = record = None
    if directory is not None:
        directory = Path(directory)
        if not directory.is_absolute() or not stat.S_ISDIR(directory.lstat().st_mode):
            raise ValueError('Pose cache requires a real absolute directory')
        png, record = directory / f'{key}.png', directory / f'{key}.json'
        if record.exists() or record.is_symlink():
            with os.fdopen(os.open(record, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK), 'rb') as stream:
                info = os.fstat(stream.fileno())
                if not stat.S_ISREG(info.st_mode) or info.st_size > 4096:
                    raise ValueError('Invalid pose cache record')
                entry = json.loads(stream.read(4097))
            if entry['key'] != key:
                raise ValueError('Pose cache key mismatch')
            data = verified_bytes(png, entry['sha256'])
            expected = resource['probe']['info']
            if target is not None:
                return _RASTERS.decode(data, (expected['width'], expected['height']), target)
            with Image.open(io.BytesIO(data)) as image:
                expected = resource['probe']['info']
                if image.format != 'PNG' or image.mode != 'RGBA' or image.size != (expected['width'], expected['height']):
                    raise ValueError('Pose cache dimensions or format mismatch')
                return image.copy()
        if png.exists() or png.is_symlink():
            raise ValueError('Incomplete pose cache entry')
    # A verified cache hit needs no PSD parser or NumPy initialization.
    import psd_pose
    psd, entries, size = psd_pose.load(resource['path'], resource['sha256'])
    expected = resource['probe']
    if expected['type'] != 'psd' or expected['info'] != psd_pose.probe(psd, entries, resource['sha256'], size):
        raise ValueError('PSD catalog differs from the registered original')
    image = psd_pose.render(psd, entries, pose['selections'])
    if directory is not None:
        files = list(directory.glob('*.png'))
        if len(files) < MAX_ENTRIES:
            encoded = io.BytesIO()
            image.save(encoded, format='PNG')
            data = encoded.getvalue()
            if len(data) <= MAX_BYTES and sum(p.stat().st_size for p in files) + len(data) <= MAX_CACHE_BYTES:
                write_new(png, data)
                write_new(record, json.dumps(dict(key=key, sha256=hashlib.sha256(data).hexdigest())).encode())
    if target is None:
        return image
    try:
        return image.resize(target, Image.Resampling.LANCZOS)
    finally:
        image.close()
