#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(deprecated)]

mod app;
mod doc;
mod editor;
mod fonts;
mod platform;
mod storage;
mod theme;

use eframe::egui;
use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    let files: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();

    // Impede duas instâncias de sobrescreverem a mesma sessão.
    let lock = storage::acquire_lock();
    if lock.is_none() {
        if !platform::focus_existing_window(app::APP_TITLE) {
            rfd::MessageDialog::new()
                .set_title(app::APP_TITLE)
                .set_description("O Death Note já está aberto.")
                .show();
        }
        return Ok(());
    }

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
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, session, lock, files)))),
    )
}

/// Ícone gerado em código: um caderno escuro com marcador violeta.
fn icon() -> egui::IconData {
    let n = 64usize;
    let mut rgba = vec![0u8; n * n * 4];
    for y in 0..n {
        for x in 0..n {
            let i = (y * n + x) * 4;
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            // cantos arredondados
            let r = 12.0f32;
            let cx = fx.clamp(4.0 + r, 60.0 - r);
            let cy = fy.clamp(2.0 + r, 62.0 - r);
            let d = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt();
            if d > r || fx < 4.0 || fx > 60.0 || fy < 2.0 || fy > 62.0 {
                continue;
            }
            let mut c = [28u8, 29, 33, 255];
            if fx < 14.0 {
                c = [124, 92, 255, 255]; // lombada
            }
            // linhas do caderno
            if fx > 20.0 && fx < 52.0 && (fy as i32 - 20) >= 0 && (fy as i32 - 20) % 9 < 2 && fy < 52.0 {
                c = [150, 152, 165, 255];
            }
            // marcador
            if fx > 42.0 && fx < 50.0 && fy < 22.0 && !(fy > 16.0 && (fx - 46.0).abs() < (fy - 16.0)) {
                c = [239, 68, 68, 255];
            }
            rgba[i..i + 4].copy_from_slice(&c);
        }
    }
    egui::IconData {
        rgba,
        width: n as u32,
        height: n as u32,
    }
}
