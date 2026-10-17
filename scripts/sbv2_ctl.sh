#!/bin/sh
# Foreground owner for isolated SBV2 qualification; production jobs use the host.
set -eu
exec python3 "$(dirname "$0")/sbv2-dev/control.py" "$@"
