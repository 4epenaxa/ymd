# 02. Архитектура

## Обзор

~~~text
┌──────────────────────────── Noctalia (C++ core) ────────────────────────────┐
│  bar ── widget entry ───┐                                                   │
│  panel ── panel entry ──┼──> Luau VM (на entry): yandex-music               │
│  control center ────────┘        │                                          │
│                                  │ noctalia.http / httpStream               │
└──────────────────────────────────┼──────────────────────────────────────────┘
                                   │ 127.0.0.1:8765 (JSON + SSE)
┌──────────────────────────────────▼──────────────────────────────────────────┐
│ ymd (Rust, systemd --user)                                                  │
│  http api ──┬── player (GStreamer → PipeWire)                               │
│             ├── library (API-клиент, подписи)                               │
│             ├── cache (SQLite + файлы, AES-CTR)                             │
│             ├── auth (OAuth loopback + libsecret)                           │
│             ├── ynison (WebSocket)                                          │
│             └── env (niri IPC, idle, power)                                 │
│  MPRIS ── org.mpris.MediaPlayer2.ymd  ──> Noctalia media widget / shell     │
└─────────────────────────────────────────────────────────────────────────────┘
                       │ HTTPS
                 api.music.yandex.ru
~~~

## Почему так

| Альтернатива | Вердикт |
|---|---|
| Только Luau-плагин | Невозможно: нет аудиостеков, AES-CTR, keyring; VM с бюджетами на вызов |
| Electron-обёртка | 350 МБ, анимации, Chromium — противоречит цели |
| Tauri-приложение | Второе окно мимо бара; интегрировать в бар всё равно через плагин |
| **Демон + плагин** | Нативность + нативный UI бара; каждый слой по своим силам |

Плагин **никогда** не обращается к API Яндекса и не видит токен. Это и
безопасность (нет секретов в скрипте), и ремонтопригодность (API меняется в одном
месте).

## Компоненты

### Демон `ymd`

| Модуль | Ответственность | Зависимости |
|---|---|---|
| `auth` | OAuth loopback, refresh, libsecret | `keyring`, `tiny_http` |
| `api` | HTTP-клиент: заголовки, ретраи, подписи, типы | `reqwest`, `hmac` |
| `player` | очередь, GStreamer `playbin3`, seek, громкость | `gstreamer` |
| `mpris` | `org.mpris.MediaPlayer2` + `Player` | `zbus` |
| `cache` | SQLite-метаданные, файлы, LRU, докачка | `rusqlite`, `aes-ctr` |
| `ynison` | WebSocket-синхронизация (опционально) | `tokio-tungstenite` |
| `env` | niri IPC, idle, AC/battery | `niri-ipc` |
| `httpd` | локальный API + SSE | `axum` |
| `cli` | `ymd ctl …`, `ymd auth login` | `clap` |

Один процесс, один tokio-runtime, состояние в `Arc<RwLock<State>>`; события —
`tokio::sync::broadcast` в SSE-подписчиков.

### Плагин `pbolkhovitin/yandex-music`

| Entry | Назначение | Ключевые API |
|---|---|---|
| `[[widget]] bar` | строка в баре | `barWidget.*`, `ui.*` |
| `[[service]] bridge` | SSE-подписка, кэш состояния, реконнект | `noctalia.httpStream` |
| `[[panel]] panel` | библиотека, поиск, очередь | `ui.scroll`, `ui.input` |
| `[[shortcut]]` | play/pause, лайк | `onClick`, `noctalia.notify` |
| `[[launcher_provider]]` | `>ym` | `onQuery`, `onActivate` |

Каждый entry — **своя VM**: общих глобалов нет, состояние синхронизируется через
демон и `noctalia.state`/`pluginDataDir()` (файл).

## IPC-контракт

Транспорт: loopback HTTP, порт по умолчанию `8765` (настраивается, `0` = UNIX-socket).
Аутентификация: capability-файл `$XDG_RUNTIME_DIR/ymd/session.key` (0600), плагин
читает его через `noctalia.readFile` и передаёт в `X-YMD-Session`. Демон принимает
только `127.0.0.1` и проверяет заголовок — так чужие локальные процессы не могут
управлять плеером.

### Эндпоинты

| Метод | Путь | Назначение |
|---|---|---|
| GET | `/v1/health` | `{ok, version, uptime}` |
| GET | `/v1/state` | снимок состояния (см. ниже) |
| GET | `/v1/events` | SSE: `state`, `track`, `queue`, `error` |
| POST | `/v1/cmd` | `{op, args}` — команды |
| GET | `/v1/search?q=&type=` | поиск (прокси с типизацией) |
| GET | `/v1/lyrics?track=&format=` | текст |
| POST | `/v1/download` | `{trackId, quality}` |
| GET | `/v1/cover/{id}?size=` | обложка (кэш) |

### Состояние

~~~json
{
  "rev": 42,
  "status": "playing",
  "track": {
    "id": "12345", "title": "Кино", "artists": ["MACAN"],
    "album": "…", "durationMs": 184000, "liked": true
  },
  "positionMs": 61000,
  "volume": 0.8,
  "queue": { "length": 32, "index": 3, "shuffle": false, "repeat": "off" },
  "wave": { "active": true },
  "offline": { "downloaded": true, "quality": "hq" },
  "env": { "fullscreen": false, "overview": false, "idle": false, "onBattery": true },
  "ui": { "mode": "full" }
}
~~~

`rev` монотонно растёт — плагин отбрасывает устаревшие ответы (гонки при быстрых
кликах).

### Команды `POST /v1/cmd`

`play`, `pause`, `toggle`, `next`, `prev`, `seek{ms}`, `volume{v}`,
`like{on}`, `dislike{on}`, `play{trackId|albumId|playlistId}`, `wave{on}`,
`queue{add|clear|set}`, `mode{name}`, `open-panel`.

Ответ: `{ok, rev}` — плагин не ждёт `state`, а применяет оптимистично и
корректирует по SSE.

## Потоки

**Клик «play/pause».** Плагин → `POST /v1/cmd {op:"toggle"}` → демон меняет
GStreamer → MPRIS `PlaybackStatus` → SSE `state` (rev+1) → виджет.

**Смена трека.** GStreamer `about-to-finish` → демон берёт следующий из очереди →
`file-info` c HMAC → `playbin3.set_uri` → SSE + MPRIS `Metadata`.

**Офлайн.** `POST /v1/download` → `encraw`-чанки → AES-128-CTR (counter =
`offset/16`) → файл + строка в SQLite → SSE `offline`.

**Адаптивность.** `env` слушает `$NIRI_SOCKET` (`Event::WindowFocusChanged`,
`WorkspaceActivated`, fullscreen) + `ext-idle-notify` + UPower → пересчитывает
`ui.mode` → SSE → виджет мгновенно меняет плотность.

## Жизненный цикл и отказы

| Ситуация | Поведение |
|---|---|
| Демон не запущен | виджет показывает `off` глиф; клик → `systemctl --user start ymd` |
| Демон упал | `Restart=on-failure`, `RestartSec=1`; плагин — реконнект SSE с backoff 1→5 с |
| Нет сети | очередь продолжает играть из кэша; ошибки — в панели, не в баре |
| Нет niri | `env` пуст, `mode` фиксируется по настройке |
| Токен истёк | демон уведомляет (`notify`), панель показывает «Войти» |
| Несколько инстансов | `ymd` — одиночка (lock-файл), второй экземпляр отдаёт `ctl` первому |

## Безопасность

- Демон слушает только `127.0.0.1`; capability-файл `0600`.
- Токен — libsecret (`service=ymd, user=<login>`), в плагин не попадает.
- Плагин не выполняет `sh -c` с пользовательскими строками: только argv-форма
  `runAsync` (`plugin_api >= 24`).
- Обложки и текст рендерятся как данные (`ui.image` / `ui.label`), не как HTML.
