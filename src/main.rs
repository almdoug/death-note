#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(deprecated)]

mod app;
mod doc;
mod editor;
mod fonts;
mod icon;
mod ipc;
mod platform;
mod storage;
mod theme;
mod updater;

use eframe::egui;
use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    // Caminhos absolutos: a outra instância pode ter outra pasta de trabalho.
    let files: Vec<PathBuf> = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .filter(|p| !p.to_string_lossy().starts_with("--"))
        .map(|p| std::fs::canonicalize(&p).unwrap_or(p))
        .map(strip_verbatim)
        .collect();

    // Instância única: se já houver uma aberta, repassa os arquivos para ela.
    let lock = storage::acquire_lock();
    if lock.is_none() {
        // A instância principal pode estar iniciando (vários arquivos abertos de uma vez).
        for _ in 0..50 {
            if ipc::send(&files) {
                platform::focus_existing_window(app::APP_TITLE);
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        if !platform::focus_existing_window(app::APP_TITLE) {
            rfd::MessageDialog::new()
                .set_title(app::APP_TITLE)
                .set_description("O Death Note já está aberto.")
                .show();
        }
        return Ok(());
    }
    let inbox = ipc::serve();

    let session = storage::load_session();

    let mut viewport = egui::ViewportBuilder::default()
        .with_title(app::APP_TITLE)
        .with_app_id("death-note")
        .with_inner_size([1100.0, 720.0])
        .with_min_inner_size([420.0, 300.0])
        .with_icon(icon())
        .with_drag_and_drop(true);
    if let Some(w) = session.as_ref().and_then(|s| s.window) {
        if w.size[0] >= 300.0 && w.size[1] >= 200.0 {
            viewport = viewport.with_inner_size(w.size);
        }
        if w.pos[0] > -10000.0 && w.pos[1] > -10000.0 {
            viewport = viewport.with_position(w.pos);
        }
        viewport = viewport.with_maximized(w.maximized);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        app::APP_TITLE,
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, session, lock, files, inbox)))),
    )
}

/// `canonicalize` no Windows devolve caminhos "verbatim" (`\\?\C:\...`); remove o prefixo.
fn strip_verbatim(p: PathBuf) -> PathBuf {
    const VERBATIM_UNC: &str = "\\\\?\\UNC\\";
    const VERBATIM: &str = "\\\\?\\";
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix(VERBATIM_UNC) {
        return PathBuf::from(format!("\\\\{rest}"));
    }
    if let Some(rest) = s.strip_prefix(VERBATIM) {
        return PathBuf::from(rest);
    }
    p
}

fn icon() -> egui::IconData {
    let size = 64;
    egui::IconData {
        rgba: icon::rgba(size),
        width: size,
        height: size,
    }
}
