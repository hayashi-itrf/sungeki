#!/bin/sh
# Sungeki専用の開発検証。startは前景の管理セッションを保持する。
set -eu
exec python3 "$(dirname "$0")/voicevox-dev/control.py" "$@"
