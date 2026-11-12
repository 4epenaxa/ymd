//! `ymd doctor` — что в системе есть, а чего нет. Без секретов.

use crate::session;

pub fn run() {
    println!("ymd doctor (P0 skeleton)");
    println!("{}", "-".repeat(60));

    let key_present = session::read_key().ok().flatten().is_some();
    row(
        "session key",
        key_present,
        &session::key_path().display().to_string(),
    );

    let niri = std::env::var("NIRI_SOCKET").unwrap_or_default();
    let niri_ok = !niri.is_empty() && std::os::unix::net::UnixStream::connect(&niri).is_ok();
    let niri_detail: &str = if niri.is_empty() {
        "NIRI_SOCKET не задан (не niri-сессия?)"
    } else {
        niri.as_str()
    };
    row("niri socket", niri_ok, niri_detail);

    row("playerctl", on_path("playerctl"), "проверка MPRIS");
    row("gst-launch-1.0", on_path("gst-launch-1.0"), "нужен для M1 (GStreamer)");
    row("mpv", on_path("mpv"), "альтернативный backend");
    row("secret-tool", on_path("secret-tool"), "libsecret CLI");
    row("curl", on_path("curl"), "проверки SSE");

    let port = 8765;
    let listening = std::net::TcpStream::connect(("127.0.0.1", port)).is_ok();
    row(
        "daemon :8765",
        listening,
        if listening {
            "отвечает"
        } else {
            "не запущен — нужен «ymd serve»"
        },
    );

    println!("{}", "-".repeat(60));
    println!("warn — не ошибка: часть возможностей появится в M1/M2.");
}

fn row(name: &str, ok: bool, detail: &str) {
    println!("{:<18} {:<5} {}", name, if ok { "ok" } else { "warn" }, detail);
}

fn on_path(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths)
                .map(|dir| dir.join(name))
                .any(|candidate| candidate.is_file())
        })
        .unwrap_or(false)
}
