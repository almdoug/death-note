//! Carrega fontes do sistema (Segoe UI no Windows, DejaVu/Noto no Linux)
//! para ter negrito/itálico reais. Se nada for encontrado, usa as fontes
//! embutidas do egui e simula o itálico.

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};
use std::sync::Arc;

pub const BOLD: &str = "dn-bold";
pub const ITALIC: &str = "dn-italic";
pub const BOLD_ITALIC: &str = "dn-bold-italic";

#[derive(Clone, Copy, Debug, Default)]
pub struct FontCaps {
    pub real_bold: bool,
    pub real_italic: bool,
}

fn candidates(kind: &str) -> &'static [&'static str] {
    #[cfg(windows)]
    {
        match kind {
            "regular" => &["segoeui.ttf"],
            "bold" => &["segoeuib.ttf", "seguisb.ttf"],
            "italic" => &["segoeuii.ttf"],
            "bold_italic" => &["segoeuiz.ttf"],
            "mono" => &["CascadiaMono.ttf", "consola.ttf"],
            "symbol" => &["seguisym.ttf"],
            _ => &[],
        }
    }
    #[cfg(target_os = "macos")]
    {
        match kind {
            "regular" => &["/System/Library/Fonts/Supplemental/Arial.ttf"],
            "bold" => &["/System/Library/Fonts/Supplemental/Arial Bold.ttf"],
            "italic" => &["/System/Library/Fonts/Supplemental/Arial Italic.ttf"],
            "bold_italic" => &["/System/Library/Fonts/Supplemental/Arial Bold Italic.ttf"],
            "mono" => &["/System/Library/Fonts/Menlo.ttc"],
            "symbol" => &["/System/Library/Fonts/Apple Symbols.ttf"],
            _ => &[],
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        match kind {
            "regular" => &[
                "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
                "/usr/share/fonts/TTF/DejaVuSans.ttf",
                "/usr/share/fonts/noto/NotoSans-Regular.ttf",
            ],
            "bold" => &[
                "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
                "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
                "/usr/share/fonts/noto/NotoSans-Bold.ttf",
            ],
            "italic" => &[
                "/usr/share/fonts/truetype/dejavu/DejaVuSans-Oblique.ttf",
                "/usr/share/fonts/TTF/DejaVuSans-Oblique.ttf",
                "/usr/share/fonts/noto/NotoSans-Italic.ttf",
            ],
            "bold_italic" => &[
                "/usr/share/fonts/truetype/dejavu/DejaVuSans-BoldOblique.ttf",
                "/usr/share/fonts/TTF/DejaVuSans-BoldOblique.ttf",
                "/usr/share/fonts/noto/NotoSans-BoldItalic.ttf",
            ],
            "mono" => &[
                "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
                "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
            ],
            "symbol" => &[
                "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
                "/usr/share/fonts/TTF/DejaVuSans.ttf",
            ],
            _ => &[],
        }
    }
}

fn load(kind: &str) -> Option<Vec<u8>> {
    for name in candidates(kind) {
        #[cfg(windows)]
        let paths = {
            let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
            let mut v = vec![std::path::PathBuf::from(&windir).join("Fonts").join(name)];
            if let Ok(local) = std::env::var("LOCALAPPDATA") {
                v.push(
                    std::path::PathBuf::from(local)
                        .join("Microsoft\\Windows\\Fonts")
                        .join(name),
                );
            }
            v
        };
        #[cfg(not(windows))]
        let paths = vec![std::path::PathBuf::from(name)];
        for p in paths {
            if let Ok(bytes) = std::fs::read(&p) {
                return Some(bytes);
            }
        }
    }
    None
}

pub fn install(ctx: &egui::Context) -> FontCaps {
    let mut defs = FontDefinitions::default();
    let mut caps = FontCaps::default();

    let fallback: Vec<String> = defs
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();

    let add = |defs: &mut FontDefinitions, key: &str, kind: &str| -> bool {
        if let Some(bytes) = load(kind) {
            defs.font_data
                .insert(key.to_owned(), Arc::new(FontData::from_owned(bytes)));
            true
        } else {
            false
        }
    };

    let has_regular = add(&mut defs, "dn-regular", "regular");
    let has_symbol = add(&mut defs, "dn-symbol", "symbol");
    caps.real_bold = add(&mut defs, BOLD, "bold");
    caps.real_italic = add(&mut defs, ITALIC, "italic");
    let has_bi = add(&mut defs, BOLD_ITALIC, "bold_italic");
    let has_mono = add(&mut defs, "dn-mono", "mono");

    let chain = |primary: Option<&str>| -> Vec<String> {
        let mut v = Vec::new();
        if let Some(p) = primary {
            v.push(p.to_owned());
        }
        if has_regular && primary != Some("dn-regular") {
            v.push("dn-regular".to_owned());
        }
        if has_symbol {
            v.push("dn-symbol".to_owned());
        }
        v.extend(fallback.iter().cloned());
        v
    };

    let regular = chain(has_regular.then_some("dn-regular"));
    let bold = chain(caps.real_bold.then_some(BOLD));
    let italic = chain(caps.real_italic.then_some(ITALIC));
    let bold_italic = chain(if has_bi {
        Some(BOLD_ITALIC)
    } else if caps.real_bold {
        Some(BOLD)
    } else {
        None
    });

    defs.families.insert(FontFamily::Proportional, regular);
    defs.families
        .insert(FontFamily::Name(BOLD.into()), bold);
    defs.families
        .insert(FontFamily::Name(ITALIC.into()), italic);
    defs.families
        .insert(FontFamily::Name(BOLD_ITALIC.into()), bold_italic);

    if has_mono {
        let mono = defs.families.entry(FontFamily::Monospace).or_default();
        mono.insert(0, "dn-mono".to_owned());
        if has_symbol {
            mono.push("dn-symbol".to_owned());
        }
    }

    ctx.set_fonts(defs);
    caps
}
