#!/usr/bin/env bash
# Локальная установка плагина как path/drop-in источника.
# Каталог в data-директории имеет высший приоритет среди источников.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
dest="$HOME/.local/share/noctalia/plugins/yandex-music"

mkdir -p "$(dirname "$dest")"
ln -sfn "$repo/plugin" "$dest"
echo "linked: $dest -> $repo/plugin"

if command -v noctalia >/dev/null 2>&1; then
  noctalia msg plugins enable pbolkhovitin/yandex-music || true
  noctalia msg plugins list | grep -i yandex || true
else
  echo "noctalia не в PATH: включи плагин вручную (Settings -> Plugins)"
fi
