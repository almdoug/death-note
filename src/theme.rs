//! Configurações de aparência e geração do tema (claro/escuro + cor de destaque).

use eframe::egui::{self, Color32, CornerRadius, Margin, Shadow, Stroke, Theme, Vec2};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ThemeMode {
    Dark,
    Light,
    System,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    pub theme: ThemeMode,
    pub accent: [u8; 3],
    /// Cor de fundo personalizada do editor (None = do tema).
    pub custom_bg: Option<[u8; 3]>,
    pub font_size: f32,
    pub line_spacing: f32,
    /// Largura máxima da coluna de texto (0 = sem limite).
    pub max_width: f32,
    pub cursor_blink: bool,
    /// Ao salvar automaticamente, grava também as notas vinculadas a arquivos.
    pub autosave_files: bool,
    pub always_on_top: bool,
    pub show_status_bar: bool,
    pub auto_lists: bool,
    /// Procurar novas versões no GitHub ao abrir o app.
    pub check_updates: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: ThemeMode::Dark,
            accent: [124, 92, 255],
            custom_bg: None,
            font_size: 16.0,
            line_spacing: 1.45,
            max_width: 0.0,
            cursor_blink: true,
            autosave_files: true,
            always_on_top: false,
            show_status_bar: true,
            auto_lists: true,
            check_updates: true,
        }
    }
}

pub const ACCENT_PRESETS: &[(&str, [u8; 3])] = &[
    ("Violeta", [124, 92, 255]),
    ("Azul", [59, 130, 246]),
    ("Ciano", [6, 182, 212]),
    ("Verde", [34, 197, 94]),
    ("Âmbar", [245, 158, 11]),
    ("Laranja", [249, 115, 22]),
    ("Vermelho", [239, 68, 68]),
    ("Rosa", [236, 72, 153]),
    ("Grafite", [120, 125, 140]),
];

/// Paleta de cores de texto/marca-texto (índice 1..=N no documento).
pub const TEXT_COLORS: &[(&str, [u8; 3], [u8; 3])] = &[
    // nome, cor no tema escuro, cor no tema claro
    ("Vermelho", [255, 107, 107], [210, 40, 40]),
    ("Laranja", [255, 159, 67], [205, 100, 10]),
    ("Amarelo", [250, 210, 80], [160, 120, 0]),
    ("Verde", [90, 210, 120], [20, 140, 60]),
    ("Ciano", [70, 205, 230], [0, 130, 160]),
    ("Azul", [100, 160, 255], [30, 90, 210]),
    ("Roxo", [175, 135, 255], [120, 60, 220]),
    ("Rosa", [250, 120, 190], [200, 40, 130]),
];

pub const HIGHLIGHTS: &[(&str, [u8; 3])] = &[
    ("Amarelo", [255, 214, 10]),
    ("Verde", [52, 211, 153]),
    ("Azul", [96, 165, 250]),
    ("Rosa", [244, 114, 182]),
    ("Laranja", [251, 146, 60]),
    ("Roxo", [167, 139, 250]),
];

/// Cores resolvidas para o tema ativo.
#[derive(Clone, Copy, Debug)]
pub struct Colors {
    pub dark: bool,
    pub bg: Color32,
    pub panel: Color32,
    pub surface: Color32,
    pub text: Color32,
    pub dim: Color32,
    pub border: Color32,
    pub accent: Color32,
    pub code_bg: Color32,
}

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

fn luminance(c: Color32) -> f32 {
    (0.2126 * c.r() as f32 + 0.7152 * c.g() as f32 + 0.0722 * c.b() as f32) / 255.0
}

impl Colors {
    pub fn resolve(settings: &Settings, dark: bool) -> Self {
        let accent = rgb(settings.accent);
        let mut c = if dark {
            Colors {
                dark,
                bg: Color32::from_rgb(28, 29, 33),
                panel: Color32::from_rgb(20, 21, 24),
                surface: Color32::from_rgb(36, 37, 42),
                text: Color32::from_rgb(230, 230, 234),
                dim: Color32::from_rgb(138, 140, 150),
                border: Color32::from_rgb(46, 47, 54),
                accent,
                code_bg: Color32::from_rgb(44, 45, 52),
            }
        } else {
            Colors {
                dark,
                bg: Color32::from_rgb(255, 255, 255),
                panel: Color32::from_rgb(242, 243, 246),
                surface: Color32::from_rgb(255, 255, 255),
                text: Color32::from_rgb(29, 30, 34),
                dim: Color32::from_rgb(110, 113, 122),
                border: Color32::from_rgb(222, 224, 229),
                accent,
                code_bg: Color32::from_rgb(240, 241, 244),
            }
        };
        if let Some(bg) = settings.custom_bg {
            let bg = rgb(bg);
            let light = luminance(bg) > 0.55;
            c.bg = bg;
            c.panel = if light {
                mix(bg, Color32::BLACK, 0.05)
            } else {
                mix(bg, Color32::BLACK, 0.28)
            };
            c.surface = if light {
                mix(bg, Color32::WHITE, 0.5)
            } else {
                mix(bg, Color32::WHITE, 0.06)
            };
            c.text = if light {
                Color32::from_rgb(29, 30, 34)
            } else {
                Color32::from_rgb(232, 232, 236)
            };
            c.dim = mix(c.text, bg, 0.45);
            c.border = mix(bg, c.text, 0.12);
            c.code_bg = mix(bg, c.text, 0.08);
        }
        c
    }

    pub fn text_color(&self, idx: u8) -> Color32 {
        match TEXT_COLORS.get(idx.wrapping_sub(1) as usize) {
            Some((_, d, l)) if idx > 0 => rgb(if self.dark { *d } else { *l }),
            _ => self.text,
        }
    }

    pub fn highlight_color(&self, idx: u8) -> Color32 {
        match HIGHLIGHTS.get(idx.wrapping_sub(1) as usize) {
            Some((_, c)) if idx > 0 => {
                let a = if self.dark { 0.32 } else { 0.45 };
                mix(self.bg, rgb(*c), a)
            }
            _ => Color32::TRANSPARENT,
        }
    }

    pub fn swatch_highlight(idx: u8) -> Color32 {
        HIGHLIGHTS
            .get(idx.wrapping_sub(1) as usize)
            .map(|(_, c)| rgb(*c))
            .unwrap_or(Color32::TRANSPARENT)
    }

    /// Cor de texto legível sobre a cor de destaque.
    pub fn on_accent(&self) -> Color32 {
        if luminance(self.accent) > 0.6 {
            Color32::from_rgb(20, 20, 24)
        } else {
            Color32::WHITE
        }
    }
}

pub fn apply(ctx: &egui::Context, settings: &Settings) {
    let pref = match settings.theme {
        ThemeMode::Dark => egui::ThemePreference::Dark,
        ThemeMode::Light => egui::ThemePreference::Light,
        ThemeMode::System => egui::ThemePreference::System,
    };
    ctx.set_theme(pref);
    for theme in [Theme::Dark, Theme::Light] {
        let c = Colors::resolve(settings, theme == Theme::Dark);
        ctx.style_mut_of(theme, |style| style_for(style, &c, settings));
    }
}

fn style_for(style: &mut egui::Style, c: &Colors, settings: &Settings) {
    let mut v = if c.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.panel_fill = c.bg;
    v.window_fill = c.surface;
    v.extreme_bg_color = c.bg;
    v.faint_bg_color = mix(c.bg, c.text, 0.04);
    v.code_bg_color = c.code_bg;
    v.hyperlink_color = c.accent;
    v.window_stroke = Stroke::new(1.0, c.border);
    v.window_corner_radius = CornerRadius::same(12);
    v.menu_corner_radius = CornerRadius::same(10);
    v.window_shadow = Shadow {
        offset: [0, 8],
        blur: 28,
        spread: 0,
        color: Color32::from_black_alpha(if c.dark { 110 } else { 40 }),
    };
    v.popup_shadow = Shadow {
        offset: [0, 4],
        blur: 16,
        spread: 0,
        color: Color32::from_black_alpha(if c.dark { 90 } else { 30 }),
    };
    v.selection.bg_fill = mix(c.bg, c.accent, if c.dark { 0.45 } else { 0.30 });
    v.selection.stroke = Stroke::new(1.0, c.text);
    v.text_cursor.stroke = Stroke::new(2.0, c.accent);
    v.text_cursor.blink = settings.cursor_blink;
    v.override_text_color = None;

    let hover = mix(c.bg, c.text, 0.08);
    let rounding = CornerRadius::same(7);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, c.text);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, c.border);
    v.widgets.noninteractive.bg_fill = c.surface;
    v.widgets.noninteractive.weak_bg_fill = c.surface;
    v.widgets.noninteractive.corner_radius = rounding;

    v.widgets.inactive.bg_fill = mix(c.surface, c.text, 0.06);
    v.widgets.inactive.weak_bg_fill = mix(c.surface, c.text, 0.05);
    v.widgets.inactive.bg_stroke = Stroke::NONE;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, c.text);
    v.widgets.inactive.corner_radius = rounding;

    v.widgets.hovered.bg_fill = hover;
    v.widgets.hovered.weak_bg_fill = hover;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, mix(c.border, c.accent, 0.5));
    v.widgets.hovered.fg_stroke = Stroke::new(1.5, c.text);
    v.widgets.hovered.corner_radius = rounding;
    v.widgets.hovered.expansion = 0.0;

    v.widgets.active.bg_fill = mix(c.surface, c.accent, 0.45);
    v.widgets.active.weak_bg_fill = mix(c.bg, c.accent, 0.35);
    v.widgets.active.bg_stroke = Stroke::new(1.0, c.accent);
    // fg_stroke do estado "active" também é a cor de `strong_text_color()`:
    // precisa ser a cor do texto, senão títulos em negrito somem no tema claro.
    v.widgets.active.fg_stroke = Stroke::new(1.5, c.text);
    v.widgets.active.corner_radius = rounding;
    v.widgets.active.expansion = 0.0;

    v.widgets.open = v.widgets.hovered;
    v.slider_trailing_fill = true;

    style.visuals = v;
    style.spacing.item_spacing = Vec2::new(8.0, 6.0);
    style.spacing.button_padding = Vec2::new(10.0, 5.0);
    style.spacing.window_margin = Margin::same(16);
    style.spacing.menu_margin = Margin::same(6);
    style.spacing.interact_size.y = 26.0;
    style.spacing.scroll.bar_width = 8.0;
    style.spacing.scroll.floating = true;
    style.interaction.selectable_labels = false;

    use egui::{FontId, TextStyle};
    style.text_styles = [
        (TextStyle::Small, FontId::proportional(11.5)),
        (TextStyle::Body, FontId::proportional(14.0)),
        (TextStyle::Button, FontId::proportional(14.0)),
        (TextStyle::Heading, FontId::proportional(20.0)),
        (TextStyle::Monospace, FontId::monospace(13.0)),
    ]
    .into();
}
