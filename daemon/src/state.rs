//! Состояние плеера и команды.
//!
//! В P0 здесь «симулированный» плеер: позиция растёт по таймеру, звука нет.
//! M1 заменяет `tick()` и `command()` на GStreamer, внешний контракт (`/v1/state`)
//! при этом не меняется.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

pub const CHANNEL_CAPACITY: usize = 64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub id: String,
    pub title: String,
    pub artists: Vec<String>,
    #[serde(default)]
    pub album: Option<String>,
    pub duration_ms: u64,
    #[serde(default)]
    pub liked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Queue {
    pub length: usize,
    pub index: usize,
    pub shuffle: bool,
    pub repeat: String,
}

impl Default for Queue {
    fn default() -> Self {
        Self {
            length: 0,
            index: 0,
            shuffle: false,
            repeat: "off".to_string(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnvState {
    pub fullscreen: bool,
    pub overview: bool,
    pub idle: bool,
    pub on_battery: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiState {
    pub mode: String,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            mode: "full".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub rev: u64,
    pub status: String,
    pub track: Option<Track>,
    pub position_ms: u64,
    pub volume: f64,
    pub queue: Queue,
    pub env: EnvState,
    pub ui: UiState,
}

impl Default for State {
    fn default() -> Self {
        Self {
            rev: 0,
            status: "paused".to_string(),
            track: Some(Track {
                id: "demo".to_string(),
                title: "Demo Track".to_string(),
                artists: vec!["ymd".to_string()],
                album: None,
                duration_ms: 180_000,
                liked: false,
            }),
            position_ms: 0,
            volume: 0.8,
            queue: Queue {
                length: 3,
                ..Queue::default()
            },
            env: EnvState::default(),
            ui: UiState::default(),
        }
    }
}

/// Общий стейт демона. Дешёвый клон: внутри `Arc`.
#[derive(Clone)]
pub struct AppState {
    inner: Arc<RwLock<State>>,
    tx: broadcast::Sender<String>,
    /// true, когда состоянием управляет внешний MPRIS-плеер.
    external: Arc<AtomicBool>,
}

impl AppState {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(CHANNEL_CAPACITY);
        Self {
            inner: Arc::new(RwLock::new(State::default())),
            tx,
            external: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn snapshot(&self) -> State {
        self.inner
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }

    /// Выполняет команду и рассылает новое состояние подписчикам SSE.
    pub fn command(&self, op: &str) -> Result<u64, String> {
        // Внешний плеер: команду выполняет MPRIS, состояние подтягиваем сразу же.
        if self.external.load(Ordering::Relaxed) && crate::mpris::send(op) {
            if let Some(snapshot) = crate::mpris::poll() {
                return Ok(self.apply_mpris(snapshot));
            }
        }

        let snapshot = {
            let mut st = self
                .inner
                .write()
                .map_err(|_| "state lock poisoned".to_string())?;

            match op {
                "toggle" => {
                    st.status = if st.status == "playing" {
                        "paused".to_string()
                    } else {
                        "playing".to_string()
                    };
                }
                "play" => st.status = "playing".to_string(),
                "pause" => st.status = "paused".to_string(),
                "next" => {
                    if st.queue.length > 0 {
                        st.queue.index = (st.queue.index + 1) % st.queue.length;
                    }
                    st.position_ms = 0;
                }
                "prev" => {
                    if st.queue.length > 0 {
                        st.queue.index = (st.queue.index + st.queue.length - 1) % st.queue.length;
                    }
                    st.position_ms = 0;
                }
                "like" => {
                    if let Some(track) = st.track.as_mut() {
                        track.liked = !track.liked;
                    }
                }
                "wave" => {}
                "mode" => {
                    st.ui.mode = if st.ui.mode == "full" {
                        "compact".to_string()
                    } else {
                        "full".to_string()
                    };
                }
                other => return Err(format!("unknown op: {other}")),
            }

            st.rev += 1;
            st.clone()
        };

        self.publish(&snapshot);
        Ok(snapshot.rev)
    }

    /// Один тик «плеера» (P0: позиция без звука).
    pub fn tick(&self) {
        let snapshot = {
            let mut st = match self.inner.write() {
                Ok(guard) => guard,
                Err(_) => return,
            };

            if st.status != "playing" {
                return;
            }

            let duration = st.track.as_ref().map(|t| t.duration_ms).unwrap_or(0);
            st.position_ms = st.position_ms.saturating_add(1000);

            if duration > 0 && st.position_ms >= duration {
                st.position_ms = 0;
                if st.queue.length > 0 {
                    st.queue.index = (st.queue.index + 1) % st.queue.length;
                }
            }

            st.rev += 1;
            st.clone()
        };

        self.publish(&snapshot);
    }

    /// Применяет снимок внешнего MPRIS-плеера. Возвращает новую ревизию.
    pub fn apply_mpris(&self, snapshot: crate::mpris::Snapshot) -> u64 {
        let next = {
            let mut st = match self.inner.write() {
                Ok(guard) => guard,
                Err(_) => return 0,
            };

            let status = match snapshot.status.as_str() {
                "playing" => "playing",
                "paused" => "paused",
                _ => "stopped",
            };
            let changed = st.status != status
                || st.position_ms.abs_diff(snapshot.position_ms) > 1500
                || st
                    .track
                    .as_ref()
                    .map(|t| t.title != snapshot.title)
                    .unwrap_or(true);

            st.status = status.to_string();
            st.position_ms = snapshot.position_ms;
            st.queue.length = 0;
            st.queue.index = 0;
            st.track = Some(Track {
                id: snapshot.player.clone(),
                title: snapshot.title,
                artists: snapshot.artists,
                album: snapshot.album,
                duration_ms: snapshot.duration_ms,
                liked: false,
            });

            if changed {
                st.rev += 1;
            }
            st.clone()
        };

        self.publish(&next);
        next.rev
    }

    fn publish(&self, state: &State) {
        if let Ok(text) = serde_json::to_string(state) {
            // Ошибка отправки = нет подписчиков, это нормально.
            let _ = self.tx.send(text);
        }
    }

    pub fn spawn_ticker(&self) {
        let app = self.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(1));
            loop {
                ticker.tick().await;
                match crate::mpris::poll() {
                    Some(snapshot) => {
                        app.external.store(true, Ordering::Relaxed);
                        app.apply_mpris(snapshot);
                    }
                    None => {
                        app.external.store(false, Ordering::Relaxed);
                        app.tick();
                    }
                }
            }
        });
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
