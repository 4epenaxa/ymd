//! GStreamer-бэкенд воспроизведения: `playbin3` → pipewire через autoaudiosink.
//!
//! Состояние трека (название, очередь) живёт в `state.rs`; здесь — только звук
//! и позиция. Все методы дешёвые и неблокирующие (GStreamer работает в своих
//! потоках).

use std::sync::Mutex;

use gstreamer as gst;
use gstreamer::prelude::*;

#[derive(Debug, thiserror::Error)]
pub enum PlayerError {
    #[error("gstreamer init: {0}")]
    Init(#[from] gst::glib::Error),

    #[error("не найден элемент playbin3/playbin")]
    NoElement,

    #[error("смена состояния: {0}")]
    State(#[from] gst::StateChangeError),
}

pub struct Player {
    playbin: gst::Element,
    /// Флаг EOS, выставленный из последнего опроса шины.
    eos: Mutex<bool>,
}

#[cfg(target_os = "macos")]
fn configure_macos_tls(source: &gst::Element, ca_file: &str) {
    // У souphttpsrc собственная база доверия: путь Homebrew на другом Mac
    // отсутствует. Проверка сертификатов остаётся включённой.
    if source.find_property("tls-database").is_some() {
        match gio::TlsFileDatabase::new(ca_file) {
            Ok(database) => source.set_property("tls-database", database),
            Err(error) => tracing::warn!("не удалось прочитать сертификаты из .app: {error}"),
        }
    }
}

impl Player {
    pub fn new() -> Result<Self, PlayerError> {
        gst::init()?;

        let playbin = gst::ElementFactory::make("playbin3")
            .build()
            .or_else(|_| gst::ElementFactory::make("playbin").build())
            .map_err(|_| PlayerError::NoElement)?;

        #[cfg(target_os = "macos")]
        if let Ok(ca_file) = std::env::var("YMD_CA_BUNDLE") {
            playbin.connect("source-setup", false, move |values| {
                if let Ok(source) = values[1].get::<gst::Element>() {
                    configure_macos_tls(&source, &ca_file);
                }
                None
            });
        }

        playbin.set_property("volume", 1.0f64);

        Ok(Self {
            playbin,
            eos: Mutex::new(false),
        })
    }

    /// Загружает URL и сразу начинает воспроизведение.
    pub fn load(&self, url: &str) -> Result<(), PlayerError> {
        let _ = self.playbin.set_state(gst::State::Null);
        self.playbin.set_property("uri", url);
        self.set_eos(false);
        self.playbin.set_state(gst::State::Playing)?;
        tracing::info!("player: load {url}");
        Ok(())
    }

    pub fn play(&self) -> Result<(), PlayerError> {
        self.playbin.set_state(gst::State::Playing)?;
        Ok(())
    }

    pub fn pause(&self) -> Result<(), PlayerError> {
        self.playbin.set_state(gst::State::Paused)?;
        Ok(())
    }

    pub fn toggle(&self) -> Result<(), PlayerError> {
        if self.is_playing() {
            self.pause()
        } else {
            self.play()
        }
    }

    #[allow(dead_code)]
    pub fn stop(&self) {
        let _ = self.playbin.set_state(gst::State::Null);
    }

    pub fn seek_ms(&self, position_ms: u64) {
        let position = gst::ClockTime::from_mseconds(position_ms);
        let _ = self
            .playbin
            .seek_simple(gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE, position);
    }

    pub fn position_ms(&self) -> u64 {
        self.playbin
            .query_position::<gst::ClockTime>()
            .map(|t| t.mseconds())
            .unwrap_or(0)
    }

    #[allow(dead_code)]
    pub fn duration_ms(&self) -> u64 {
        self.playbin
            .query_duration::<gst::ClockTime>()
            .map(|t| t.mseconds())
            .unwrap_or(0)
    }

    pub fn set_volume(&self, volume: f64) {
        self.playbin.set_property("volume", volume.clamp(0.0, 1.0));
    }

    #[allow(dead_code)]
    pub fn volume(&self) -> f64 {
        self.playbin.property::<f64>("volume")
    }

    pub fn is_playing(&self) -> bool {
        self.playbin.current_state() == gst::State::Playing
    }

    /// Пайплайн ещё переходит в Playing (буферизация, открытие потока).
    /// В этот момент `is_playing()` уже false, но пауза ещё не наступила.
    pub fn is_buffering(&self) -> bool {
        let (result, _current, _pending) = self.playbin.state(gst::ClockTime::ZERO);
        matches!(result, Ok(gst::StateChangeSuccess::Async))
    }

    /// Забирает сообщения шины и сообщает, дошло ли до конца потока.
    pub fn poll_eos(&self) -> bool {
        let Some(bus) = self.playbin.bus() else {
            return false;
        };

        while let Some(message) = bus.timed_pop(gst::ClockTime::ZERO) {
            match message.view() {
                gst::MessageView::Eos(..) => {
                    tracing::debug!("player: eos");
                    self.set_eos(true);
                }
                gst::MessageView::Error(err) => {
                    tracing::warn!(
                        "player error: {} ({})",
                        err.error(),
                        err.debug().unwrap_or_default()
                    );
                }
                _ => {}
            }
        }

        self.take_eos()
    }

    fn set_eos(&self, value: bool) {
        if let Ok(mut guard) = self.eos.lock() {
            *guard = value;
        }
    }

    fn take_eos(&self) -> bool {
        match self.eos.lock() {
            Ok(mut guard) => std::mem::replace(&mut *guard, false),
            Err(_) => false,
        }
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        let _ = self.playbin.set_state(gst::State::Null);
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn bundled_tls_uses_explicit_ca_without_disabling_verification() {
        gst::init().unwrap();
        let source = gst::ElementFactory::make("souphttpsrc").build().unwrap();
        let ca_file = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/macos-ca.pem");
        configure_macos_tls(&source, ca_file);
        let database = source.property::<gio::TlsDatabase>("tls-database");
        assert_eq!(database.property::<String>("anchors"), ca_file);
        assert!(source.property::<bool>("ssl-strict"));
        // Локальный файл также вызывает source-setup, но TLS-свойств не имеет.
        let file = gst::ElementFactory::make("filesrc").build().unwrap();
        configure_macos_tls(&file, ca_file);
    }
}
