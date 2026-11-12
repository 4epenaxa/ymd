# HANDOFF — состояние и следующий шаг

Обновлено: 2026-11-12. Проект: `~/github/ym-noctalia`.

## Что это

Клиент Яндекс Музыки для **niri + Noctalia v5**: бар-виджет + панель + Rust-демон.
Плагины Noctalia — Luau без аудио/крипты/MPRIS, поэтому вся «тяжёлая» часть живёт
в демоне `ymd`, а плагин говорит с ним по локальному HTTP + SSE.

## Что уже проверено (факты, не предположения)

| Проверка | Команда | Результат |
|---|---|---|
| сборка | `cargo build -p ymd` | ok, 28 s, без warning'ов |
| запуск | `./target/debug/ymd serve` | `listening on http://127.0.0.1:8765` |
| health | `curl :8765/v1/health` | `{"ok":true,"version":"0.1.0"}` |
| auth | `curl :8765/v1/state` без ключа | `401` |
| state | с `X-YMD-Session` | полный JSON состояния |
| cmd | `POST /v1/cmd {"op":"toggle"}` | `{"ok":true,"rev":1}` |
| SSE | `curl -N /v1/events` | `event: state` раз в секунду |
| CLI | `ymd status` | читает состояние через сокет |

Ключ: `$XDG_RUNTIME_DIR/ymd/session.key` (32 hex, `0600`).

## Что НЕ проверено (это и есть P0)

Плагинная половина — требует запущенного Noctalia и niri:

1. **S1** — версия Noctalia и максимальный `plugin_api` (в манифесте стоит 35).
2. **S2** — реальные методы `barWidget.*` (виджет `probe` печатает дамп в лог).
3. **S3** — работает ли `ui.*` в бар-виджете и панели.
4. **S4** — `noctalia.httpStream` (демонная сторона SSE уже подтверждена).
5. **S5** — читается ли `session.key` из `$XDG_RUNTIME_DIR` через `readFile`.
6. **S6–S9** — MPRIS, GStreamer, niri IPC, свой OAuth.

Порядок и команды: `docs/09-p0-spikes.md`. Результаты писать в
`notes/p0-findings.md` (шаблон готов).

## Следующий шаг

```bash
cd ~/github/ym-noctalia
cargo run -p ymd -- serve          # терминал 1
bash scripts/dev-install.sh        # плагин как drop-in
bash scripts/p0-check.sh
journalctl --user -f -o cat | grep -iE "\[probe\]|\[bridge\]"
```

Затем — S6–S9 и M1: GStreamer (`playbin3` → `pipewiresink`), MPRIS, OAuth.

## Зависимости, которых ещё нет

`gstreamer1-devel`, `gstreamer1-plugins-base`, `gstreamer1-plugins-good`,
`gstreamer1-plugin-libav`, `libsecret-devel`, `playerctl` (для проверок).
Установка — только с sudo от пользователя.

## Связанный проект

`~/github/yandex-music-research` — реверс-инжиниринг официального клиента:
API, заголовки, HMAC-подписи `file-info`, `encraw` + AES-CTR, Ynison, OAuth.
Там же готовые крейты `ym-api` / `ym-auth` / `ym-media` — их код переносится
в демон на этапе M1.

## Правила

- Плагин не видит токен и не ходит в API Яндекса.
- Цвета — только `noctalia.getColor(...)`, без `#rrggbb`.
- Ноль анимаций по умолчанию.
- Факт → `docs/`, гипотеза → `notes/p0-findings.md`.
