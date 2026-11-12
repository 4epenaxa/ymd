//! Локальный HTTP API для плагина Noctalia: JSON + SSE.
//!
//! Авторизация — capability-файл `$XDG_RUNTIME_DIR/ymd/session.key` в заголовке
//! `X-YMD-Session`. Контракт: `docs/02-architecture.md`.

use std::convert::Infallible;
use std::io::{Read, Write};

use axum::{
    extract::State as AxumState,
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        Json,
    },
    routing::{get, post},
    Router,
};
use futures::{future, StreamExt};
use serde::Deserialize;
use tokio_stream::wrappers::BroadcastStream;

use crate::state::{AppState, State};

#[derive(Clone)]
struct Shared {
    app: AppState,
    key: String,
    started: std::time::Instant,
}

pub async fn serve(
    app: AppState,
    port: u16,
    key: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let shared = Shared {
        app,
        key,
        started: std::time::Instant::now(),
    };

    let router = Router::new()
        .route("/v1/health", get(health))
        .route("/v1/state", get(get_state))
        .route("/v1/cmd", post(cmd))
        .route("/v1/events", get(events))
        .with_state(shared);

    let addr = format!("127.0.0.1:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("listening on http://{addr}");
    axum::serve(listener, router).await?;
    Ok(())
}

async fn health(AxumState(shared): AxumState<Shared>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "ok": true,
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_s": shared.started.elapsed().as_secs(),
        "state_rev": shared.app.snapshot().rev,
    }))
}

async fn get_state(
    AxumState(shared): AxumState<Shared>,
    headers: HeaderMap,
) -> Result<Json<State>, StatusCode> {
    if !authorized(&headers, &shared.key) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(Json(shared.app.snapshot()))
}

#[derive(Deserialize)]
struct CmdRequest {
    op: String,
}

async fn cmd(
    AxumState(shared): AxumState<Shared>,
    headers: HeaderMap,
    Json(request): Json<CmdRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if !authorized(&headers, &shared.key) {
        return Err((
            StatusCode::UNAUTHORIZED,
            "bad or missing X-YMD-Session".to_string(),
        ));
    }

    match shared.app.command(&request.op) {
        Ok(rev) => Ok(Json(serde_json::json!({ "ok": true, "rev": rev }))),
        Err(err) => Err((StatusCode::BAD_REQUEST, err)),
    }
}

async fn events(
    AxumState(shared): AxumState<Shared>,
    headers: HeaderMap,
) -> Result<Sse<impl futures::Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    if !authorized(&headers, &shared.key) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let receiver = shared.app.subscribe();
    let stream = BroadcastStream::new(receiver).filter_map(|item| {
        future::ready(
            item.ok()
                .map(|text| Ok(Event::default().event("state").data(text))),
        )
    });

    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(std::time::Duration::from_secs(15))))
}

fn authorized(headers: &HeaderMap, key: &str) -> bool {
    headers
        .get("x-ymd-session")
        .and_then(|value| value.to_str().ok())
        .map(|value| value == key)
        .unwrap_or(false)
}

// ─── Мини-клиент для CLI (ymd status / ymd ctl) ───────────────────────────────
// Намеренно без reqwest: нужен только loopback.

pub fn request(
    port: u16,
    method: &str,
    path: &str,
    key: Option<&str>,
    body: Option<&str>,
) -> std::io::Result<String> {
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port))?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;

    let payload = body.unwrap_or("");
    let mut request = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",
        payload.len()
    );
    if let Some(key) = key {
        request.push_str(&format!("X-YMD-Session: {key}\r\n"));
    }
    request.push_str("\r\n");
    request.push_str(payload);

    stream.write_all(request.as_bytes())?;

    let mut raw = Vec::new();
    stream.read_to_end(&mut raw)?;
    Ok(String::from_utf8_lossy(&raw).into_owned())
}

pub fn status_line(response: &str) -> &str {
    response.lines().next().unwrap_or("")
}

pub fn body_of(response: &str) -> &str {
    response.split("\r\n\r\n").nth(1).unwrap_or("")
}
