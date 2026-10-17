"""Prepare exact cached MPL crate sources for a release review, without downloading.

Does not publish or certify license compliance. Compare archive contents with the
installed source used by this checkout; reject changes instead of omitting them.
"""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import shutil
import tarfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cargo-metadata', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    metadata = json.loads(args.cargo_metadata.read_text())
    lock = tomllib.loads((ROOT / 'Cargo.lock').read_text())
    records = []
    for package in metadata['packages']:
        if 'MPL-2.0' not in (package.get('license') or '') or not package['source']:
            continue
        entry = next(p for p in lock['package'] if (p['name'], p['version'], p.get('source')) ==
                     (package['name'], package['version'], package['source']))
        source = Path(package['manifest_path']).parent
        archive = source.parent.parent.parent / 'cache' / source.parent.name / (source.name + '.crate')
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        if digest != entry['checksum']:
            raise ValueError(f'Archive checksum mismatch: {package["name"]}')
        checked = 0
        archived_paths = set()
        with tarfile.open(archive, 'r:gz') as tar:
            for member in tar.getmembers():
                path = PurePosixPath(member.name)
                if path.is_absolute() or '..' in path.parts or path.parts[0] != source.name:
                    raise ValueError(f'Unexpected archive path: {member.name}')
                if member.isdir():
                    continue
                if not member.isfile():
                    raise ValueError(f'Unsupported archive member: {member.name}')
                installed = source.joinpath(*path.parts[1:])
                archived_paths.add(installed)
                if installed.is_symlink() or tar.extractfile(member).read() != installed.read_bytes():
                    raise ValueError(f'Installed source differs: {package["name"]}/{path}')
                checked += 1
        extra = [str(p.relative_to(source)) for p in source.rglob('*')
                 if p.is_file() and p not in archived_paths and p.name != '.cargo-ok']
        if extra:
            raise ValueError(f'Unaccounted installed files: {package["name"]}: {extra}')
        records.append(dict(name=package['name'], version=package['version'], license=package['license'],
            archive=archive.name, sha256=digest, checked_files=checked,
            source_url=f'https://static.crates.io/crates/{package["name"]}/{archive.name}',
            local_archive=archive))
    if not records:
        raise ValueError('No MPL crates found; check metadata scope')
    notices = ROOT / 'licenses/native-supplements'
    notice_manifest = json.loads((notices / 'manifest.json').read_text())
    mpl = next(document for package in notice_manifest['packages']
               for document in package['documents'] if document['file'] == 'MPL-2.0.txt')
    license_bytes = (notices / mpl['file']).read_bytes()
    if hashlib.sha256(license_bytes).hexdigest() != mpl['sha256']:
        raise ValueError('MPL license text checksum mismatch')
    args.output.mkdir(parents=True, exist_ok=False)
    for record in records:
        shutil.copyfile(record.pop('local_archive'), args.output / record['archive'])
    manifest = dict(scope='Exact original MPL crate archives for review; not final distribution approval',
        cargo_lock_sha256=hashlib.sha256((ROOT / 'Cargo.lock').read_bytes()).hexdigest(),
        license_text=mpl, packages=records)
    (args.output / 'sources.json').write_text(json.dumps(manifest, indent=2) + '\n')
    (args.output / 'MPL-2.0.txt').write_bytes(license_bytes)
    instructions = [
        'Source code for Mozilla Public License 2.0 components\n',
        'The original source archives listed below are included in this directory.\n',
        'Each .crate file is a gzip-compressed tar archive and can be opened with a tar-compatible archiver.\n',
        'The MPL-2.0.txt file contains the license. Existing source notices are preserved in the archives.\n',
        'The archives matched the installed source files at preparation time.\n',
        'sources.json records checksums and upstream download addresses.\n',
    ]
    for record in records:
        instructions.append(f"\n{record['name']} {record['version']}\n"
                            f"Local source: {record['archive']}\nUpstream: {record['source_url']}\n")
    (args.output / 'SOURCE_CODE.txt').write_text(''.join(instructions))
    print(json.dumps(dict(packages=len(records), checked_files=sum(p['checked_files'] for p in records),
                         output=str(args.output))))


if __name__ == '__main__':
    main()
