# M1 — факты по API и воспроизведению

Дата: 2026-11-12. Всё ниже проверено вручную на реальном токене.

## Что работает

| Запрос | Результат |
|---|---|
| `GET /search?text=…&type=all&page=0` | 200, с токеном — `{"result":{…}}`, без токена — плоский объект |
| `GET /tracks/{id}/full-info` | 200 |
| `GET /albums/{id}/with-tracks` | 200 |
| `GET /artists/{id}/info` | 200 |
| `GET /account/about` | 200, `login=pbolkhovitin uid=12967407 plus=true` |

Реализовано в `daemon/src/api.rs`, CLI: `ymd search`, `ymd track`, `ymd album`,
`ymd artist`, `ymd stream`.

## Что не работает (блокер)

### 1. `file-info` → 403 `not-allowed`

```
GET /resources/v2/file-info?ts=…&trackId=…&quality=…&codecs=…&transports=raw&sign=…
→ 403 {"result":{"name":"track-download-info-error","message":"not-allowed"}}
```

Проверено:

- `/resources/v2/file-info` и `/get-file-info` — оба 403;
- все четыре `secretKey` (linux/web/win/mac) — 403;
- качества `nq/mp3`, `hq/aac`, `lossless/flac` — 403;
- с `x-yandex-music-device` и с `x-yandex-music-without-invocation-info: 1` — 403;
- подпись собрана точно как в рендерере официального клиента:
  `btoa(HMAC-SHA256(secretKey, ts+trackId+quality+codecs+transports)).slice(0,-1)`
  (найдено в `_next/static/chunks/9616-*.js` и `app/(product)/layout-*.js`).

Вывод: сервер отказывает не из-за подписи, а из-за клиента/аккаунта
(вероятно, нужна аттестация клиента — `x-ya-service-ticket`/`x-ya-user-ticket`).

### 2. `download-info` → XML → 401

```
GET /tracks/{id}/download-info → 200, downloadInfoUrl
GET <downloadInfoUrl> → 200, XML: <host> <path> <ts> <region> <s>
GET https://<host><path> → 401
```

Варианты `?ts=`, `?ts=&s=`, `?ts=&region=`, `?s=`, с `Authorization`,
с `x-yandex-music-client`, с `Range` — все 401 (host `mds.storage.yandex.net`).

## Решение для M1

Пока прямой поток недоступен, бар управляет **любым MPRIS-плеером**
(включая официальный Яндекс Музыка) через `playerctl`:

- демон читает `status`, `metadata` (`title`, `artist`, `artUrl`, `mpris:length`),
  `position` раз в секунду;
- команды `toggle/next/prev/seek` транслируются в `playerctl`;
- когда поток появится, MPRIS станет исходящим (демон сам станет плеером),
  контракт `/v1/state` и плагин не меняются.

Это даёт работающий бар уже сейчас и не блокирует M2.
