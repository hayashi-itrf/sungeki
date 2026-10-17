"""Bounded owned bytes for image/PSD workers; never trust a mutable path alone."""
import hashlib
import os
from pathlib import Path
import re
import stat

MAX_BYTES = 64 * 1024 * 1024

def verified_bytes(path, expected_hash):
    if not re.fullmatch('[0-9a-f]{64}', expected_hash):
        raise ValueError('Invalid content hash')
    path = Path(path)
    if not path.is_absolute():
        raise ValueError('Captured originals require an absolute path')
    with os.fdopen(os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK), 'rb') as source:
        before = os.fstat(source.fileno())
        if not stat.S_ISREG(before.st_mode) or not 0 < before.st_size <= MAX_BYTES:
            raise ValueError('Invalid original file or size')
        data = source.read(MAX_BYTES + 1)
        after = os.fstat(source.fileno())
        identity = lambda info: (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns, info.st_ctime_ns)
        if identity(before) != identity(after) or len(data) != before.st_size:
            raise ValueError('Original changed during read')
    if hashlib.sha256(data).hexdigest() != expected_hash:
        raise ValueError('Original hash mismatch')
    return data
