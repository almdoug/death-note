//! Modelo de documento rico: texto + um estilo por caractere.
//!
//! Guardar um `CharStyle` (4 bytes) por caractere é simples e robusto:
//! inserções e remoções só precisam manter os dois vetores alinhados.

use serde::{Deserialize, Serialize};
use std::ops::Range;

pub const BOLD: u8 = 1 << 0;
pub const ITALIC: u8 = 1 << 1;
pub const UNDERLINE: u8 = 1 << 2;
pub const STRIKE: u8 = 1 << 3;
pub const CODE: u8 = 1 << 4;

pub const CHECK_OPEN: char = '☐';
pub const CHECK_DONE: char = '☑';
pub const BULLET: char = '•';

/// Estilo de um caractere.
/// `color` / `highlight`: 0 = padrão, 1.. = índice na paleta.
/// `heading`: 0 = texto normal, 1..=3 = títulos.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CharStyle {
    pub flags: u8,
    pub color: u8,
    pub highlight: u8,
    pub heading: u8,
}

impl CharStyle {
    pub fn has(&self, flag: u8) -> bool {
        self.flags & flag != 0
    }
}

/// Uma operação de formatação aplicada a um intervalo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Flag(u8),
    Color(u8),
    Highlight(u8),
    Heading(u8),
    Clear,
}

#[derive(Clone, Debug, Default)]
pub struct Doc {
    pub text: String,
    pub styles: Vec<CharStyle>,
}

/// Formato serializado: estilos comprimidos em "runs" (comprimento + estilo).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct DocData {
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runs: Vec<(u32, u8, u8, u8, u8)>,
}

impl Doc {
    pub fn from_plain(text: &str) -> Self {
        let text = text.replace("\r\n", "\n");
        let n = text.chars().count();
        Self {
            text,
            styles: vec![CharStyle::default(); n],
        }
    }

    pub fn len_chars(&self) -> usize {
        self.styles.len()
    }

    pub fn to_data(&self) -> DocData {
        let mut runs: Vec<(u32, u8, u8, u8, u8)> = Vec::new();
        for s in &self.styles {
            match runs.last_mut() {
                Some(r) if (r.1, r.2, r.3, r.4) == (s.flags, s.color, s.highlight, s.heading) => {
                    r.0 += 1
                }
                _ => runs.push((1, s.flags, s.color, s.highlight, s.heading)),
            }
        }
        if runs.len() == 1 && (runs[0].1, runs[0].2, runs[0].3, runs[0].4) == (0, 0, 0, 0) {
            runs.clear();
        }
        DocData {
            text: self.text.clone(),
            runs,
        }
    }

    pub fn from_data(data: DocData) -> Self {
        let n = data.text.chars().count();
        let mut styles = Vec::with_capacity(n);
        for (len, flags, color, highlight, heading) in data.runs {
            let st = CharStyle {
                flags,
                color,
                highlight,
                heading,
            };
            for _ in 0..len {
                if styles.len() < n {
                    styles.push(st);
                }
            }
        }
        styles.resize(n, CharStyle::default());
        Self {
            text: data.text,
            styles,
        }
    }

    pub fn byte_index(&self, char_index: usize) -> usize {
        byte_index(&self.text, char_index)
    }

    pub fn char_at(&self, char_index: usize) -> Option<char> {
        self.text[self.byte_index(char_index)..].chars().next()
    }

    /// Estilo que um texto digitado em `idx` deve herdar.
    pub fn inherited_style(&self, idx: usize) -> CharStyle {
        let n = self.styles.len();
        if idx > 0 && idx <= n {
            let prev_is_newline = self.char_at(idx - 1) == Some('\n');
            if !prev_is_newline {
                return self.styles[idx - 1];
            }
        }
        // Início de linha: herda do primeiro caractere da linha, se houver.
        if idx < n && self.char_at(idx) != Some('\n') {
            return self.styles[idx];
        }
        CharStyle::default()
    }

    /// Insere texto com um estilo. Retorna quantos caracteres foram inseridos.
    pub fn insert(&mut self, idx: usize, text: &str, style: CharStyle) -> usize {
        let clean;
        let text = if text.contains('\r') {
            clean = text.replace("\r\n", "\n").replace('\r', "\n");
            clean.as_str()
        } else {
            text
        };
        let idx = idx.min(self.styles.len());
        let b = self.byte_index(idx);
        self.text.insert_str(b, text);
        let n = text.chars().count();
        self.styles
            .splice(idx..idx, std::iter::repeat_n(style, n));
        n
    }

    pub fn delete(&mut self, range: Range<usize>) {
        let end = range.end.min(self.styles.len());
        let start = range.start.min(end);
        if start == end {
            return;
        }
        let bs = self.byte_index(start);
        let be = self.byte_index(end);
        self.text.drain(bs..be);
        self.styles.drain(start..end);
    }

    /// Aplica (ou alterna) um formato no intervalo.
    pub fn apply(&mut self, range: Range<usize>, fmt: Format) {
        let end = range.end.min(self.styles.len());
        let start = range.start.min(end);
        if start == end {
            return;
        }
        // Títulos valem para a linha inteira.
        let (start, end) = if matches!(fmt, Format::Heading(_)) {
            self.line_bounds(start, end)
        } else {
            (start, end)
        };
        let slice = &mut self.styles[start..end];
        match fmt {
            Format::Flag(f) => {
                let all = slice.iter().all(|s| s.has(f));
                for s in slice.iter_mut() {
                    if all {
                        s.flags &= !f
                    } else {
                        s.flags |= f
                    }
                }
            }
            Format::Color(c) => slice.iter_mut().for_each(|s| s.color = c),
            Format::Highlight(h) => slice.iter_mut().for_each(|s| s.highlight = h),
            Format::Heading(h) => {
                let all = slice.iter().all(|s| s.heading == h);
                let v = if all { 0 } else { h };
                slice.iter_mut().for_each(|s| s.heading = v);
            }
            Format::Clear => slice.iter_mut().for_each(|s| *s = CharStyle::default()),
        }
    }

    /// Se todos os caracteres do intervalo têm a flag.
    pub fn range_has(&self, range: Range<usize>, flag: u8) -> bool {
        let r = self.clamp(range);
        !r.is_empty() && self.styles[r].iter().all(|s| s.has(flag))
    }

    pub fn range_heading(&self, range: Range<usize>) -> Option<u8> {
        let r = self.clamp(range);
        let first = self.styles.get(r.start)?.heading;
        self.styles[r].iter().all(|s| s.heading == first).then_some(first)
    }

    fn clamp(&self, range: Range<usize>) -> Range<usize> {
        let end = range.end.min(self.styles.len());
        range.start.min(end)..end
    }

    /// Expande [start, end) para linhas completas (sem incluir o '\n' final).
    pub fn line_bounds(&self, start: usize, end: usize) -> (usize, usize) {
        let chars: Vec<char> = self.text.chars().collect();
        let mut s = start.min(chars.len());
        while s > 0 && chars[s - 1] != '\n' {
            s -= 1;
        }
        let mut e = end.max(s).min(chars.len());
        if e > s && e <= chars.len() && chars[e - 1] == '\n' && e - 1 >= start {
            e -= 1;
        }
        while e < chars.len() && chars[e] != '\n' {
            e += 1;
        }
        (s, e)
    }

    /// Índices (em chars) de início de cada linha tocada pelo intervalo.
    pub fn line_starts(&self, start: usize, end: usize) -> Vec<usize> {
        let (s, e) = self.line_bounds(start, end);
        let mut out = vec![s];
        for (i, c) in self.text.chars().enumerate().skip(s).take(e - s) {
            if c == '\n' {
                out.push(i + 1);
            }
        }
        out
    }

    /// Prefixo de lista no início da linha que começa em `line_start`.
    pub fn list_prefix_at(&self, line_start: usize) -> Option<String> {
        let b = self.byte_index(line_start);
        let line = self.text[b..].split('\n').next().unwrap_or("");
        list_prefix(line).map(|s| s.to_string())
    }

    pub fn word_count(&self) -> usize {
        self.text.split_whitespace().count()
    }

    // ---------------------------------------------------------------- Markdown

    pub fn to_markdown(&self) -> String {
        let mut out = String::with_capacity(self.text.len() + 64);
        let chars: Vec<char> = self.text.chars().collect();
        let mut i = 0;
        let mut line_start = true;
        while i < chars.len() {
            if chars[i] == '\n' {
                out.push('\n');
                i += 1;
                line_start = true;
                continue;
            }
            if line_start {
                let h = self.styles[i].heading;
                if h > 0 {
                    out.push_str(&"#".repeat(h as usize));
                    out.push(' ');
                }
                // Listas e checklists
                let rest: String = chars[i..].iter().take(2).collect();
                if rest == format!("{CHECK_OPEN} ") {
                    out.push_str("- [ ] ");
                    i += 2;
                } else if rest == format!("{CHECK_DONE} ") {
                    out.push_str("- [x] ");
                    i += 2;
                } else if rest == format!("{BULLET} ") {
                    out.push_str("- ");
                    i += 2;
                }
                line_start = false;
                continue;
            }
            // run de mesmo estilo até o fim da linha
            let st = self.styles[i];
            let mut j = i;
            while j < chars.len() && chars[j] != '\n' && self.styles[j].flags == st.flags {
                j += 1;
            }
            let run: String = chars[i..j].iter().collect();
            let (lead, body, trail) = split_ws(&run);
            out.push_str(lead);
            if !body.is_empty() {
                let mut open = String::new();
                if st.has(CODE) {
                    open.push('`');
                } else {
                    if st.has(BOLD) && st.heading == 0 {
                        open.push_str("**");
                    }
                    if st.has(ITALIC) {
                        open.push('*');
                    }
                    if st.has(STRIKE) {
                        open.push_str("~~");
                    }
                    if st.has(UNDERLINE) {
                        open.push_str("<u>");
                    }
                }
                let close: String = if st.has(CODE) {
                    "`".into()
                } else {
                    let mut c = String::new();
                    if st.has(UNDERLINE) {
                        c.push_str("</u>");
                    }
                    if st.has(STRIKE) {
                        c.push_str("~~");
                    }
                    if st.has(ITALIC) {
                        c.push('*');
                    }
                    if st.has(BOLD) && st.heading == 0 {
                        c.push_str("**");
                    }
                    c
                };
                out.push_str(&open);
                out.push_str(body);
                out.push_str(&close);
            }
            out.push_str(trail);
            i = j;
        }
        out
    }

    /// Importa um subconjunto de Markdown: títulos, **negrito**, *itálico*,
    /// ~~tachado~~, `código`, <u>sublinhado</u>, listas e checklists.
    pub fn from_markdown(md: &str) -> Self {
        let md = md.replace("\r\n", "\n");
        let mut doc = Doc::default();
        for (li, line) in md.split('\n').enumerate() {
            if li > 0 {
                let n = doc.len_chars();
                doc.insert(n, "\n", CharStyle::default());
            }
            let mut heading = 0u8;
            let mut rest = line;
            for (h, p) in [(3u8, "### "), (2, "## "), (1, "# ")] {
                if let Some(r) = line.strip_prefix(p) {
                    heading = h;
                    rest = r;
                    break;
                }
            }
            let trimmed = rest.trim_start();
            let indent = &rest[..rest.len() - trimmed.len()];
            let mut prefix = String::new();
            let mut body = rest;
            for (p, rep) in [
                ("- [ ] ", format!("{CHECK_OPEN} ")),
                ("- [x] ", format!("{CHECK_DONE} ")),
                ("- [X] ", format!("{CHECK_DONE} ")),
                ("- ", format!("{BULLET} ")),
                ("* ", format!("{BULLET} ")),
            ] {
                if let Some(r) = trimmed.strip_prefix(p) {
                    prefix = format!("{indent}{rep}");
                    body = r;
                    break;
                }
            }
            let base = CharStyle {
                heading,
                ..Default::default()
            };
            if !prefix.is_empty() {
                let n = doc.len_chars();
                doc.insert(n, &prefix, base);
            }
            parse_inline(&mut doc, body, base);
        }
        doc
    }
}

fn parse_inline(doc: &mut Doc, s: &str, base: CharStyle) {
    let mut flags = 0u8;
    let mut i = 0;
    let bytes = s.as_bytes();
    let mut buf = String::new();
    let flush = |doc: &mut Doc, buf: &mut String, flags: u8| {
        if !buf.is_empty() {
            let n = doc.len_chars();
            let st = CharStyle {
                flags: flags | base.flags,
                ..base
            };
            doc.insert(n, buf, st);
            buf.clear();
        }
    };
    while i < s.len() {
        let rest = &s[i..];
        let toggles: [(&str, u8); 7] = [
            ("**", BOLD),
            ("__", BOLD),
            ("~~", STRIKE),
            ("<u>", UNDERLINE),
            ("</u>", UNDERLINE),
            ("`", CODE),
            ("*", ITALIC),
        ];
        let mut matched = false;
        for (tok, f) in toggles {
            if rest.starts_with(tok) {
                // Só trata como marcador se houver um par mais adiante (ou se estiver fechando).
                let closing = flags & f != 0;
                let close_tok = if tok == "<u>" { "</u>" } else { tok };
                if tok == "</u>" && !closing {
                    break;
                }
                if !closing && !rest[tok.len()..].contains(close_tok) {
                    break;
                }
                flush(doc, &mut buf, flags);
                flags ^= f;
                i += tok.len();
                matched = true;
                break;
            }
        }
        if !matched {
            let c = rest.chars().next().unwrap();
            buf.push(c);
            i += c.len_utf8();
        }
        let _ = bytes;
    }
    flush(doc, &mut buf, flags);
}

fn split_ws(s: &str) -> (&str, &str, &str) {
    let t = s.trim_start();
    let lead = &s[..s.len() - t.len()];
    let body = t.trim_end();
    let trail = &t[body.len()..];
    (lead, body, trail)
}

pub fn byte_index(s: &str, char_index: usize) -> usize {
    s.char_indices()
        .nth(char_index)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}

/// Reconhece prefixos de lista: "• ", "- ", "☐ ", "☑ ", "1. " (com indentação).
pub fn list_prefix(line: &str) -> Option<&str> {
    let trimmed = line.trim_start_matches([' ', '\t']);
    let indent = line.len() - trimmed.len();
    for p in ["• ", "- ", "☐ ", "☑ ", "* "] {
        if trimmed.starts_with(p) {
            return Some(&line[..indent + p.len()]);
        }
    }
    let digits = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && digits < 4 && trimmed[digits..].starts_with(". ") {
        return Some(&line[..indent + digits + 2]);
    }
    None
}

/// Próximo prefixo de lista (incrementa numeração, desmarca checklist).
pub fn next_prefix(prefix: &str) -> String {
    let trimmed = prefix.trim_start_matches([' ', '\t']);
    let indent = &prefix[..prefix.len() - trimmed.len()];
    if trimmed.starts_with(CHECK_DONE) {
        return format!("{indent}{CHECK_OPEN} ");
    }
    let digits: String = trimmed.chars().take_while(|c| c.is_ascii_digit()).collect();
    if let Ok(n) = digits.parse::<u32>() {
        return format!("{indent}{}. ", n + 1);
    }
    prefix.to_string()
}

// -------------------------------------------------------------------- Undo

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EditKind {
    Insert,
    Delete,
    Other,
}

pub struct Snapshot {
    pub text: String,
    pub styles: Vec<CharStyle>,
    pub cursor: usize,
}

#[derive(Default)]
pub struct History {
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    last_time: f64,
    last_kind: Option<EditKind>,
    chars_since: usize,
}

const MAX_UNDO: usize = 200;
const MAX_UNDO_BYTES: usize = 24 * 1024 * 1024;

impl History {
    /// Decide se o estado atual (antes da edição) deve virar um ponto de desfazer.
    pub fn should_snapshot(&mut self, now: f64, kind: EditKind, inserted: usize) -> bool {
        let snap = self.last_kind != Some(kind)
            || kind == EditKind::Other
            || now - self.last_time > 1.0
            || self.chars_since > 40
            || inserted > 1;
        self.last_time = now;
        self.last_kind = Some(kind);
        if snap {
            self.chars_since = 0;
        }
        self.chars_since += inserted.max(1);
        snap
    }

    pub fn push(&mut self, snap: Snapshot) {
        self.redo.clear();
        self.undo.push(snap);
        self.trim();
    }

    fn trim(&mut self) {
        let mut bytes: usize = self
            .undo
            .iter()
            .map(|s| s.text.len() + s.styles.len() * 4)
            .sum();
        while self.undo.len() > MAX_UNDO || (bytes > MAX_UNDO_BYTES && self.undo.len() > 1) {
            let s = self.undo.remove(0);
            bytes -= s.text.len() + s.styles.len() * 4;
        }
    }

    pub fn break_group(&mut self) {
        self.last_kind = None;
    }

    pub fn undo(&mut self, doc: &mut Doc, cursor: usize) -> Option<usize> {
        let snap = self.undo.pop()?;
        self.redo.push(Snapshot {
            text: std::mem::take(&mut doc.text),
            styles: std::mem::take(&mut doc.styles),
            cursor,
        });
        doc.text = snap.text;
        doc.styles = snap.styles;
        self.last_kind = None;
        Some(snap.cursor)
    }

    pub fn redo(&mut self, doc: &mut Doc, cursor: usize) -> Option<usize> {
        let snap = self.redo.pop()?;
        self.undo.push(Snapshot {
            text: std::mem::take(&mut doc.text),
            styles: std::mem::take(&mut doc.styles),
            cursor,
        });
        doc.text = snap.text;
        doc.styles = snap.styles;
        self.last_kind = None;
        Some(snap.cursor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_delete_keep_alignment() {
        let mut d = Doc::from_plain("olá mundo");
        d.insert(4, "grande ", CharStyle::default());
        assert_eq!(d.text, "olá grande mundo");
        assert_eq!(d.styles.len(), d.text.chars().count());
        d.delete(0..4);
        assert_eq!(d.text, "grande mundo");
        assert_eq!(d.styles.len(), d.text.chars().count());
    }

    #[test]
    fn toggle_bold() {
        let mut d = Doc::from_plain("abc");
        d.apply(0..2, Format::Flag(BOLD));
        assert!(d.range_has(0..2, BOLD));
        assert!(!d.range_has(0..3, BOLD));
        d.apply(0..2, Format::Flag(BOLD));
        assert!(!d.range_has(0..1, BOLD));
    }

    #[test]
    fn roundtrip_data() {
        let mut d = Doc::from_plain("ação rápida");
        d.apply(0..4, Format::Flag(ITALIC));
        d.apply(5..11, Format::Color(3));
        let back = Doc::from_data(d.to_data());
        assert_eq!(back.text, d.text);
        assert_eq!(back.styles, d.styles);
    }

    #[test]
    fn markdown_roundtrip() {
        let md = "# Título\nUm **negrito** e *itálico* e `code`\n- [ ] tarefa\n- item";
        let d = Doc::from_markdown(md);
        assert!(d.text.starts_with("Título\nUm negrito"));
        assert_eq!(d.styles[0].heading, 1);
        assert_eq!(d.to_markdown(), md.replace("- item", "- item"));
    }

    #[test]
    fn heading_applies_to_line() {
        let mut d = Doc::from_plain("linha um\nlinha dois");
        d.apply(2..3, Format::Heading(1));
        assert_eq!(d.styles[0].heading, 1);
        assert_eq!(d.styles[7].heading, 1);
        assert_eq!(d.styles[9].heading, 0);
    }

    #[test]
    fn list_prefixes() {
        assert_eq!(list_prefix("• item"), Some("• "));
        assert_eq!(list_prefix("  12. x"), Some("  12. "));
        assert_eq!(next_prefix("  12. "), "  13. ");
        assert_eq!(next_prefix("☑ "), "☐ ");
        assert_eq!(list_prefix("texto"), None);
    }

    #[test]
    fn inherit_at_line_start() {
        let mut d = Doc::from_plain("ab\n");
        d.apply(0..2, Format::Heading(1));
        assert_eq!(d.inherited_style(3).heading, 0);
        assert_eq!(d.inherited_style(2).heading, 1);
    }
}
