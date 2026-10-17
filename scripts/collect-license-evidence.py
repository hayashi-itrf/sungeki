"""Collect installed dependency notices for review; does not certify redistribution.

Cargo input: cargo metadata --locked --offline --filter-platform <target> --format-version 1
No downloads, source changes, publishing, or automatic license selection.
"""
import argparse
import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
NOTICE = re.compile(r'^(licen[cs]e|copying|notice|copyright|authors)(?:[._-].*)?$', re.I)


def notice_files(directory, declared=None):
    candidates = set()
    for path in directory.rglob('*'):
        relative = path.relative_to(directory)
        if any(part in ('node_modules', '.git', 'target', '__pycache__') for part in relative.parts):
            continue
        if path.is_file() and (NOTICE.match(path.name) or any(
                part.lower() in ('licenses', 'licences') for part in relative.parts[:-1])):
            candidates.add(path)
    if declared:
        candidates.add(directory / declared)
    result, missing = [], []
    for path in sorted(candidates):
        if not path.is_file() or not path.resolve().is_relative_to(directory.resolve()):
            missing.append(str(path.relative_to(directory)))
            continue
        raw = path.read_bytes()
        try:
            text = raw.decode('utf-8')
        except UnicodeDecodeError:
            missing.append(str(path.relative_to(directory)))
            continue
        if not text.strip():
            missing.append(str(path.relative_to(directory)))
            continue
        result.append(dict(path=str(path.relative_to(directory)),
                           sha256=hashlib.sha256(raw).hexdigest(), text=text))
    return result, missing


def collect(cargo):
    packages = []
    for package in cargo['packages']:
        if not package['source']:
            continue
        directory = Path(package['manifest_path']).parent
        files, missing = notice_files(directory, package.get('license_file'))
        packages.append(dict(ecosystem='cargo', name=package['name'], version=package['version'],
            source=package['source'], repository=package.get('repository'),
            declared_license=package['license'], files=files, unreadable=missing,
            status='documents_collected' if files and not missing else 'needs_review'))
    lock = json.loads((ROOT / 'package-lock.json').read_text())
    for location, package in lock['packages'].items():
        if 'node_modules/' not in location or package.get('link'):
            continue
        directory = ROOT / location
        manifest = directory / 'package.json'
        installed = json.loads(manifest.read_text()) if manifest.is_file() else {}
        matches = installed.get('version') == package.get('version')
        files, missing = notice_files(directory) if matches else ([], ['installed version missing or mismatched'])
        packages.append(dict(ecosystem='npm', name=installed.get('name', location.split('node_modules/')[-1]),
            version=package.get('version'), declared_license=package.get('license'), files=files,
            integrity=package.get('integrity'), optional=package.get('optional', False),
            platforms=package.get('os'), architectures=package.get('cpu'),
            unreadable=missing, status='documents_collected' if files and not missing else 'needs_review'))
    return sorted(packages, key=lambda p: (p['ecosystem'], p['name'], p['version'] or ''))


def supplement(packages, cargo, directory):
    manifest = json.loads((directory / 'manifest.json').read_text())
    for item in manifest['packages']:
        key = (item['name'], item['version'])
        matches = [p for p in packages if p['ecosystem'] == 'cargo' and (p['name'], p['version']) == key]
        if not matches:
            continue
        package = matches[0]
        source = next(p for p in cargo['packages'] if (p['name'], p['version']) == key)
        installed = Path(source['manifest_path']).parent
        vcs = json.loads((installed / '.cargo_vcs_info.json').read_text())
        original = (installed / 'Cargo.toml.orig').read_bytes()
        if (vcs['git']['sha1'] != item['commit'] or source['license'] != item['declared_license']
                or hashlib.sha256(original).hexdigest() != item['manifest_sha256']):
            raise ValueError(f'Supplement source mismatch: {key}')
        for document in item['documents']:
            path = directory / document['file']
            if not path.resolve().is_relative_to(directory.resolve()):
                raise ValueError('Supplement path outside directory')
            raw = path.read_bytes()
            if hashlib.sha256(raw).hexdigest() != document['sha256']:
                raise ValueError(f'Supplement checksum mismatch: {key}')
            package['files'].append(dict(path='upstream/' + document['file'],
                sha256=document['sha256'], text=raw.decode('utf-8'), source_url=document['url']))
        package['supplement_commit'] = item['commit']
        if item.get('selected_license'):
            package['selected_license'] = item['selected_license']
        if item.get('remaining_review'):
            package['remaining_review'] = item['remaining_review']
        if package['files'] and not package['unreadable']:
            package['status'] = 'documents_collected'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cargo-metadata', type=Path, required=True)
    parser.add_argument('--target', required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--cargo-tree', type=Path,
        help='Restrict to Cargo packages in cargo tree --prefix none --format {p}; excludes npm')
    parser.add_argument('--cargo-tree-edges', choices=['normal', 'normal,build'], default='normal',
        help='Edge selection used for the supplied Cargo tree')
    parser.add_argument('--supplements', type=Path, help='Verified fixed-commit supplemental documents')
    args = parser.parse_args()
    raw = args.cargo_metadata.read_bytes()
    packages = collect(json.loads(raw))
    tree_hash = None
    if args.cargo_tree:
        tree_bytes = args.cargo_tree.read_bytes()
        tree_hash = hashlib.sha256(tree_bytes).hexdigest()
        pairs = set()
        for line in tree_bytes.decode().splitlines():
            if not line.strip():
                continue
            match = re.fullmatch(r'([^ ]+) v([^ ]+)(?: .*?)?', line)
            if not match:
                raise ValueError(f'Unrecognized Cargo tree line: {line}')
            pairs.add(match.groups())
        metadata = json.loads(raw)
        known = {(p['name'], p['version']) for p in metadata['packages']}
        if not pairs or pairs - known:
            raise ValueError(f'Cargo tree does not match metadata: {sorted(pairs - known)}')
        packages = [p for p in packages if p['ecosystem'] == 'cargo'
                    and (p['name'], p['version']) in pairs]
        if not packages:
            raise ValueError('No external dependencies selected')
    if args.supplements:
        supplement(packages, json.loads(raw), args.supplements)
    args.output.mkdir(parents=True, exist_ok=False)
    inventory = dict(scope='Dependency documents for review; not final linkage or license clearance',
        target=args.target, cargo_metadata_sha256=hashlib.sha256(raw).hexdigest(),
        cargo_lock_sha256=hashlib.sha256((ROOT / 'Cargo.lock').read_bytes()).hexdigest(),
        npm_lock_sha256=hashlib.sha256((ROOT / 'package-lock.json').read_bytes()).hexdigest(),
        selection=f'Cargo {args.cargo_tree_edges} dependency graph; not final linked symbols' if args.cargo_tree else 'All metadata and npm packages',
        cargo_tree_sha256=tree_hash, packages=packages)
    (args.output / 'inventory.json').write_text(json.dumps(inventory, ensure_ascii=False, indent=2) + '\n')
    sections = ['THIRD-PARTY DOCUMENTS — REVIEW DRAFT\nNot a redistribution clearance or complete source offer.\n']
    for package in packages:
        sections.append(f"\n{'=' * 72}\n{package['ecosystem']}: {package['name']} {package['version']}\n"
                        f"Declared license: {package['declared_license']}\nStatus: {package['status']}\n")
        if package.get('selected_license'):
            sections.append(f"Selected license: {package['selected_license']}\n")
        if package.get('remaining_review'):
            sections.append(f"Remaining review: {package['remaining_review']}\n")
        for document in package['files']:
            sections.append(f"\n--- {document['path']} ---\n{document['text']}\n")
    (args.output / 'THIRD_PARTY_DOCUMENTS.txt').write_text(''.join(sections))
    unresolved = [dict(ecosystem=p['ecosystem'], name=p['name'], version=p['version'],
                       declared_license=p['declared_license'], unreadable=p['unreadable'])
                  for p in packages if p['status'] == 'needs_review']
    (args.output / 'needs-review.json').write_text(json.dumps(unresolved, ensure_ascii=False, indent=2) + '\n')
    open_reviews = [dict(name=p['name'], version=p['version'], remaining_review=p['remaining_review'])
                    for p in packages if p.get('remaining_review')]
    (args.output / 'explicit-review-items.json').write_text(json.dumps(dict(
        scope='Known open review items; absence is not redistribution clearance',
        items=open_reviews), ensure_ascii=False, indent=2) + '\n')
    print(json.dumps(dict(packages=len(packages), needs_review=len(unresolved), output=str(args.output))))


if __name__ == '__main__':
    main()
