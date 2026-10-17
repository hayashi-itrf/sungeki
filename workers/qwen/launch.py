"""Sungeki-owned local Qwen entrypoint; caller owns resources and inherited socket.

Use an inspected environment and complete manifests. This launcher neither
attests loaded weights nor adopts/stops a server found at an existing port.
"""
import argparse
import importlib
import os
from pathlib import Path
import socket
import sys


def directory(value):
    path = Path(value)
    if not path.is_absolute() or '..' in path.parts or not path.is_dir():
        raise argparse.ArgumentTypeError('an existing absolute directory is required')
    return path.resolve()


def port_number(value):
    value = int(value)
    if not 1024 <= value <= 65535:
        raise argparse.ArgumentTypeError('port must be between 1024 and 65535')
    return value


def validate_listener(fd, port):
    if fd < 0:
        raise ValueError('negative listener descriptor')
    duplicate = os.dup(fd)
    try:
        listener = socket.socket(fileno=duplicate)
    except OSError:
        os.close(duplicate)
        raise
    with listener:
        if (listener.family != socket.AF_INET
                or listener.getsockopt(socket.SOL_SOCKET, socket.SO_TYPE) != socket.SOCK_STREAM
                or listener.getsockname() != ('127.0.0.1', port)):
            raise ValueError('expected the parent loopback listener')
        listener.listen(socket.SOMAXCONN)


def configure_caches(work_root):
    # Resolve every destination before creating anything: an existing cache
    # symlink must not redirect writes into originals or another application's data.
    relative = {
        'XDG_CACHE_HOME': 'cache', 'HF_HOME': 'cache/huggingface',
        'HF_HUB_CACHE': 'cache/huggingface/hub',
        'HF_ASSETS_CACHE': 'cache/huggingface/assets',
        'HF_MODULES_CACHE': 'cache/huggingface/modules',
        'HF_XET_CACHE': 'cache/huggingface/xet', 'TORCH_HOME': 'cache/torch',
    }
    paths = {key: work_root / value for key, value in relative.items()}
    if any(path.resolve() != path for path in paths.values()):
        raise ValueError('cache directories must not contain symlinks')
    for path in paths.values():
        path.mkdir(parents=True, exist_ok=True)
    # Older Transformers names take precedence over HF_HOME when inherited.
    for key in ('PYTORCH_PRETRAINED_BERT_CACHE', 'PYTORCH_TRANSFORMERS_CACHE',
                'TRANSFORMERS_CACHE'):
        os.environ.pop(key, None)
    os.environ.update({key: str(path) for key, path in paths.items()})


def main(argv=None):
    parser = argparse.ArgumentParser(description='Sungeki local Qwen launcher')
    for name in ('engine-root', 'model-root', 'voice-root', 'work-root'):
        parser.add_argument('--' + name, type=directory, required=True)
    parser.add_argument('--port', type=port_number, required=True)
    parser.add_argument('--listen-fd', type=int, required=True)
    args = parser.parse_args(argv)
    try:
        validate_listener(args.listen_fd, args.port)
    except (OSError, ValueError) as error:
        parser.error(f'invalid listener: {error}')
    if not (args.engine_root / 'api/main.py').is_file():
        parser.error('Qwen api/main.py is missing')
    if not (args.model_root / 'config.json').is_file():
        parser.error('local model config.json is missing')
    # All mutable work belongs to Sungeki; vendor sources and model directories
    # must never become the working directory or receive Python bytecode caches.
    if any(args.work_root == root or args.work_root.is_relative_to(root)
           for root in (args.engine_root, args.model_root, args.voice_root)):
        parser.error('work-root must be separate from engine/model/voice originals')
    os.chdir(args.work_root)
    sys.dont_write_bytecode = True
    try:
        configure_caches(args.work_root)
    except (OSError, ValueError) as error:
        parser.error(f'invalid cache directory: {error}')
    os.environ.update({
        'HF_HUB_OFFLINE': '1', 'TRANSFORMERS_OFFLINE': '1',
        'HF_HUB_DISABLE_TELEMETRY': '1', 'GRADIO_ANALYTICS_ENABLED': 'False',
        'TTS_BACKEND': 'official', 'TTS_MODEL_ID': str(args.model_root),
        'TTS_MODEL_NAME': str(args.model_root), 'HOST': '127.0.0.1',
        'PORT': str(args.port), 'WORKERS': '1', 'RELOAD': 'false',
        'ENABLE_VOICE_STUDIO': 'false', 'TTS_WARMUP_ON_START': 'false',
        'GPU_KEEPALIVE_INTERVAL': '0', 'VOICE_LIBRARY_DIR': str(args.voice_root),
        'TTS_CUSTOM_VOICES': str(args.voice_root / 'custom'),
    })
    sys.path.insert(0, str(args.engine_root))
    uvicorn = importlib.import_module('uvicorn')
    uvicorn.run('api.main:app', host='127.0.0.1', port=args.port,
                fd=args.listen_fd, workers=1, reload=False, lifespan='on')


if __name__ == '__main__':
    main()
