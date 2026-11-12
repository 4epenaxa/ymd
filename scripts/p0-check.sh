#!/usr/bin/env bash
# Автоматические проверки P0. Запуск: bash scripts/p0-check.sh
set -uo pipefail

pass=0
warn=0
ok()  { printf '  [ok]   %s\n' "$1"; pass=$((pass + 1)); }
no()  { printf '  [warn] %s\n' "$1"; warn=$((warn + 1)); }
head1() { printf '\n== %s ==\n' "$1"; }
have() { command -v "$1" >/dev/null 2>&1; }

head1 "S1 Noctalia"
if have noctalia; then
  ok "noctalia: $(noctalia --version 2>&1 | head -n1)"
else
  no "noctalia не в PATH"
fi

head1 "S4/S5 daemon и capability-файл"
key="${XDG_RUNTIME_DIR:-/tmp}/ymd/session.key"
if [ -r "$key" ]; then
  ok "session.key найден: $key"
else
  no "session.key нет — запусти: cargo run -p ymd -- serve"
fi
if curl -fsS --max-time 2 http://127.0.0.1:8765/v1/health >/dev/null 2>&1; then
  ok "daemon отвечает на /v1/health"
else
  no "daemon не отвечает на :8765"
fi

head1 "S6 MPRIS"
if have playerctl; then
  ok "playerctl: $(playerctl --version 2>&1 | head -n1)"
else
  no "playerctl не установлен (dnf install playerctl)"
fi
if have busctl; then
  ok "busctl есть"
else
  no "busctl нет (systemd)"
fi

head1 "S7 GStreamer / PipeWire"
if have gst-launch-1.0; then
  ok "gst-launch-1.0 есть"
else
  no "нет gstreamer1-tools (sudo dnf install gstreamer1-plugins-base)"
fi
if have pactl; then
  ok "pactl есть"
  if pactl info 2>/dev/null | grep -qi pipewire; then
    ok "PipeWire как звуковой сервер"
  else
    no "PipeWire не обнаружен через pactl"
  fi
else
  no "нет pactl (sudo dnf install pulseaudio-utils)"
fi
if have mpv; then ok "mpv есть (резервный backend)"; else no "mpv нет (опционально)"; fi

head1 "S8 niri"
if [ -n "${NIRI_SOCKET:-}" ] && [ -S "${NIRI_SOCKET}" ]; then
  ok "NIRI_SOCKET=${NIRI_SOCKET}"
else
  no "NIRI_SOCKET не задан или сокета нет"
fi
if have niri; then
  ok "niri: $(niri --version 2>&1 | head -n1)"
  if niri msg --json focused-window >/dev/null 2>&1; then
    ok "niri msg отвечает"
  else
    no "niri msg не отвечает"
  fi
else
  no "niri не в PATH"
fi

head1 "Плагин"
plug="$HOME/.local/share/noctalia/plugins/yandex-music"
if [ -e "$plug" ]; then
  ok "плагин слинкован: $plug"
else
  no "нет $plug — запусти: bash scripts/dev-install.sh"
fi

printf '\nИтого: ok=%d warn=%d\n' "$pass" "$warn"
printf 'warn — это не всегда ошибка, см. docs/09-p0-spikes.md\n'
