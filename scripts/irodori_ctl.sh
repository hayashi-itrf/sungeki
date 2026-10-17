#!/bin/sh
# Foreground isolated development owner; production synthesis uses the host.
set -eu
exec python3 "$(dirname "$0")/irodori-dev/control.py" "$@"
