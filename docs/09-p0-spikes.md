# 09. P0 — пошаговые спайки

Цель P0: **до написания демона и плагина** убедиться, что связка работает.
Каждый шаг заканчивается записью факта в `notes/p0-findings.md`.

Порядок важен: сначала S1–S5 (API хоста и мост), потом S6–S9 (системные
возможности). Если S2/S4 провалились — архитектура меняется, дальше идти нет смысла.

---

## Шаг 0. Подготовка

~~~bash
cd ~/github/ym-noctalia

# 1. Собрать демон (Rust 1.82+)
cargo build -p ymd

# 2. Запустить в отдельном терминале
cargo run -p ymd -- serve
# либо: ./target/debug/ymd serve

# 3. Проверить, что демон жив
curl -s http://127.0.0.1:8765/v1/health | jq
# {"ok":true,"version":"0.1.0","uptime_s":1,"state_rev":0}

# 4. Проверить авторизацию по capability-файлу
KEY="$(cat "${XDG_RUNTIME_DIR}/ymd/session.key")"
curl -s -H "X-YMD-Session: $KEY" http://127.0.0.1:8765/v1/state | jq '.status, .track.title'
# "paused" / "Demo Track"

# 5. Проверить команду и SSE
curl -s -X POST -H "X-YMD-Session: $KEY" -H 'Content-Type: application/json' \
     -d '{"op":"toggle"}' http://127.0.0.1:8765/v1/cmd
# {"ok":true,"rev":2}
bash scripts/sse-watch.sh        # в другом терминале: поток событий раз в секунду

# 6. Автоматические проверки окружения
bash scripts/p0-check.sh
~~~

Если `cargo build` падает — это первый баг, чинится до всего остального.

---

## S1. Какая версия Noctalia и какой `plugin_api`

**Зачем:** `plugin_api = 35` в манифесте должен быть `<=` максимума хоста
(документация заявляет поддержку 3…43).

~~~bash
noctalia --version
noctalia msg plugins list
journalctl --user -b | grep -i "plugin api" | tail -20
~~~

**Ожидаемо:** версия v5.x, список плагинов печатается.
**Записать:** точную версию, максимальный `plugin_api` из лога, путь к логу.

**Если версия v4 (Quickshell/QML):** манифест `plugin.toml` не подойдёт — нужен
`manifest.json` + QML. Это меняет `docs/03`, решение фиксируем в findings.

---

## S2. Реальный список `barWidget.*` (главный риск)

**Зачем:** в документации показаны только `setText`/`setGlyph`; остальное —
догадки. Виджет `probe` перечисляет методы и печатает типы.

~~~bash
# 1. Слинковать плагин как drop-in (высший приоритет источников)
bash scripts/dev-install.sh

# 2. Добавить виджет probe в бар (любой способ)
#    a) Settings -> Bar -> Add widget -> "Yandex Music (P0 stub)" -> probe
#    b) или вручную в ~/.config/noctalia/config.toml:
#       [widget.ym-probe]
#       type = "pbolkhovitin/yandex-music:probe"

# 3. Смотреть лог
journalctl --user -f -o cat | grep -i "\[probe\]"
~~~

**Ожидаемо** в логе:

~~~text
[probe] barWidget (N): setGlyph:function, setText:function, ...
[probe] barWidget[]: setText=function setGlyph=function setIcon=nil ...
[probe] ui (...): button:function, column:function, label:function, progress:function ...
[probe] noctalia (...): getColor:function, http:function, httpStream:function ...
[probe] getColor(primary) = #50d8ed
[probe] getSetting(bar.order) = default
~~~

**Записать:** полный список `barWidget.*` (только реально существующие) и
`ui.*`. Это станет `docs/03` вместо «эскиза».

**Если `barWidget` не имеет нужного метода** (например, нет `setIcon`):
использовать `ui.glyph` внутри `ui.*`-дерева виджета — S3 покажет, разрешено ли.

---

## S3. Работает ли `ui.*` внутри бар-виджета и в панели

**Зачем:** прогресс в баре и вся панель строятся на декларативном дереве.

~~~bash
# 1. Смотрим результат из probe (строка "ui.column in widget = true table")
journalctl --user -f -o cat | grep "ui.column in widget"

# 2. Открыть панель
noctalia msg panel-toggle pbolkhovitin/yandex-music:panel
~~~

**Ожидаемо:** панель открывается, видны заголовок, строка трека, полоса прогресса,
три кнопки. Кнопки меняют состояние (проверять `ymd status` в терминале).

**Проверить настройки (S3b, отдельно):** добавить в `plugin.toml`
`[[widget.setting]]` с `key/type/default`, перезагрузить плагин, открыть настройки
виджета. Если манифест не принимается — прочитать ошибку и записать правильный
синтаксис (в доках встречаются обе формы).

---

## S4. `noctalia.httpStream` и SSE

**Зачем:** push-состояние вместо поллинга — основа отзывчивости.

~~~bash
# эталон: тот же поток через curl
bash scripts/sse-watch.sh

# затем включить service bridge и смотреть лог
journalctl --user -f -o cat | grep "\[bridge\]"
~~~

**Ожидаемо:**

~~~text
[bridge] subscribe -> <handle>
[bridge] rev=1 status=paused title=Demo Track
[bridge] rev=2 status=playing title=Demo Track
[bridge] stream closed: closed          # если демон перезапустить
~~~

**Проверить разрыв:** остановить демон (Ctrl+C) — в логе `stream closed` через
несколько секунд; запустить снова — `subscribe` в течение 5 с (update-цикл).

**Записать:** формат ответа `httpStream` (объект? функция?), поведение при
разрыве, есть ли лимит длительности.

---

## S5. `noctalia.readFile` вне каталога плагина

**Зачем:** capability-ключ демона лежит в `$XDG_RUNTIME_DIR`. Если чтение
запрещено — нужен другой способ (например, ключ в `pluginDataDir`).

**Тест:** при запущенном демоне виджет `bar` должен показывать «Demo Track», а не
«ymd off». Второй тест — в `probe`: добавить и перезагрузить

~~~lua
local rt = noctalia.getenv("XDG_RUNTIME_DIR")
local ok, data = pcall(noctalia.readFile, rt .. "/ymd/session.key")
noctalia.log("[probe] session.key ok=" .. tostring(ok) .. " len=" .. tostring(type(data) == "string" and #data or -1))
~~~

**Ожидаемо:** `ok=true len=32`, виджет показывает трек.

**Если запрещено:** `ymd` при старте пишет копию ключа в
`~/.local/state/noctalia/plugins/<id>/session.key` (через `pluginDataDir`) —
записать это как решение в findings.

---

## S6. MPRIS виден шеллу

**Зачем:** если да — встроенный виджет `media` и медиа-клавиши работают без нашего
кода. Это снимает половину M1.

~~~bash
# 1. Запустить любой MPRIS-плеер (официальный клиент или mpv с --script)
playerctl --list-all
playerctl status --all

# 2. Посмотреть, видит ли шелл: виджет media в баре + Settings -> Plugins
busctl --user list | grep -i mpris
~~~

**Ожидаемо:** `playerctl` видит плеер; встроенный `media` показывает трек.
**Записать:** какие поля метаданных шелл реально отображает (title/artist/artUrl).

---

## S7. GStreamer → PipeWire

**Зачем:** backend по умолчанию — `playbin3` + `pipewiresink`.

~~~bash
# 1. Есть ли инструменты
gst-launch-1.0 --version
pactl info | grep -i "server name"

# 2. Тест звука (любой локальный файл)
gst-launch-1.0 playbin3 uri="file://$HOME/Music/test.mp3"
# ожидаемо: звук; при ошибке — см. ниже

# 3. Альтернатива
mpv --no-video "$HOME/Music/test.mp3"
~~~

**Если нет dev-пакетов:**
~~~bash
sudo dnf install gstreamer1-devel gstreamer1-plugins-base \
                 gstreamer1-plugins-good gstreamer1-plugin-libav
~~~
**Если звука нет:** проверить `pactl list short sinks` и что PipeWire запущен
(`systemctl --user status pipewire pipewire-pulse`).

---

## S8. niri IPC: события и fullscreen

**Зачем:** режим `auto` (`compact`/`hidden`) зависит от состояния окон.

~~~bash
niri msg --json focused-window | jq
niri msg --json workspaces | jq '.[0]'
niri msg --json event-stream | head -50     # в отдельном терминале

# в другом окне: переключить workspace, развернуть окно на весь экран
~~~

**Ожидаемо:** в потоке появляются `WorkspaceActivated` и `WindowFocusChanged`;
у окна есть признак `is_fullscreen` (имя поля записать точно).

**Известная особенность:** при open-then-focus `WindowFocusChanged` может не
прийти (niri issue #1889) — тогда опираться на `WorkspaceActiveWindowChanged`.

---

## S9. Свой OAuth-клиент: `response_type=code` + loopback

**Зачем:** официальный `client_id` не пустит на `127.0.0.1`; без своего клиента
остаётся только ручной импорт токена.

~~~bash
# 1. Зарегистрировать приложение: https://oauth.yandex.ru/client/new
#    Redirect URI: http://127.0.0.1:8766/callback
#    Доступы: music:content, music:read, music:write

# 2. Проверить редирект (подставить свой client_id)
xdg-open "https://oauth.yandex.ru/authorize?response_type=code&client_id=<ID>&redirect_uri=http%3A%2F%2F127.0.0.1%3A8766%2Fcallback&state=test"

# 3. Поймать код (браузер откроет callback; можно nc -l 8766)
# 4. Обменять код на токен
curl -s -X POST https://oauth.yandex.ru/token \
  -d grant_type=authorization_code -d code=<CODE> \
  -d client_id=<ID> -d client_secret=<SECRET> | jq
~~~

**Ожидаемо:** `access_token`, `refresh_token`, `expires_in`.

**Если `response_type=code` недоступен:** оставляем `ymd auth import` (вставить
URL с `#access_token`) и фиксируем как ограничение.

---

## Definition of Done для P0

- [ ] S1: версия и `plugin_api` записаны.
- [ ] S2: список `barWidget.*` записан, виджет рендерится.
- [ ] S3: `ui.*` в баре и панели работает.
- [ ] S4: SSE приходит в Luau, разрыв и реконнект обработаны.
- [ ] S5: capability-файл читается, виджет показывает трек.
- [ ] S6: MPRIS виден шеллу.
- [ ] S7: звук через GStreamer/PipeWire (или решение `backend = "mpv"`).
- [ ] S8: события niri приходят, поле fullscreen найдено.
- [ ] S9: OAuth loopback работает (или зафиксирован фоллбэк).
- [ ] `notes/p0-findings.md` заполнен, `docs/03` и `docs/04` обновлены фактами.

## Матрица отказов

| Провал | Что делаем |
|---|---|
| S2: нет нужных методов `barWidget` | виджет на `ui.*`-дереве; кнопки — через `onClick` контейнера |
| S3: `ui.*` не работает в баре | в баре только глиф+текст, прогресс — в панели |
| S4: `httpStream` не годится | поллинг `/v1/state` раз в 1–2 с (проще, но больше CPU) |
| S5: чтение runtime запрещено | ключ в `pluginDataDir`, который пишет демон |
| S7: нет GStreamer | backend `mpv --no-video --input-ipc-server` |
| S9: нет `code`-потока | `ymd auth import` + инструкция «взять токен из официального клиента» |
