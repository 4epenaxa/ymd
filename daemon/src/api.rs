//! HTTP-клиент внутреннего API Яндекс Музыки.
//!
//! Контракт непубличный, поэтому ответы разбираются толерантно:
//! * часть эндпоинтов заворачивает данные в `result`, часть — нет;
//! * неизвестные поля игнорируются.
//!
//! Схема ответа `/search` зависит от авторизации: без токена — плоский объект,
//! с токеном — `{"result": {…}}`.

use std::time::Duration;

use serde_json::Value;

pub const BASE_URL: &str = "https://api.music.yandex.ru";
pub const CLIENT_HEADER: &str = "YandexMusicDesktopApp";

const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("HTTP: {0}")]
    Http(#[from] reqwest::Error),
    #[error("API ответил {status}: {body}")]
    Status { status: u16, body: String },
    #[error("нет токена: ymd auth import --from-official")]
    NoToken,
    #[error("нет потока для трека {0}")]
    NoStream(String),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
}

pub struct Client {
    http: reqwest::Client,
    token: String,
}

impl Client {
    #[allow(dead_code)] // album/artist/playlists/likes — для M2
    pub fn new(token: String) -> Result<Self, ApiError> {
        let http = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .user_agent(concat!("ymd/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self { http, token })
    }

    /// Клиент с токеном из keyring/файла.
    pub fn from_store() -> Result<Self, ApiError> {
        let token = crate::auth::load_token().ok_or(ApiError::NoToken)?;
        Self::new(token)
    }

    async fn get(&self, path: &str, query: &[(&str, String)]) -> Result<Value, ApiError> {
        let response = self
            .http
            .get(format!("{BASE_URL}{path}"))
            .query(query)
            .header("authorization", format!("OAuth {}", self.token))
            .header("x-yandex-music-client", CLIENT_HEADER)
            .header("accept-language", "ru")
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ApiError::Status {
                status: status.as_u16(),
                body: body.chars().take(200).collect(),
            });
        }

        Ok(response.json().await?)
    }

    /// Поиск. Возвращает `result` (или корень, если обёртки нет).
    pub async fn search(&self, text: &str, page: u32) -> Result<Value, ApiError> {
        let value = self
            .get(
                "/search",
                &[
                    ("text", text.to_string()),
                    ("type", "all".to_string()),
                    ("page", page.to_string()),
                ],
            )
            .await?;
        Ok(unwrap_result(value))
    }

    pub async fn track(&self, id: &str) -> Result<Value, ApiError> {
        let value = self.get(&format!("/tracks/{id}/full-info"), &[]).await?;
        Ok(unwrap_result(value))
    }

    #[allow(dead_code)]
    pub async fn album(&self, id: &str) -> Result<Value, ApiError> {
        let value = self
            .get(&format!("/albums/{id}/with-tracks"), &[])
            .await?;
        Ok(unwrap_result(value))
    }

    #[allow(dead_code)]
    pub async fn artist(&self, id: &str) -> Result<Value, ApiError> {
        let value = self.get(&format!("/artists/{id}/info"), &[]).await?;
        Ok(unwrap_result(value))
    }

    #[allow(dead_code)]
    pub async fn playlists(&self, uid: u64) -> Result<Value, ApiError> {
        let value = self
            .get(&format!("/users/{uid}/playlists/list"), &[])
            .await?;
        Ok(unwrap_result(value))
    }

    #[allow(dead_code)]
    pub async fn likes(&self, uid: u64) -> Result<Value, ApiError> {
        let value = self.get(&format!("/users/{uid}/likes/tracks"), &[]).await?;
        Ok(unwrap_result(value))
    }

    /// Прямой URL потока.
    ///
    /// Используется `/tracks/{id}/download-info` (без подписи): он отдаёт
    /// `downloadInfoUrl`, а тот — XML с `host`/`path`. Подписанный
    /// `/resources/v2/file-info` для этого аккаунта отвечает `403 not-allowed`,
    /// поэтому оставлен только для `encraw` (офлайн).
    pub async fn stream(&self, track_id: u64, quality: Quality) -> Result<Stream, ApiError> {
        let value = unwrap_result(
            self.get(&format!("/tracks/{track_id}/download-info"), &[])
                .await?,
        );

        let entries = value.as_array().cloned().unwrap_or_default();
        let entry = pick_entry(&entries, quality).ok_or(ApiError::NoStream(track_id.to_string()))?;

        let info_url = entry
            .get("downloadInfoUrl")
            .and_then(Value::as_str)
            .ok_or(ApiError::NoStream(track_id.to_string()))?;

        let xml = self.http.get(info_url).send().await?.text().await?;
        let (host, path) = parse_download_info(&xml).ok_or(ApiError::NoStream(track_id.to_string()))?;

        Ok(Stream {
            url: format!("https://{host}{path}"),
            transport: "raw".to_string(),
            key: None,
            codec: entry
                .get("codec")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            bitrate: entry.get("bitrateInKbps").and_then(Value::as_u64),
        })
    }
}

/// Предпочтение кодеков по качеству.
fn pick_entry<'a>(entries: &'a [Value], quality: Quality) -> Option<&'a Value> {
    let wanted: &[&str] = match quality {
        Quality::Lossless => &["flac", "flac-mp4", "aac-mp4"],
        Quality::Hq => &["aac", "he-aac", "mp3"],
        Quality::Nq => &["mp3"],
        Quality::Lq => &["mp3"],
    };

    for codec in wanted {
        let mut best: Option<&Value> = None;
        for entry in entries {
            if entry.get("codec").and_then(Value::as_str) != Some(codec) {
                continue;
            }
            let bitrate = entry
                .get("bitrateInKbps")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let better = best
                .and_then(|b| b.get("bitrateInKbps").and_then(Value::as_u64))
                .map(|current| match quality {
                    // Для lossless берём максимальный, для остальных — ближайший снизу.
                    Quality::Lossless => bitrate > current,
                    _ => bitrate < current && bitrate >= target_bitrate(quality),
                })
                .unwrap_or(true);
            if better {
                best = Some(entry);
            }
        }
        if best.is_some() {
            return best;
        }
    }
    entries.first()
}

fn target_bitrate(quality: Quality) -> u64 {
    match quality {
        Quality::Lossless => 0,
        Quality::Hq => 320,
        Quality::Nq => 192,
        Quality::Lq => 128,
    }
}

/// `<download-info><host>…</host><path>…</path>…` → (host, path).
pub fn parse_download_info(xml: &str) -> Option<(String, String)> {
    Some((tag(xml, "host")?, tag(xml, "path")?))
}

fn tag(xml: &str, name: &str) -> Option<String> {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let start = xml.find(&open)? + open.len();
    let rest = &xml[start..];
    let end = rest.find(&close)?;
    Some(rest[..end].to_string())
}

fn unwrap_result(value: Value) -> Value {
    match value.get("result") {
        Some(inner) => inner.clone(),
        None => value,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Lossless,
    Hq,
    Nq,
    Lq,
}

#[allow(dead_code)] // as_str/codec нужны для encraw и логов
impl Quality {
    pub fn as_str(self) -> &'static str {
        match self {
            Quality::Lossless => "lossless",
            Quality::Hq => "hq",
            Quality::Nq => "nq",
            Quality::Lq => "lq",
        }
    }

    fn codec(self) -> &'static str {
        match self {
            Quality::Lossless => "flac",
            Quality::Hq => "aac",
            Quality::Nq => "mp3",
            Quality::Lq => "mp3",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "lossless" => Some(Quality::Lossless),
            "hq" => Some(Quality::Hq),
            "nq" => Some(Quality::Nq),
            "lq" => Some(Quality::Lq),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Stream {
    pub url: String,
    pub transport: String,
    pub key: Option<String>,
    pub codec: String,
    pub bitrate: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unwraps_result_envelope() {
        let value = serde_json::json!({"result": {"tracks": {"total": 1}}});
        assert_eq!(unwrap_result(value)["tracks"]["total"], 1);

        let flat = serde_json::json!({"tracks": {"total": 2}});
        assert_eq!(unwrap_result(flat)["tracks"]["total"], 2);
    }

    #[test]
    fn parses_download_info_xml() {
        let xml = "<?xml version=\"1.0\"?><download-info><host>mds.storage.yandex.net</host><path>/rmusic/abc/29043</path><ts>1</ts></download-info>";
        let (host, path) = parse_download_info(xml).expect("parsed");
        assert_eq!(host, "mds.storage.yandex.net");
        assert_eq!(path, "/rmusic/abc/29043");
    }

    #[test]
    fn picks_codec_per_quality() {
        let entries = vec![
            serde_json::json!({"codec": "mp3", "bitrateInKbps": 320}),
            serde_json::json!({"codec": "aac", "bitrateInKbps": 192}),
        ];
        let chosen = pick_entry(&entries, Quality::Hq).expect("entry");
        assert_eq!(chosen["codec"], "aac");
    }
}
