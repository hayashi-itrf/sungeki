#!/bin/sh
# Foreground owner for isolated local speech validation.
set -eu
exec python3 "$(dirname "$0")/local-speech-dev/control.py" coeiroink "$@"
