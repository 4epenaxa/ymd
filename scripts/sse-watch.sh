#!/usr/bin/env bash
# Живой просмотр SSE демона — эталон для S4.
set -euo pipefail

port="${1:-8765}"
key_path="${XDG_RUNTIME_DIR:-/tmp}/ymd/session.key"

if [ ! -r "$key_path" ]; then
  echo "нет $key_path — сначала запусти: cargo run -p ymd -- serve" >&2
  exit 1
fi

key="$(cat "$key_path")"
echo "# GET http://127.0.0.1:$port/v1/events"
curl -N -sS -H "X-YMD-Session: $key" "http://127.0.0.1:$port/v1/events"
