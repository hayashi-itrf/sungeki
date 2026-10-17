#!/bin/sh
# Foreground owner for isolated Qwen engine qualification, not production jobs.
set -eu
exec python3 "$(dirname "$0")/qwen-dev/control.py" "$@"
