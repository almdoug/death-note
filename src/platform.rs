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
/// O título é "Death Note" ou "<nota> — Death Note".
#[cfg(windows)]
pub fn focus_existing_window(title: &str) -> bool {
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextW, IsIconic, IsWindowVisible, SW_RESTORE,
        SetForegroundWindow, ShowWindow,
    };

    struct Search {
        exact: String,
        suffix: String,
        found: HWND,
    }

    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> i32 {
        let search = unsafe { &mut *(lparam as *mut Search) };
        if unsafe { IsWindowVisible(hwnd) } == 0 {
            return 1;
        }
        let mut buf = [0u16; 512];
        let len = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
        if len <= 0 {
            return 1;
        }
        let text = String::from_utf16_lossy(&buf[..len as usize]);
        if text == search.exact || text.ends_with(&search.suffix) {
            search.found = hwnd;
            return 0; // para a enumeração
        }
        1
    }

    let mut search = Search {
        exact: title.to_string(),
        suffix: format!(" — {title}"),
        found: std::ptr::null_mut(),
    };
    unsafe {
        EnumWindows(Some(visit), &mut search as *mut Search as LPARAM);
        if search.found.is_null() {
            return false;
        }
        // Só restaura se estiver minimizada (SW_RESTORE tiraria uma janela do modo maximizado).
        if IsIconic(search.found) != 0 {
            ShowWindow(search.found, SW_RESTORE);
        }
        SetForegroundWindow(search.found);
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

/// Abre um endereço no navegador padrão.
pub fn open_url(url: &str) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let _ = std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
    }
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}
