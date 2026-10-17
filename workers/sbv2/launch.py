"""Sungeki's local-only SBV2 launcher. Does not download models or edit engine files.

The host must inspect this file and the runtime, reserve the engine, and own the
process tree before invoking it. A listening endpoint is not ownership evidence.
"""
import argparse
import importlib
import os
from pathlib import Path
import runpy
import socket
import sys
import shutil
import tempfile
import signal


def absolute_directory(value: str) -> Path:
    path = Path(value)
    if not path.is_absolute() or ".." in path.parts or not path.is_dir():
        raise argparse.ArgumentTypeError("an existing absolute directory is required")
    return path.resolve()


def port_number(value: str) -> int:
    port = int(value)
    if not 1024 <= port <= 65535:
        raise argparse.ArgumentTypeError("port must be between 1024 and 65535")
    return port


def validate_listener(fd: int, port: int) -> None:
    if fd < 0:
        raise ValueError("negative listening descriptor")
    # Inspect a duplicate: validation must not close the inherited descriptor.
    duplicate = os.dup(fd)
    try:
        listener = socket.socket(fileno=duplicate)
    except OSError:
        os.close(duplicate)
        raise
    with listener:
        if (listener.family != socket.AF_INET
                or listener.getsockopt(socket.SOL_SOCKET, socket.SO_TYPE) != socket.SOCK_STREAM
                or listener.getsockname() != ("127.0.0.1", port)):
            raise ValueError("expected the parent's loopback listening socket")
        # macOS does not implement getsockopt(SO_ACCEPTCONN). Reapply listen
        # to this already-bound descriptor; never bind a new endpoint here.
        listener.listen(socket.SOMAXCONN)


def prepare_work(engine, models, work):
    if any(work == root or work.is_relative_to(root) or root.is_relative_to(work)
           for root in (engine, models)):
        raise ValueError("work-root must be separate from originals")
    # Fresh private directory prevents earlier compiled dictionaries or caches
    # from silently becoming undeclared synthesis inputs.
    return tempfile.TemporaryDirectory(prefix="sbv2-", dir=work)


def configure_work(engine, work):
    for name in ("config.yml", "configs/paths.yml", "dict_data/default.csv"):
        source = engine / name
        if not source.is_file():
            raise ValueError(f"required engine input missing: {name}")
        destination = work / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)
    user_dictionary = engine / "dict_data/user_dict.json"
    if user_dictionary.exists():
        if not user_dictionary.is_file():
            raise ValueError("user dictionary must be a regular file")
        shutil.copyfile(user_dictionary, work / "dict_data/user_dict.json")
    cache_names = {
        "XDG_CACHE_HOME": "cache", "HF_HOME": "cache/huggingface",
        "HF_HUB_CACHE": "cache/huggingface/hub", "HF_ASSETS_CACHE": "cache/huggingface/assets",
        "HF_MODULES_CACHE": "cache/huggingface/modules", "HF_XET_CACHE": "cache/huggingface/xet",
        "TORCH_HOME": "cache/torch", "NUMBA_CACHE_DIR": "cache/numba",
        "MPLCONFIGDIR": "cache/matplotlib", "TMPDIR": "tmp",
    }
    for name in ("PYTORCH_PRETRAINED_BERT_CACHE", "PYTORCH_TRANSFORMERS_CACHE", "TRANSFORMERS_CACHE"):
        os.environ.pop(name, None)
    for name, relative in cache_names.items():
        target = work / relative
        target.mkdir(parents=True, exist_ok=True)
        os.environ[name] = str(target)
    tempfile.tempdir = str(work / "tmp")
    os.chdir(work)


def main(argv=None) -> None:
    parser = argparse.ArgumentParser(description="Sungeki local SBV2 launcher")
    parser.add_argument("--engine-root", type=absolute_directory, required=True)
    parser.add_argument("--work-root", type=absolute_directory, required=True)
    parser.add_argument("--model-root", type=absolute_directory, required=True)
    parser.add_argument("--port", type=port_number, required=True)
    parser.add_argument("--device", choices=("cpu", "auto"), required=True)
    parser.add_argument("--listen-fd", type=int, required=True)
    args = parser.parse_args(argv)
    try:
        validate_listener(args.listen_fd, args.port)
    except (OSError, ValueError) as error:
        parser.error(f"invalid listening socket: {error}")
    server = args.engine_root / "server_fastapi.py"
    if not server.is_file():
        parser.error("server_fastapi.py is missing")
    try:
        work_session = prepare_work(args.engine_root, args.model_root, args.work_root)
    except ValueError as error:
        parser.error(str(error))
    # Uvicorn restores the previous handler and re-raises SIGTERM after its
    # graceful shutdown. Unwind the work context instead of exiting abruptly.
    def terminate(signum, _frame):
        raise SystemExit(128 + signum)

    previous = signal.signal(signal.SIGTERM, terminate)
    try:
        with work_session as work:
            run_engine(args, server, Path(work))
    finally:
        signal.signal(signal.SIGTERM, previous)


def run_engine(args, server, work):
    configure_work(args.engine_root, work)
    sys.dont_write_bytecode = True
    os.environ.pop("OPEN_JTALK_DICT_DIR", None)
    os.environ["HF_HUB_OFFLINE"] = "1"
    os.environ["TRANSFORMERS_OFFLINE"] = "1"
    sys.path.insert(0, str(args.engine_root))
    constants = importlib.import_module("style_bert_vits2.constants")
    constants.DEFAULT_USER_DICT_DIR = work / "dict_data"
    # Keep the engine's own parser/config implementation. Adjust the in-memory
    # port only, so startup logs and actual binding agree; no YAML is rewritten.
    config = importlib.import_module("config")
    original_config = config.get_config

    def get_config():
        value = original_config()
        value.server_config.port = args.port
        return value

    config.get_config = get_config
    # The vendor worker detaches with setsid and may adopt an existing global
    # worker port. Its public functions already support WORKER_CLIENT=None by
    # using pyopenjtalk locally. Keep that supported path inside our owned process.
    openjtalk = importlib.import_module("pyopenjtalk")
    if not Path(os.fsdecode(openjtalk.OPEN_JTALK_DICT_DIR)).is_dir():
        raise ValueError("installed OpenJTalk dictionary is missing")

    def reject_dictionary_download():
        raise RuntimeError("implicit OpenJTalk dictionary download is disabled")

    openjtalk._extract_dic = reject_dictionary_download
    japanese = importlib.import_module("style_bert_vits2.nlp.japanese.pyopenjtalk_worker")

    def initialize_local(*positional, **options):
        if positional or options or japanese.WORKER_CLIENT is not None:
            raise RuntimeError("unsupported shared Japanese worker state")

    japanese.initialize_worker = initialize_local
    uvicorn = importlib.import_module("uvicorn")
    original_run = uvicorn.run

    def run_local(app, *positional, **options):
        # The inspected vendor entrypoint currently supplies keyword arguments.
        if positional:
            raise RuntimeError("unsupported uvicorn invocation")
        options.update(host="127.0.0.1", port=args.port, fd=args.listen_fd)
        return original_run(app, **options)

    uvicorn.run = run_local
    sys.argv = [str(server), "--dir", str(args.model_root)]
    if args.device == "cpu":
        sys.argv.append("--cpu")
    runpy.run_path(str(server), run_name="__main__")


if __name__ == "__main__":
    main()
