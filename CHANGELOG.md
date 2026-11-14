# Журнал изменений

Формат — [Keep a Changelog](https://keepachangelog.com/ru/1.1.0/),
версии — [SemVer](https://semver.org/lang/ru/). Теги — `vMAJOR.MINOR.PATCH`.

Разделы: **Добавлено**, **Изменено**, **Исправлено**, **Удалено**.
Релизы и собранные пакеты — на странице
[Releases](https://github.com/pbolkhovitin/ymd/releases).

## [Unreleased]

## [0.2.1] - 2026-11-14

### Добавлено

- `scripts/version.sh`: единый источник версии (`Cargo.toml`), сверка
  манифестов, `--set`/`--bump`/`--tag`.
- `CHANGELOG.md` в формате Keep a Changelog.
- CI (`.github/workflows/ci.yml`): версии, документация, тесты в Docker,
  сборка архива плагина.
- Релизный workflow (`.github/workflows/release.yml`): проверка тега, тесты,
  `.deb` демона/плазмоида/`ymd-qt`, архив плагина Noctalia, GitHub Release
  с заметками из этого файла.
- Шаблоны `bug_report`, `feature_request`, `config.yml` и `PULL_REQUEST_TEMPLATE.md`.
- Бейджи CI, релиза и лицензии в `README.md`.

## [0.2.0] - 2026-11-14

### Добавлено

- Демон `ymd`: OAuth Device Flow и импорт токена из официального клиента,
  токен в GNOME Keyring, capability-ключ `0600` в `$XDG_RUNTIME_DIR/ymd`.
- GStreamer `playbin3` → PipeWire, исходящий MPRIS `org.mpris.MediaPlayer2.ymd`,
  резервный мост к внешним плеерам через `playerctl`.
- API-клиент: поиск, треки, альбомы, исполнители, главная, discovery, радио,
  лайки, тексты песен, «Моя волна» (rotor) с автодогрузкой.
- Поток: подписанный `/get-file-info` (пара «заголовок клиента + ключ»),
  качества `lossless`/`hq`/`nq`, фоллбэк на `hq`, настройка в
  `~/.config/ymd/config.toml`.
- Офлайн-кэш `encraw` + AES-CTR, LRU-лимит, докачка после обрыва
  (`.part` + `Range` + отпечаток ключа).
- Логирование в `~/.local/state/ymd/ymd.log` с ротацией, `ymd logs [-f]`,
  структурные события очереди и EOS.
- systemd user-юнит `ymd.service`, `scripts/install-user-service.sh`.
- Клиенты: плазмоид Plasma 6, приложение `ymd-qt` (Qt 6/Kirigami),
  плагин Noctalia (бар, панель, bridge, тексты, обложки).
- Пакеты `.deb` (демон, плазмоид, `ymd-qt`) и архив плагина Noctalia.

### Исправлено

- Зацикливание одного трека: расчёт следующего индекса вынесен в общий
  `plan_step()`, индекс передаётся в команду — состояние и звук совпадают
  при `repeat`/`shuffle`.
- Виджет Noctalia залипал в «ymd off» после рестарта демона (`rev`
  не монотонен между запусками); «off» теперь только после трёх ошибок.
- Висячий симлинк плагина Noctalia после переименования репозитория:
  `ymd doctor` проверяет ссылку, установщик пересоздаёт её.

[Unreleased]: https://github.com/pbolkhovitin/ymd/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/pbolkhovitin/ymd/releases/tag/v0.2.0
