"""Launch only an owned Irodori worker with explicit local originals.

Invoke with the inspected Python environment using -I -B. The parent must hold
engine/GPU leases, attest manifests, and stop/reap this process on cancellation.
"""
import argparse
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from environment import absolute_path, configure
from bridge import RuntimeBridge, registered_voice, text
from server import OwnedServer, unique_object, reject_constant


def main(argv=None):
    parser = argparse.ArgumentParser(description='Sungeki owned Irodori worker')
    for name in ('engine-root', 'checkpoint', 'codec', 'registry', 'work-root', 'checkpoint-id'):
        parser.add_argument('--' + name, required=True)
    parser.add_argument('--dependency-cache')
    parser.add_argument('--listen-fd', type=int, required=True)
    parser.add_argument('--device', choices=('cpu', 'mps', 'cuda'), required=True)
    parser.add_argument('--precision', choices=('fp32', 'fp16', 'bf16'), default='fp32')
    args = parser.parse_args(argv)
    try:
        engine = absolute_path(args.engine_root, directory=True)
        checkpoint = absolute_path(args.checkpoint)
        codec = absolute_path(args.codec)
        registry = absolute_path(args.registry)
        work = absolute_path(args.work_root, directory=True)
        text(args.checkpoint_id, 4096, 'checkpoint id', True)
        if not (engine / 'irodori_tts/inference_runtime.py').is_file():
            raise ValueError('Irodori inference runtime is missing')
        with registry.open('rb') as stream:
            raw = stream.read(1024 * 1024 + 1)
        if len(raw) > 1024 * 1024:
            raise ValueError('voice registry exceeds 1 MiB')
        voices = json.loads(raw.decode('utf-8'), object_pairs_hook=unique_object,
                            parse_constant=reject_constant)
        if not isinstance(voices, list) or not 1 <= len(voices) <= 4096:
            raise ValueError('invalid voice registry')
        voices = [registered_voice(v) for v in voices]
        if len({v['id'] for v in voices}) != len(voices):
            raise ValueError('duplicate voice')
        originals = [engine, checkpoint.parent, codec.parent, registry.parent]
        originals += [Path(v['path']).parent for v in voices if v.get('path')]
        dependency_cache = absolute_path(args.dependency_cache, directory=True) if args.dependency_cache else None
        configure(work, originals, dependency_cache)
        # Reject an invalid descriptor before importing torch or loading models.
        server = OwnedServer(args.listen_fd, None)
    except (OSError, ValueError, RecursionError) as error:
        parser.error(str(error))
    runtime = None
    try:
        sys.path.insert(0, str(engine))
        from irodori_tts.inference_runtime import InferenceRuntime, RuntimeKey
        key = RuntimeKey(checkpoint=str(checkpoint), codec_repo=str(codec),
                         model_device=args.device, codec_device=args.device,
                         model_precision=args.precision, codec_precision=args.precision,
                         codec_deterministic_encode=True, codec_deterministic_decode=True,
                         compile_model=False, compile_dynamic=False)
        runtime = InferenceRuntime.from_key(key)
        server.bridge = RuntimeBridge(runtime, args.checkpoint_id, voices)
        server.serve_forever(poll_interval=.1)
    finally:
        server.server_close()
        # Never unload tensors beneath a still-running request thread. SIGTERM
        # uses normal OS process termination; the parent confirms process exit.
        if runtime is not None and not server.synthesis.locked():
            runtime.unload()


if __name__ == '__main__':
    main()
