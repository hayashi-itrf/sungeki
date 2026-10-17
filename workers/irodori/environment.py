"""Writable runtime state belongs to Sungeki, never to inspected originals."""
import os
from pathlib import Path


def absolute_path(value, directory=False):
    path = Path(value)
    if not path.is_absolute() or '..' in path.parts:
        raise ValueError('existing absolute path required')
    path = path.resolve(strict=True)
    if not (path.is_dir() if directory else path.is_file()):
        raise ValueError('unexpected path kind')
    return path


def configure(work, originals, dependency_cache=None):
    if any(work == root or work.is_relative_to(root) or root.is_relative_to(work)
           for root in originals):
        raise ValueError('work directory must be separate from originals')
    destinations = {
        'XDG_CACHE_HOME': 'cache', 'HF_HOME': 'cache/huggingface',
        'HF_HUB_CACHE': 'cache/huggingface/hub',
        'HF_ASSETS_CACHE': 'cache/huggingface/assets',
        'HF_MODULES_CACHE': 'cache/huggingface/modules',
        'HF_XET_CACHE': 'cache/huggingface/xet', 'TORCH_HOME': 'cache/torch',
        'TMPDIR': 'tmp',
    }
    paths = {key: work / value for key, value in destinations.items()}
    if any(path.resolve() != path for path in paths.values()):
        raise ValueError('cache destinations must not contain symlinks')
    for path in paths.values():
        path.mkdir(parents=True, exist_ok=True)
    for key in ('PYTORCH_PRETRAINED_BERT_CACHE', 'PYTORCH_TRANSFORMERS_CACHE',
                'TRANSFORMERS_CACHE'):
        os.environ.pop(key, None)
    os.environ.update({key: str(path) for key, path in paths.items()})
    os.environ.update(HF_HUB_OFFLINE='1', TRANSFORMERS_OFFLINE='1',
                      HF_HUB_DISABLE_TELEMETRY='1', GRADIO_ANALYTICS_ENABLED='False')
    if dependency_cache is not None:
        if dependency_cache.resolve() != dependency_cache or not dependency_cache.is_dir():
            raise ValueError('canonical dependency cache required')
        if work == dependency_cache or work.is_relative_to(dependency_cache) or dependency_cache.is_relative_to(work):
            raise ValueError('dependency cache must be separate from work directory')
        os.environ['HF_HUB_CACHE'] = str(dependency_cache)
    os.chdir(work)
