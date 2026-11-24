//! Ícones Phosphor (https://phosphoricons.com, licença MIT).
//!
//! `assets/phosphor-icons.ttf` é um subconjunto da fonte "Phosphor Regular"
//! só com os ícones abaixo (~13 KB). Para acrescentar um ícone, inclua o
//! código dele no comando de `fonttools subset` descrito no CLAUDE.md.

use eframe::egui::{FontFamily, FontId};

pub const FAMILY: &str = "dn-icons";
pub const FONT_BYTES: &[u8] = include_bytes!("../assets/phosphor-icons.ttf");

pub fn font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(FAMILY.into()))
}

pub const LIST: &str = "\u{E2F0}";
pub const GEAR: &str = "\u{E272}";
pub const PUSH_PIN: &str = "\u{E3E2}";
pub const MOON: &str = "\u{E330}";
pub const SUN: &str = "\u{E472}";
pub const CIRCLE_HALF: &str = "\u{E18C}";
pub const FILE_TEXT: &str = "\u{E23A}";
pub const CHECK: &str = "\u{E182}";
pub const WARNING: &str = "\u{E4E0}";
pub const ARROW_CIRCLE_UP: &str = "\u{E030}";
pub const DOWNLOAD: &str = "\u{E20C}";
pub const SEARCH: &str = "\u{E30C}";
pub const CARET_UP: &str = "\u{E13C}";
pub const CARET_DOWN: &str = "\u{E136}";
pub const CARET_LEFT: &str = "\u{E138}";
pub const X: &str = "\u{E4F6}";
pub const BOLD: &str = "\u{E5BE}";
pub const ITALIC: &str = "\u{E5C0}";
pub const UNDERLINE: &str = "\u{E5C4}";
pub const STRIKE: &str = "\u{E5C2}";
pub const CODE: &str = "\u{E1BC}";
pub const H1: &str = "\u{E6BC}";
pub const H2: &str = "\u{E6BE}";
pub const H3: &str = "\u{E6C0}";
pub const TEXT_COLOR: &str = "\u{ED34}";
pub const HIGHLIGHTER: &str = "\u{EC76}";
pub const CHECKLIST: &str = "\u{E186}";
pub const BULLETS: &str = "\u{E2F2}";
pub const MARKDOWN: &str = "\u{E508}";
pub const CLEAR_FORMAT: &str = "\u{E488}";
pub const NOTEBOOK: &str = "\u{E34E}";
pub const PLUS: &str = "\u{E3D4}";
pub const FOLDER_OPEN: &str = "\u{E256}";
pub const FLOPPY: &str = "\u{E248}";
pub const PENCIL: &str = "\u{E3B4}";
pub const SPLIT_H: &str = "\u{E870}";
pub const SPLIT_V: &str = "\u{E874}";
pub const MERGE: &str = "\u{E09A}";
pub const HISTORY: &str = "\u{E1A0}";
pub const FOLDER: &str = "\u{E24A}";
pub const REVEAL: &str = "\u{E5DE}";
pub const COPY: &str = "\u{E1CA}";
pub const FILE_PLUS: &str = "\u{E236}";
