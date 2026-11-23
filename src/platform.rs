//! Integrações específicas do sistema operacional.

use eframe::egui::Color32;

/// Pinta a barra de título nativa do Windows 11 com as cores do tema.
#[cfg(windows)]
pub fn style_titlebar(frame: &eframe::Frame, caption: Color32, text: Color32, dark: bool) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::Graphics::Dwm::{
        DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE,
        DwmSetWindowAttribute,
    };

    let Ok(handle) = frame.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(h) = handle.as_raw() else {
        return;
    };
    let hwnd = h.hwnd.get() as windows_sys::Win32::Foundation::HWND;
    let colorref = |c: Color32| -> u32 { c.r() as u32 | (c.g() as u32) << 8 | (c.b() as u32) << 16 };
    unsafe {
        let dark_flag: i32 = dark as i32;
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
            &dark_flag as *const i32 as *const _,
            4,
        );
        let cap = colorref(caption);
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_CAPTION_COLOR as u32,
            &cap as *const u32 as *const _,
            4,
        );
        let txt = colorref(text);
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_TEXT_COLOR as u32,
            &txt as *const u32 as *const _,
            4,
        );
    }
}

#[cfg(not(windows))]
pub fn style_titlebar(_frame: &eframe::Frame, _caption: Color32, _text: Color32, _dark: bool) {}

/// Tenta trazer para frente a janela de uma instância já aberta.
#[cfg(windows)]
pub fn focus_existing_window(title: &str) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        FindWindowW, SW_RESTORE, SetForegroundWindow, ShowWindow,
    };
    let wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let hwnd = FindWindowW(std::ptr::null(), wide.as_ptr());
        if hwnd.is_null() {
            return false;
        }
        ShowWindow(hwnd, SW_RESTORE);
        SetForegroundWindow(hwnd);
    }
    true
}

#[cfg(not(windows))]
pub fn focus_existing_window(_title: &str) -> bool {
    false
}

/// Abre a pasta de dados no gerenciador de arquivos.
pub fn open_folder(path: &std::path::Path) {
    #[cfg(windows)]
    let cmd = "explorer";
    #[cfg(target_os = "macos")]
    let cmd = "open";
    #[cfg(all(unix, not(target_os = "macos")))]
    let cmd = "xdg-open";
    let _ = std::process::Command::new(cmd).arg(path).spawn();
}
