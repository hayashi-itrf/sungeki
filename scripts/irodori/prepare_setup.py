"""Inventory an explicit installed environment, without loading TTS or installing settings.

The host must hash and validate this setup before use. OS libraries and dynamic
loader coverage are not proved by enumerating these local package trees.
"""
import argparse
import json
from pathlib import Path


def tree(root, prefix):
    result = {}
    for path in sorted(root.rglob('*')):
        if '__pycache__' in path.parts or path.suffix == '.pyc':
            continue
        if path.is_file():
            result[prefix + path.relative_to(root).as_posix()] = str(path.resolve(strict=True))
    return result


def prepare(vendor, venv, staged, sample_rate, port):
    repo = Path(__file__).resolve().parents[2]
    python = venv / 'bin/python'
    target = python.resolve(strict=True)
    standard = target.parent.parent / 'lib/python3.11'
    if not standard.is_dir():
        raise ValueError('expected pinned Python 3.11 runtime')
    voice = staged / 'originals/voice'
    voice.mkdir(exist_ok=True)
    registry = voice / 'registry.json'
    expected = [dict(id='narrator', name='参照なし・日本語ナレーション', reference='none', caption=None)]
    if registry.exists() and json.loads(registry.read_text()) != expected:
        raise ValueError('existing registry differs; never overwrite')
    if not registry.exists():
        with registry.open('x') as stream:
            json.dump(expected, stream, ensure_ascii=False)
    cache = staged / 'originals/hf-hub'
    engine = tree(vendor / 'irodori_tts', 'irodori_tts/')
    engine.update(tree(venv / 'lib', 'venv/lib/'))
    engine.update(tree(standard, 'python/lib/'))
    engine.update(tree(cache, 'hf-cache/'))
    engine.update(tree(repo / 'workers/irodori', 'next/'))
    engine['python_executable'] = str(target)
    engine['pyvenv.cfg'] = str(venv / 'pyvenv.cfg')
    # Include Python's linked runtime libraries as well as the executable.
    for path in (target.parent.parent / 'lib').glob('*.dylib'):
        engine['python/dylib/' + path.name] = str(path.resolve(strict=True))
    model = tree(staged / 'originals/model', '')
    checkpoint = model.pop('model.safetensors')
    model['checkpoint'] = checkpoint
    codec = staged / 'originals/codec/weights.pth'
    work = staged / 'work'
    work.mkdir(exist_ok=True)
    return dict(format_version=1, id='irodori-v4-small-mps', name='Irodori v4 Small（Sungeki検証）',
        port=port, default_settings=dict(voice_id='narrator', caption='落ち着いた日本語のナレーション。', cfg_scale=3., num_steps=40, seed=42),
        model=dict(checkpoint='Irodori-TTS-v4-Small-e4aaac4d', speaker_conditioning=True,
                   caption_conditioning=True, sample_rate=sample_rate, channels=1),
        binding=dict(engine_root=str(vendor), checkpoint=checkpoint, codec=str(codec),
            registry=str(registry), work_root=str(work), dependency_cache=str(cache),
            executable=str(python), executable_target=str(target), launcher=str(repo / 'workers/irodori/launch.py'),
            device='mps', precision='fp32', originals=dict(model=model, codec={'codec': str(codec)},
            voice={'registry': str(registry)}, engine=engine)))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('vendor', 'venv', 'staged'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--sample-rate', type=int, required=True, help='declared value, checked against runtime later')
    parser.add_argument('--port', type=int, required=True)
    args = parser.parse_args()
    paths = [p.resolve(strict=True) for p in (args.vendor, args.venv, args.staged)]
    print(json.dumps(prepare(*paths, args.sample_rate, args.port), ensure_ascii=False, indent=2))
