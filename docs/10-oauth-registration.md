# 10. Регистрация OAuth-приложения в Яндекс ID

По официальной документации (`yandex.ru/dev/id/doc/ru/register-auth`), проверено 2026-11-12.

## Что нужно сделать

1. Войти на **https://oauth.yandex.ru/** под своим аккаунтом.
2. Создать приложение: кнопка **«Создать приложение»** → тип
   **«Для авторизации пользователей»** (или сразу https://oauth.yandex.ru/client/new/id/).

   > Тип приложения **нельзя поменять** после регистрации. «Для доступа к API»
   > (только свои данные, без группы `login`) нам не подходит — нужен вход от
   > имени пользователя.

3. Заполнить:
   - **Название сервиса** — например `ymd — Yandex Music for niri`;
   - иконка (≤ 1 МБ, необязательно);
   - **Контактная почта** — рабочая.
4. **Платформы** → выбрать **Веб-сервисы**.
5. **Redirect URI** — ровно:

   ```
   http://127.0.0.1:8765/callback
   ```

   Можно добавить второй адрес (запасной порт):

   ```
   http://127.0.0.1:8787/callback
   ```

   Правила совпадения (важно): должны совпадать **схема, хост, порт и путь**.
   Query и fragment могут отличаться. `localhost` и `127.0.0.1` — **разные** хосты;
   регистрируйте именно `127.0.0.1`.

6. **Доступ к API** (разрешения). Нужны три scope из группы `music`:

   ```
   music:content
   music:read
   music:write
   ```

7. Сохранить и открыть свойства приложения. Записать в `~/.config/ymd/config.toml`:

   ```toml
   [auth]
   client_id     = "<ClientID>"
   client_secret = "<ClientSecret>"   # можно не использовать при PKCE
   redirect_uri  = "http://127.0.0.1:8765/callback"
   ```

   `client_secret` — секрет: хранить в GNOME Keyring (`secret-tool`), не в git.

## Как это работает (факты из документации)

Запрос кода:

```
https://oauth.yandex.ru/authorize
  ?response_type=code
  &client_id=<ClientID>
  &redirect_uri=http://127.0.0.1:8765/callback
  &scope=music:content music:read music:write
  &state=<случайная строка>
  &code_challenge=<base64url(sha256(verifier))>
  &code_challenge_method=S256
  [&device_id=<uuid>]
  [&device_name=<имя>]
```

- PKCE поддерживается: `code_challenge_method` — `S256` (предпочтительно) или `plain`.
- Код подтверждения живёт **10 минут**.
- Ответ: `http://127.0.0.1:8765/callback?code=<code>&state=<state>`.

Обмен кода на токен:

```bash
curl -s -X POST https://oauth.yandex.ru/token \
  -H "Content-Type: application/x-www-form-urlencoded" \
  -d grant_type=authorization_code \
  -d code="<code>" \
  -d client_id="<ClientID>" \
  -d code_verifier="<verifier>"
```

- При передаче `code_verifier` **client_secret не требуется**.
- Срок жизни токена зависит от запрошенных разрешений (в ответе есть `expires_in`;
  может прийти `refresh_token`).

## Риск, который надо проверить сразу после регистрации

Публичная документация **Яндекс Музыка API** больше недоступна (страницы
`yandex.ru/dev/music/...` отдают 404), поэтому нельзя заранее подтвердить, что
группа `music:*` доступна новым приложениям.

Проверка занимает минуту: в свойствах приложения открыть **«Доступ к API»** и
посмотреть, есть ли группа «Яндекс Музыка».

| Исход | Что делаем |
|---|---|
| группа `music:*` есть | регистрируем scope, идём по loopback-потоку (S9) |
| группы нет | остаётся импорт токена из официального клиента (`ymd auth import`): `response_type=token`, `client_id 97fe03033fa34407ac9bcf91d5afed5b`, токен приходит во фрагменте URL |

## Чек-лист

- [ ] приложение создано, тип — «Для авторизации пользователей»
- [ ] платформа «Веб-сервисы», Redirect URI `http://127.0.0.1:8765/callback`
- [ ] scope `music:content`, `music:read`, `music:write` выбраны
- [ ] `client_id` записан в конфиг, `client_secret` — в keyring
- [ ] проверено, что `response_type=code` не отклоняется (`unauthorized_client`
      означает модерацию/блокировку приложения)

## Источники

- https://yandex.ru/dev/id/doc/ru/register-client
- https://yandex.ru/dev/id/doc/ru/register-auth
- https://yandex.ru/dev/id/doc/ru/codes/code-url
