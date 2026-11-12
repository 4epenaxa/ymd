//! `ymd` — демон Яндекс Музыки (скелет P0).
//!
//! P0-цель: проверить связку «плагин Noctalia → локальный HTTP/SSE → состояние».
//! GStreamer, MPRIS и API Яндекса появятся в M1 (см. `docs/07-roadmap.md`).

mod doctor;
mod httpd;
mod session;
mod state;

use clap::{Parser, Subcommand};

use crate::state::AppState;

#[derive(Parser, Debug)]
#[command(name = "ymd", version, about = "Yandex Music daemon (P0 skeleton)")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Запустить демон (это же делает systemd --user).
    Serve {
        /// Порт локального API для плагина.
        #[arg(long, default_value_t = 8765)]
        port: u16,
    },
    /// Проверить окружение и напечатать отчёт.
    Doctor,
    /// Напечатать текущее состояние плеера.
    Status {
        #[arg(long)]
        json: bool,
        #[arg(long, default_value_t = 8765)]
        port: u16,
    },
    /// Отправить команду запущенному демону.
    Ctl {
        /// toggle | play | pause | next | prev | like | mode
        op: String,
        #[arg(long, default_value_t = 8765)]
        port: u16,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    match Cli::parse().command {
        Command::Serve { port } => {
            let key = session::ensure_key()?;
            let app = AppState::new();
            app.spawn_ticker();
            httpd::serve(app, port, key).await
        }
        Command::Doctor => {
            doctor::run();
            Ok(())
        }
        Command::Status { json, port } => status(json, port),
        Command::Ctl { op, port } => ctl(&op, port),
    }
}

fn key() -> Result<String, Box<dyn std::error::Error>> {
    session::read_key()?.ok_or_else(|| "session.key не найден — запущен ли «ymd serve»?".into())
}

fn status(json: bool, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let key = key()?;
    let response = httpd::request(port, "GET", "/v1/state", Some(&key), None)?;
    let body = httpd::body_of(&response);

    if body.trim().is_empty() {
        return Err(format!("пустой ответ: {}", httpd::status_line(&response)).into());
    }

    if json {
        println!("{}", body.trim());
        return Ok(());
    }

    let state: state::State = serde_json::from_str(body)?;
    let track = state
        .track
        .as_ref()
        .map(|track| format!("{} — {}", track.title, track.artists.join(", ")))
        .unwrap_or_else(|| "—".to_string());
    let duration = state
        .track
        .as_ref()
        .map(|track| track.duration_ms)
        .unwrap_or(0);

    println!("status   {}", state.status);
    println!("track    {track}");
    println!("position {} / {duration} ms", state.position_ms);
    println!("mode     {}", state.ui.mode);
    println!("rev      {}", state.rev);
    Ok(())
}

fn ctl(op: &str, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let key = key()?;
    let payload = serde_json::json!({ "op": op }).to_string();
    let response = httpd::request(port, "POST", "/v1/cmd", Some(&key), Some(&payload))?;
    let line = httpd::status_line(&response);

    if !line.contains(" 200 ") {
        return Err(format!("{line} {}", httpd::body_of(&response)).into());
    }

    println!("{}", httpd::body_of(&response).trim());
    Ok(())
}
