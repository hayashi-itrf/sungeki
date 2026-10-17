"""Stage native dependency documents and exact MPL sources for local bundles.

This assembles evidence, not permission to publish. Known open reviews accompany
the documents. Only the dedicated generated resource directory is replaced.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
DESTINATION = ROOT / 'apps/desktop/src-tauri/resources/native-notices'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target', required=True)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='vvm-native-notices-') as temporary:
        work = Path(temporary)
        metadata = work / 'cargo.json'
        tree = work / 'tree.txt'
        metadata.write_bytes(subprocess.check_output([
            'cargo', 'metadata', '--locked', '--offline', '--filter-platform', args.target,
            '--format-version', '1'], cwd=ROOT))
        # Include build-time code generators, even when their crates are not linked.
        tree.write_bytes(subprocess.check_output([
            'cargo', 'tree', '--locked', '--offline', '--target', args.target,
            '-p', 'sungeki-desktop', '-p', 'sungeki-host', '--edges', 'normal,build',
            '--prefix', 'none', '--format', '{p}'], cwd=ROOT))
        review = work / 'review'
        subprocess.run([sys.executable, str(ROOT / 'scripts/collect-license-evidence.py'),
            '--cargo-metadata', str(metadata), '--target', args.target, '--cargo-tree', str(tree),
            '--cargo-tree-edges', 'normal,build',
            '--supplements', str(ROOT / 'licenses/native-supplements'),
            '--output', str(review)], cwd=ROOT, check=True)
        missing = json.loads((review / 'needs-review.json').read_text())
        if missing:
            names = ', '.join(f"{p['name']} {p['version']}" for p in missing)
            raise ValueError(f'Native dependency documents are missing: {names}')
        staged = work / 'native-notices'
        staged.mkdir()
        for source, target in [('THIRD_PARTY_DOCUMENTS.txt', 'THIRD_PARTY_NATIVE.txt'),
                               ('inventory.json', 'inventory.json'),
                               ('explicit-review-items.json', 'explicit-review-items.json')]:
            shutil.copyfile(review / source, staged / target)
        subprocess.run([sys.executable, str(ROOT / 'scripts/prepare-mpl-sources.py'),
            '--cargo-metadata', str(metadata), '--output', str(staged / 'mpl-sources')],
            cwd=ROOT, check=True)
        hashes = {str(p.relative_to(staged)): hashlib.sha256(p.read_bytes()).hexdigest()
                  for p in sorted(staged.rglob('*')) if p.is_file()}
        (staged / 'manifest.json').write_text(json.dumps(dict(target=args.target,
            scope='Native dependency documents and MPL source archives; not release clearance',
            files=hashes), indent=2) + '\n')
        # Staging has completed successfully before touching the old generated copy.
        if DESTINATION.is_symlink():
            raise ValueError('Generated notices directory must not be a symlink')
        if DESTINATION.exists():
            shutil.rmtree(DESTINATION)
        shutil.copytree(staged, DESTINATION)
        print(f'Staged {len(hashes)} native notice/source files')


if __name__ == '__main__':
    main()
