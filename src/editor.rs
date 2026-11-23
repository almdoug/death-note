//! Ponte entre o `Doc` e o `TextEdit` do egui: um `TextBuffer` que mantém
//! os estilos alinhados com o texto, e a montagem do `LayoutJob` formatado.

use crate::doc::{
    BOLD, CHECK_DONE, CODE, CharStyle, Doc, EditKind, History, ITALIC, STRIKE, Snapshot, UNDERLINE,
};
use crate::fonts::{self, FontCaps};
use crate::theme::Colors;
use eframe::egui::{
    self, Color32, FontFamily, FontId, Stroke, TextBuffer,
    text::{CharIndex, LayoutJob, TextFormat},
};
use std::any::TypeId;
use std::ops::Range;

/// Buffer editável: dono temporário do documento durante o frame.
pub struct RichBuffer {
    pub doc: Doc,
    pub history: History,
    /// Estilo pendente para a próxima digitação (ex.: Ctrl+B sem seleção).
    pub pending: Option<(usize, CharStyle)>,
    pub now: f64,
    pub changed: bool,
    snap_taken: bool,
}

impl RichBuffer {
    pub fn new(doc: Doc, history: History, pending: Option<(usize, CharStyle)>, now: f64) -> Self {
        Self {
            doc,
            history,
            pending,
            now,
            changed: false,
            snap_taken: false,
        }
    }

    fn before_edit(&mut self, kind: EditKind, inserted: usize, at: usize) {
        if !self.snap_taken {
            self.snap_taken = true;
            if self.history.should_snapshot(self.now, kind, inserted) {
                self.history.push(Snapshot {
                    text: self.doc.text.clone(),
                    styles: self.doc.styles.clone(),
                    cursor: at,
                });
            }
        }
        self.changed = true;
    }
}

impl TextBuffer for RichBuffer {
    fn is_mutable(&self) -> bool {
        true
    }

    fn as_str(&self) -> &str {
        &self.doc.text
    }

    fn insert_text(&mut self, text: &str, char_index: CharIndex) -> usize {
        let idx = char_index.0.min(self.doc.len_chars());
        self.before_edit(EditKind::Insert, text.chars().count(), idx);
        let style = match self.pending {
            Some((p, s)) if p == idx => s,
            _ => self.doc.inherited_style(idx),
        };
        let n = self.doc.insert(idx, text, style);
        if let Some((p, _)) = &mut self.pending {
            if *p == idx {
                *p = idx + n;
            }
        }
        n
    }

    fn delete_char_range(&mut self, char_range: Range<CharIndex>) {
        if char_range.start.0 >= char_range.end.0 {
            return;
        }
        self.before_edit(EditKind::Delete, 0, char_range.start.0);
        self.doc.delete(char_range.start.0..char_range.end.0);
    }

    fn type_id(&self) -> TypeId {
        TypeId::of::<RichBuffer>()
    }
}

/// Tenta recuperar o `RichBuffer` dentro do layouter.
pub fn downcast(buf: &dyn TextBuffer) -> Option<&RichBuffer> {
    if buf.type_id() == TypeId::of::<RichBuffer>() {
        // SAFETY: o tipo concreto foi verificado via `type_id` (padrão documentado pelo egui).
        Some(unsafe { &*(buf as *const dyn TextBuffer as *const RichBuffer) })
    } else {
        None
    }
}

/// Parâmetros visuais para montar o layout.
#[derive(Clone, Copy)]
pub struct RenderCtx {
    pub colors: Colors,
    pub caps: FontCaps,
    pub size: f32,
    pub line_spacing: f32,
    pub match_bg: Color32,
    pub current_match_bg: Color32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Key {
    style: CharStyle,
    matched: u8, // 0 = não, 1 = resultado, 2 = resultado atual
    checked: bool,
}

pub fn heading_scale(h: u8) -> f32 {
    match h {
        1 => 1.75,
        2 => 1.4,
        3 => 1.18,
        _ => 1.0,
    }
}

fn format_for(k: Key, rc: &RenderCtx) -> TextFormat {
    let st = k.style;
    let c = &rc.colors;
    let size = rc.size * heading_scale(st.heading);
    let bold = st.has(BOLD) || st.heading > 0;
    let italic = st.has(ITALIC);
    let code = st.has(CODE);

    let family = if code {
        FontFamily::Monospace
    } else {
        match (bold && rc.caps.real_bold, italic && rc.caps.real_italic) {
            (true, true) => FontFamily::Name(fonts::BOLD_ITALIC.into()),
            (true, false) => FontFamily::Name(fonts::BOLD.into()),
            (false, true) => FontFamily::Name(fonts::ITALIC.into()),
            _ => FontFamily::Proportional,
        }
    };
    let font_size = if code { size * 0.9 } else { size };

    let mut color = c.text_color(st.color);
    if k.checked {
        color = c.dim;
    }
    let mut background = Color32::TRANSPARENT;
    if code {
        background = c.code_bg;
    }
    if st.highlight > 0 {
        background = c.highlight_color(st.highlight);
    }
    match k.matched {
        1 => background = rc.match_bg,
        2 => background = rc.current_match_bg,
        _ => {}
    }

    let line_w = (size / 15.0).clamp(1.0, 2.5);
    TextFormat {
        font_id: FontId::new(font_size, family),
        color,
        background,
        expand_bg: if background != Color32::TRANSPARENT { 1.0 } else { 0.0 },
        italics: italic && (!rc.caps.real_italic || code),
        underline: if st.has(UNDERLINE) {
            Stroke::new(line_w, color)
        } else {
            Stroke::NONE
        },
        strikethrough: if st.has(STRIKE) || k.checked {
            Stroke::new(line_w, color)
        } else {
            Stroke::NONE
        },
        // Trechos com fundo usam a altura natural da fonte, para o fundo
        // não ocupar a linha inteira; a altura da linha vem do texto ao redor.
        line_height: if background == Color32::TRANSPARENT {
            Some((size * rc.line_spacing).round())
        } else {
            None
        },
        ..Default::default()
    }
}

/// Monta o texto formatado. `matches` são intervalos (em chars) ordenados.
pub fn build_job(
    doc: &Doc,
    rc: &RenderCtx,
    wrap_width: f32,
    matches: &[Range<usize>],
    current: Option<usize>,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_width;
    let text = doc.text.as_str();

    if text.is_empty() {
        job.append(
            "",
            0.0,
            format_for(
                Key {
                    style: CharStyle::default(),
                    matched: 0,
                    checked: false,
                },
                rc,
            ),
        );
        return job;
    }

    let mut cur: Option<Key> = None;
    let mut run_start = 0usize;
    let mut line_start = true;
    let mut checked = false;
    let mut mi = 0usize;

    for (ci, (bi, ch)) in text.char_indices().enumerate() {
        if line_start {
            let rest = &text[bi..];
            let t = rest.trim_start_matches([' ', '\t']);
            checked = t.starts_with(CHECK_DONE);
            line_start = false;
        }
        while mi < matches.len() && matches[mi].end <= ci {
            mi += 1;
        }
        let matched = if mi < matches.len() && matches[mi].start <= ci {
            if Some(mi) == current { 2 } else { 1 }
        } else {
            0
        };
        let key = Key {
            style: doc.styles.get(ci).copied().unwrap_or_default(),
            matched,
            checked,
        };
        if cur != Some(key) {
            if let Some(k) = cur {
                job.append(&text[run_start..bi], 0.0, format_for(k, rc));
            }
            cur = Some(key);
            run_start = bi;
        }
        if ch == '\n' {
            line_start = true;
        }
    }
    if let Some(k) = cur {
        job.append(&text[run_start..], 0.0, format_for(k, rc));
    }
    job
}

pub fn set_cursor(ctx: &egui::Context, id: egui::Id, range: Range<usize>) {
    use egui::text::{CCursor, CCursorRange};
    let mut st = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
    st.cursor.set_char_range(Some(CCursorRange::two(
        CCursor::new(range.start),
        CCursor::new(range.end),
    )));
    st.store(ctx, id);
}

pub fn get_cursor(ctx: &egui::Context, id: egui::Id) -> Option<Range<usize>> {
    let st = egui::text_edit::TextEditState::load(ctx, id)?;
    let r = st.cursor.char_range()?;
    let [a, b] = r.sorted_cursors();
    Some(a.index.0..b.index.0)
}
