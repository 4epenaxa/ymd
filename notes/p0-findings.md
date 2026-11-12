# P0 — факты проверок

Заполнять по мере прохождения `docs/09-p0-spikes.md`. Только факты, без «должно
работать». Каждая строка — вывод команды или цитата из лога.

## Уже проверено: демон (2026-11-12)

| Проверка | Факт |
|---|---|
| `cargo build -p ymd` | собирается, 28 s, без warning'ов |
| `/v1/health` | `{"ok":true,"state_rev":0,"uptime_s":1,"version":"0.1.0"}` |
| `/v1/state` без `X-YMD-Session` | `401` |
| `/v1/state` с ключом | JSON: `rev/status/track/position_ms/volume/queue/env/ui` |
| `/v1/cmd {"op":"toggle"}` | `{"ok":true,"rev":1}`, статус меняется на `playing` |
| `/v1/events` | `event: state` + `data: {...}` раз в секунду (тик 1 Гц) |
| `ymd status` | читает состояние через loopback-сокет |
| `session.key` | 32 hex-символа, права `0600` |

Осталось: всё, что требует Noctalia/niri (S1–S3, S5) и систему (S6–S9).

## Окружение

| Параметр | Значение |
|---|---|
| Дата проверки | |
| Noctalia | |
| `plugin_api` (максимум хоста) | |
| niri | |
| PipeWire | |
| GStreamer | |

## S1. Версия и API

~~~text
(вывод noctalia --version / msg plugins list)
~~~

## S2. `barWidget.*`

| Метод | Есть | Тип |
|---|---|---|
| setText | | |
| setGlyph | | |
| setIcon | | |
| setTooltip | | |
| setColor | | |
| … | | |

Полный дамп:

~~~text
(строка [probe] barWidget (...))
~~~

## S3. `ui.*`

| Проверка | Результат |
|---|---|
| `ui.column` в бар-виджете | |
| `ui.progress` в баре | |
| панель рендерится | |
| `[[widget.setting]]` принимается | |

## S4. `httpStream` / SSE

| Проверка | Результат |
|---|---|
| формат ответа | |
| разрыв detected | |
| реконнект | |
| лимит длительности | |

## S5. `readFile` вне плагина

~~~text
(строка [probe] session.key ...)
~~~

## S6. MPRIS

~~~text
(playerctl --list-all / busctl --user list | grep mpris)
~~~

## S7. GStreamer → PipeWire

~~~text
(вывод gst-launch-1.0 --version / pactl info)
~~~

## S8. niri

| Проверка | Результат |
|---|---|
| `NIRI_SOCKET` | |
| `WorkspaceActivated` приходит | |
| `WindowFocusChanged` приходит | |
| поле fullscreen | |

## S9. OAuth

| Проверка | Результат |
|---|---|
| свой client_id | |
| `response_type=code` | |
| loopback callback | |
| обмен кода на токен | |

## Решения по итогам

| Вопрос | Решение | Дата |
|---|---|---|
| `plugin_api` | | |
| `ui.*` в баре | | |
| транспорт состояния | | |
| backend плеера | | |
| OAuth | | |

## Что правим в документации

- [ ] `docs/03-noctalia-plugin.md` — заменить «эскиз» на фактический API.
- [ ] `docs/04-daemon.md` — уточнить поля niri/fullscreen.
- [ ] `docs/07-roadmap.md` — скорректировать оценки.
