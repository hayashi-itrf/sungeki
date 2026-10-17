"""Stage a reviewed local inventory using macOS clonefile, never a hard link.

No download, model startup, configuration installation, or copying fallback.
The destination must be new; failed runs leave it for inspection.
"""
import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import sys


def digest(path):
    value = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            value.update(block)
    return value.hexdigest()


def clone(source, destination):
    if sys.platform != 'darwin':
        raise RuntimeError('copy-on-write staging currently requires macOS')
    before = source.stat()
    library = ctypes.CDLL('/usr/lib/libSystem.B.dylib', use_errno=True)
    operation = library.clonefile
    operation.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_int]
    operation.restype = ctypes.c_int
    if operation(os.fsencode(source), os.fsencode(destination), 0):
        raise OSError(ctypes.get_errno(), 'clonefile failed; no copying fallback')
    after = source.stat()
    target = destination.stat()
    if (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns, before.st_ctime_ns) != (
            after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns):
        raise ValueError('source changed during clone')
    if (after.st_dev, after.st_ino) == (target.st_dev, target.st_ino):
        raise ValueError('independent inode required')
    expected = digest(source)
    if digest(destination) != expected:
        raise ValueError('cloned content differs')
    return expected


def stage(inventory, destination):
    if not destination.is_absolute() or destination.resolve() != destination:
        raise ValueError('canonical absolute destination required')
    roles = inventory['roles']
    if inventory['format_version'] != 1 or set(roles) != {'model', 'codec', 'tokenizer', 'watermark'}:
        raise ValueError('unexpected inventory')
    entries = []
    watermark_revision = roles['watermark']['revision']
    if not re.fullmatch('[a-f0-9]{40}', watermark_revision):
        raise ValueError('invalid watermark revision')
    roots = {'model': 'originals/model', 'tokenizer': 'originals/model/tokenizer',
             'codec': 'originals/codec', 'watermark':
             f'originals/hf-hub/models--sony--silentcipher/snapshots/{watermark_revision}'}
    for role, group in roles.items():
        for item in group['files']:
            name = Path(item['logical_name'])
            source = Path(item['original_path'])
            if name.is_absolute() or not name.parts or '..' in name.parts:
                raise ValueError('invalid logical name')
            if not source.is_absolute() or source.resolve(strict=True) != source or not stat.S_ISREG(source.stat().st_mode):
                raise ValueError('canonical regular original required')
            if source.stat().st_size != item['bytes'] or destination == source or destination in source.parents:
                raise ValueError('original size or location mismatch')
            entries.append((role, source, destination / roots[role] / name))
    if len({target for _, _, target in entries}) != len(entries):
        raise ValueError('duplicate destination')
    destination.mkdir()  # Never reuse or overwrite an existing staging tree.
    records = []
    for role, source, target in entries:
        target.parent.mkdir(parents=True, exist_ok=True)
        sha = clone(source, target)
        records.append(dict(role=role, source=str(source), path=str(target), sha256=sha, bytes=target.stat().st_size))
    ref = destination / 'originals/hf-hub/models--sony--silentcipher/refs/main'
    ref.parent.mkdir(parents=True)
    ref.write_text(watermark_revision)
    report = dict(format_version=1, method='clonefile', model_loaded=False,
                  dependency_completeness_checked=False, files=records)
    (destination / 'staging-report.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('inventory', type=Path)
    parser.add_argument('destination', type=Path)
    args = parser.parse_args()
    result = stage(json.loads(args.inventory.read_text()), args.destination)
    print(f"Staged and verified {len(result['files'])} originals; no model loaded")
