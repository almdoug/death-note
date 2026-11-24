//! Aplicativo: abas com divisão de tela, editor rico, barra flutuante de
//! formatação, busca, troca rápida de notas, configurações e autosalvamento.

use crate::doc::{
    self, BOLD, BULLET, CHECK_DONE, CHECK_OPEN, CODE, CharStyle, Doc, EditKind, Format, History,
    ITALIC, STRIKE, Snapshot, UNDERLINE,
};
use crate::editor::{self, RenderCtx, RichBuffer};
use crate::fonts::{self, FontCaps};
use crate::icons;
use crate::platform;
use crate::{ipc, updater};
use std::sync::mpsc::Receiver;
use crate::storage::{self, InstanceLock, NoteData, NoteId, SessionData, WindowGeom};
use crate::theme::{self, ACCENT_PRESETS, Colors, HIGHLIGHTS, Settings, TEXT_COLORS, ThemeMode};
use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, Event, FontFamily, FontId, Frame, Id, Key,
    Margin, Modifiers, Order, Pos2, Rect, RichText, Sense, Stroke, TextBuffer, Theme, Ui, Vec2,
    WidgetText, pos2, vec2,
};
use egui_dock::{
    AllowedSplits, DockArea, DockState, Node, NodePath, Split, TabDestination,
    TabInsert, TabViewer,
};
use egui_dock::tab_viewer::OnCloseResponse;
use std::collections::HashMap;
use std::ops::Range;
use std::path::PathBuf;
use std::time::Duration;

pub const APP_TITLE: &str = "Death Note";
const MAX_CLOSED: usize = 50;

// ===================================================================== Nota

pub struct Note {
    pub id: NoteId,
    pub title: String,
    pub custom_title: bool,
    pub doc: Doc,
    pub file: Option<PathBuf>,
    pub file_dirty: bool,
    pub created: i64,
    pub modified: i64,
    pub history: History,
    pub pending: Option<(usize, CharStyle)>,
    pub words: usize,
    pub cursor: usize,
    pub restore_cursor: bool,
}

fn now_ts() -> i64 {
    chrono::Local::now().timestamp()
}

impl Note {
    fn new(id: NoteId, doc: Doc) -> Self {
        let mut n = Self {
            id,
            title: String::new(),
            custom_title: false,
            doc,
            file: None,
            file_dirty: false,
            created: now_ts(),
            modified: now_ts(),
            history: History::default(),
            pending: None,
            words: 0,
            cursor: 0,
            restore_cursor: false,
        };
        n.refresh();
        n
    }

    fn from_data(d: NoteData) -> Self {
        let mut n = Self {
            id: d.id,
            title: d.title,
            custom_title: d.custom_title,
            doc: Doc::from_data(d.doc),
            file: d.file,
            file_dirty: d.file_dirty,
            created: d.created,
            modified: d.modified,
            history: History::default(),
            pending: None,
            words: 0,
            cursor: d.cursor,
            restore_cursor: true,
        };
        n.refresh();
        n
    }

    fn to_data(&self) -> NoteData {
        NoteData {
            id: self.id,
            title: self.title.clone(),
            custom_title: self.custom_title,
            doc: self.doc.to_data(),
            file: self.file.clone(),
            file_dirty: self.file_dirty,
            created: self.created,
            modified: self.modified,
            cursor: self.cursor,
        }
    }

    /// Recalcula título automático e contagem de palavras.
    fn refresh(&mut self) {
        self.words = self.doc.word_count();
        if !self.custom_title {
            self.title = auto_title(&self.doc.text, self.file.as_ref(), self.created);
        }
    }

    fn touch(&mut self) {
        self.modified = now_ts();
        if self.file.is_some() {
            self.file_dirty = true;
        }
        self.refresh();
    }

    fn snapshot(&mut self, cursor: usize) {
        self.history.push(Snapshot {
            text: self.doc.text.clone(),
            styles: self.doc.styles.clone(),
            cursor,
        });
        self.history.break_group();
        let _ = self.history.should_snapshot(0.0, EditKind::Other, 0);
    }
}

fn auto_title(text: &str, file: Option<&PathBuf>, created: i64) -> String {
    let first = text
        .lines()
        .map(|l| {
            l.trim()
                .trim_start_matches([CHECK_OPEN, CHECK_DONE, BULLET, '#', '-', '*'])
                .trim()
        })
        .find(|l| !l.is_empty());
    match first {
        Some(l) => {
            let mut s: String = l.chars().take(28).collect();
            if l.chars().count() > 28 {
                s.push('…');
            }
            s
        }
        None => file
            .and_then(|f| f.file_stem())
            .and_then(|s| s.to_str())
            .map(|s| s.to_owned())
            .unwrap_or_else(|| untitled_label(created)),
    }
}

/// "Nova nota · 22:05" (ou "· 23/11" se foi criada em outro dia), para
/// distinguir várias notas vazias abertas ao mesmo tempo.
fn untitled_label(created: i64) -> String {
    use chrono::TimeZone;
    match chrono::Local.timestamp_opt(created, 0).single() {
        Some(t) if t.date_naive() == chrono::Local::now().date_naive() => {
            format!("Nova nota · {}", t.format("%H:%M"))
        }
        Some(t) => format!("Nova nota · {}", t.format("%d/%m")),
        None => "Nova nota".into(),
    }
}

/// Corta títulos longos para as abas não ocuparem a barra inteira.
fn short_title(t: &str) -> String {
    const MAX: usize = 28;
    if t.chars().count() > MAX {
        let mut s: String = t.chars().take(MAX - 1).collect();
        s = s.trim_end().to_string();
        s.push('…');
        s
    } else {
        t.to_string()
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    let num = thousands(n);
    if n == 1 { format!("{num} {one}") } else { format!("{num} {many}") }
}

fn thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push('.');
        }
        out.push(ch);
    }
    out
}

fn bold(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text).font(FontId::new(size, FontFamily::Name(fonts::BOLD.into())))
}

/// Moldura padrão dos modais (margem de 20px, cantos de 14px).
fn modal(id: &str, c: &Colors, ctx: &egui::Context) -> egui::Modal {
    egui::Modal::new(Id::new(id))
        .frame(
            Frame::new()
                .fill(c.surface)
                .stroke(Stroke::new(1.0, c.border))
                .corner_radius(CornerRadius::same(14))
                .inner_margin(Margin::same(20))
                .shadow(ctx.global_style().visuals.window_shadow),
        )
        .backdrop_color(Color32::from_black_alpha(if c.dark { 150 } else { 90 }))
}

/// Título do modal com botão de fechar alinhado à direita.
fn modal_header(ui: &mut Ui, title: &str, c: &Colors) -> bool {
    let mut close = false;
    ui.horizontal(|ui| {
        ui.label(bold(title, 18.0).color(c.text));
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            if icon_button(ui, icons::X, "Fechar (Esc)", c).clicked() {
                close = true;
            }
        });
    });
    ui.add_space(10.0);
    close
}

fn primary_button(text: &str, c: &Colors) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.to_string()).color(c.on_accent()))
        .fill(c.accent)
        .min_size(vec2(0.0, 30.0))
}

fn editor_id(id: NoteId) -> Id {
    Id::new(("dn-editor", id))
}

// ================================================================ UI state

#[derive(Clone, Copy, PartialEq, Eq)]
enum ToolbarMenu {
    None,
    Color,
    Highlight,
}

struct FindState {
    note: NoteId,
    query: String,
    matches: Vec<Range<usize>>,
    current: usize,
    focus: bool,
    stale: bool,
    scroll: bool,
}

struct Switcher {
    query: String,
    selected: usize,
    focus: bool,
}

struct UiState {
    toolbar_menu: ToolbarMenu,
    toolbar_rect: Option<(NoteId, Rect)>,
    find: Option<FindState>,
    switcher: Option<Switcher>,
    settings_open: bool,
    rename: Option<(NoteId, String, bool)>,
    toast: Option<(String, f64)>,
    focus_editor: Option<NoteId>,
    settings_was_open: bool,
    settings_fresh: bool,
    confirm_reset: bool,
}

impl UiState {
    fn toast(&mut self, ctx: &egui::Context, msg: impl Into<String>) {
        let now = ctx.input(|i| i.time);
        self.toast = Some((msg.into(), now + 2.8));
    }
}

enum Action {
    New,
    NewIn(NodePath),
    Close(NoteId),
    Closed(NoteId),
    Reopen(usize),
    Open,
    OpenPath(PathBuf),
    Save(NoteId),
    SaveAs(NoteId),
    Rename(NoteId),
    Duplicate(NoteId),
    Split(NoteId, Split),
    Unsplit,
    Focus(NoteId),
    Reveal(NoteId),
}

// ===================================================================== App

pub struct App {
    ctx: egui::Context,
    notes: HashMap<NoteId, Note>,
    dock: DockState<NoteId>,
    closed: Vec<NoteData>,
    next_id: NoteId,
    settings: Settings,
    caps: FontCaps,
    ui: UiState,
    actions: Vec<Action>,
    dirty_since: Option<f64>,
    last_change: f64,
    settings_dirty: bool,
    last_saved: Option<chrono::DateTime<chrono::Local>>,
    save_error: Option<String>,
    titlebar: Option<(Color32, bool)>,
    active: Option<NoteId>,
    applied_on_top: bool,
    _lock: Option<InstanceLock>,
    inbox: Option<Receiver<ipc::Message>>,
    update: updater::Shared,
    update_open: bool,
    last_update_check: f64,
    window_title: String,
}

fn translations() -> egui_dock::Translations {
    let mut t = egui_dock::Translations::english();
    t.tab_context_menu.close_button = "Fechar".into();
    t.tab_context_menu.eject_button = "Destacar".into();
    t.tab_context_menu.hide_tab_bar_button = "Ocultar barra de abas".into();
    t.tab_context_menu.show_tab_bar_button = "Mostrar barra de abas".into();
    t.leaf.close_button_disabled_tooltip = "Esta aba não pode ser fechada".into();
    t.leaf.close_all_button = "Fechar todas".into();
    t.accessibility.close_button = "Fechar aba".into();
    t.accessibility.add_button = "Nova nota".into();
    t
}

const WELCOME: &str = "# Bem-vindo ao Death Note
Um bloco de notas **rápido**, *leve* e que nunca perde nada.

## O básico
- Selecione um texto para ver a **barra de formatação** flutuante.
- Arraste uma aba para a lateral para **dividir a tela**. Arraste de volta para desfazer.
- Tudo é salvo automaticamente, até se o PC desligar de repente.
- Abas fechadas vão para *Fechadas recentemente* (`Ctrl+Shift+T` reabre).

## Atalhos
- `Ctrl+T` nova nota · `Ctrl+W` fechar · `Ctrl+Tab` próxima aba
- `Ctrl+B` negrito · `Ctrl+I` itálico · `Ctrl+U` sublinhado · `Ctrl+E` código
- `Ctrl+1` `Ctrl+2` `Ctrl+3` títulos · `Ctrl+L` checklist · `F5` data e hora
- `Ctrl+F` buscar · `Ctrl+P` busca rápida entre notas · `Ctrl+S` salvar em arquivo
- `Ctrl` + roda do mouse muda o tamanho do texto

## Checklist
- [ ] Clique na caixinha para marcar
- [x] Tarefas concluídas ficam riscadas";

impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        session: Option<SessionData>,
        lock: Option<InstanceLock>,
        files: Vec<PathBuf>,
        inbox: Option<Receiver<ipc::Message>>,
    ) -> Self {
        let ctx = cc.egui_ctx.clone();
        let caps = fonts::install(&ctx);
        let settings = storage::load_settings();
        theme::apply(&ctx, &settings);
        ctx.options_mut(|o| o.zoom_with_keyboard = false);

        let mut notes = HashMap::new();
        let mut next_id = 1;
        let mut closed = Vec::new();
        let mut dock = None;

        if let Some(s) = session {
            next_id = s.next_id.max(1);
            for d in s.notes {
                next_id = next_id.max(d.id + 1);
                notes.insert(d.id, Note::from_data(d));
            }
            closed = s.closed;
            for c in &closed {
                next_id = next_id.max(c.id + 1);
            }
            dock = s.dock;
        }

        if notes.is_empty() && dock.is_none() {
            let mut d = Doc::from_markdown(WELCOME);
            if let Some(b) = d.text.find("nunca perde nada") {
                let s = d.text[..b].chars().count();
                d.apply(s..s + 16, Format::Highlight(1));
            }
            let mut n = Note::new(next_id, d);
            n.restore_cursor = false;
            notes.insert(next_id, n);
            next_id += 1;
        }

        // Valida o layout salvo contra as notas existentes.
        // Um layout salvo sem nenhuma aba (todas fechadas) volta sem a árvore
        // da superfície principal, e o egui_dock entra em pânico ao usá-lo.
        // Nesse caso, ou se o layout estiver inválido, começa um layout novo.
        let mut dock = dock
            .filter(|d| {
                d.get_surface(egui_dock::SurfaceIndex::main()).is_some()
                    && d.iter_all_tabs().next().is_some()
            })
            .unwrap_or_else(|| DockState::new(vec![]));
        dock.retain_tabs(|id| notes.contains_key(id));
        if dock.iter_all_tabs().next().is_none() {
            dock = DockState::new(vec![]);
        }
        let mut in_dock: Vec<NoteId> = dock.iter_all_tabs().map(|(_, id)| *id).collect();
        in_dock.sort();
        let mut missing: Vec<NoteId> = notes
            .keys()
            .filter(|id| in_dock.binary_search(id).is_err())
            .copied()
            .collect();
        missing.sort();
        for id in missing {
            dock.main_surface_mut().push_to_focused_leaf(id);
        }
        dock.translations = translations();

        let mut app = Self {
            ctx,
            notes,
            dock,
            closed,
            next_id,
            settings,
            caps,
            ui: UiState {
                toolbar_menu: ToolbarMenu::None,
                toolbar_rect: None,
                find: None,
                switcher: None,
                settings_open: false,
                rename: None,
                toast: None,
                focus_editor: None,
                settings_was_open: false,
                settings_fresh: false,
                confirm_reset: false,
            },
            actions: files.into_iter().map(Action::OpenPath).collect(),
            dirty_since: None,
            last_change: 0.0,
            settings_dirty: false,
            last_saved: None,
            save_error: None,
            titlebar: None,
            active: None,
            applied_on_top: false,
            _lock: lock,
            inbox,
            update: updater::new_shared(),
            update_open: false,
            last_update_check: 0.0,
            window_title: String::new(),
        };
        app.active = app.focused_note();
        app.ui.focus_editor = app.active;
        let _ = ipc::CTX.set(app.ctx.clone());
        if app.settings.check_updates {
            updater::check(&app.update, false);
        }
        app
    }

    fn colors(&self) -> Colors {
        Colors::resolve(&self.settings, self.ctx.theme() == Theme::Dark)
    }

    fn mark_dirty(&mut self) {
        let now = self.ctx.input(|i| i.time);
        self.dirty_since.get_or_insert(now);
        self.last_change = now;
    }

    fn toast(&mut self, msg: impl Into<String>) {
        let ctx = self.ctx.clone();
        self.ui.toast(&ctx, msg);
    }

    fn focused_note(&mut self) -> Option<NoteId> {
        self.dock.find_active_focused().map(|(_, id)| *id).or_else(|| {
            self.dock
                .iter_all_tabs()
                .map(|(_, id)| *id)
                .next()
        })
    }

    fn alloc_id(&mut self) -> NoteId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn add_note(&mut self, note: Note, at: Option<NodePath>) -> NoteId {
        let id = note.id;
        self.notes.insert(id, note);
        match at {
            Some(path) if self.dock.leaf(path).is_ok() => {
                if let Ok(leaf) = self.dock.leaf_mut(path) {
                    leaf.append_tab(id);
                }
            }
            _ => self.dock.main_surface_mut().push_to_focused_leaf(id),
        }
        self.focus(id);
        self.mark_dirty();
        id
    }

    fn focus(&mut self, id: NoteId) {
        if let Some(path) = self.dock.find_tab(&id) {
            let _ = self.dock.set_active_tab(path);
            self.dock.set_focused_node_and_surface(path.node_path());
        }
        self.active = Some(id);
        self.ui.focus_editor = Some(id);
    }

    // ------------------------------------------------------------ ações

    fn process_actions(&mut self) {
        let actions = std::mem::take(&mut self.actions);
        for a in actions {
            match a {
                Action::New => {
                    let id = self.alloc_id();
                    self.add_note(Note::new(id, Doc::default()), None);
                }
                Action::NewIn(path) => {
                    let id = self.alloc_id();
                    self.add_note(Note::new(id, Doc::default()), Some(path));
                }
                Action::Close(id) => {
                    if let Some(path) = self.dock.find_tab(&id) {
                        self.dock.remove_tab(path);
                    }
                    self.after_close(id);
                }
                Action::Closed(id) => self.after_close(id),
                Action::Reopen(i) => {
                    if i < self.closed.len() {
                        let d = self.closed.remove(i);
                        let mut n = Note::from_data(d);
                        if self.notes.contains_key(&n.id) {
                            n.id = self.alloc_id();
                        }
                        let title = n.title.clone();
                        self.add_note(n, None);
                        self.toast(format!("Reaberta: {}", short_title(&title)));
                    }
                }
                Action::Open => {
                    if let Some(paths) = rfd::FileDialog::new()
                        .set_title("Abrir anotação")
                        .add_filter("Anotações", &["dnote", "md", "markdown", "txt"])
                        .add_filter("Todos os arquivos", &["*"])
                        .pick_files()
                    {
                        for p in paths {
                            self.open_path(p);
                        }
                    }
                }
                Action::OpenPath(p) => self.open_path(p),
                Action::Save(id) => self.save_note(id, false),
                Action::SaveAs(id) => self.save_note(id, true),
                Action::Rename(id) => {
                    if let Some(n) = self.notes.get(&id) {
                        self.ui.rename = Some((id, n.title.clone(), true));
                    }
                }
                Action::Duplicate(id) => {
                    if let Some(n) = self.notes.get(&id) {
                        let doc = n.doc.clone();
                        let title = format!("{} (cópia)", n.title);
                        let new_id = self.alloc_id();
                        let mut note = Note::new(new_id, doc);
                        note.title = title;
                        note.custom_title = true;
                        let at = self.dock.find_tab(&id).map(|p| p.node_path());
                        self.add_note(note, at);
                    }
                }
                Action::Split(id, split) => self.split(id, split),
                Action::Unsplit => {
                    let tabs: Vec<NoteId> = self.dock.iter_all_tabs().map(|(_, t)| *t).collect();
                    let active = self.active;
                    let mut d = DockState::new(tabs);
                    d.translations = translations();
                    self.dock = d;
                    if let Some(a) = active {
                        self.focus(a);
                    }
                    self.mark_dirty();
                }
                Action::Focus(id) => self.focus(id),
                Action::Reveal(id) => match self.notes.get(&id).and_then(|n| n.file.clone()) {
                    Some(path) => platform::reveal_in_explorer(&path),
                    None => self.toast("Esta nota ainda não está salva em arquivo. Use Ctrl+S."),
                },
            }
        }
    }

    fn after_close(&mut self, id: NoteId) {
        if let Some(n) = self.notes.remove(&id) {
            if !n.doc.text.trim().is_empty() {
                self.closed.insert(0, n.to_data());
                self.closed.truncate(MAX_CLOSED);
                self.toast("Nota fechada  ·  Ctrl+Shift+T reabre");
            }
        }
        if self.ui.find.as_ref().is_some_and(|f| f.note == id) {
            self.ui.find = None;
        }
        if self.active == Some(id) {
            self.active = self.focused_note();
            self.ui.focus_editor = self.active;
        }
        self.mark_dirty();
    }

    fn split(&mut self, id: NoteId, split: Split) {
        let Some(path) = self.dock.find_tab(&id) else {
            return;
        };
        let count = self
            .dock
            .leaf(path.node_path())
            .map(|l| l.len())
            .unwrap_or(0);
        if count > 1 {
            self.dock.move_tab(
                path,
                TabDestination::Node(path.node_path(), TabInsert::Split(split)),
            );
            self.focus(id);
        } else {
            let new_id = self.alloc_id();
            self.notes.insert(new_id, Note::new(new_id, Doc::default()));
            self.dock
                .split(path.node_path(), split, 0.5, Node::leaf(new_id));
            self.focus(new_id);
        }
        self.mark_dirty();
    }

    fn open_path(&mut self, p: PathBuf) {
        // Já aberto? Só foca.
        if let Some(id) = self
            .notes
            .values()
            .find(|n| n.file.as_ref() == Some(&p))
            .map(|n| n.id)
        {
            self.focus(id);
            return;
        }
        match storage::read_file(&p) {
            Ok((title, doc)) => {
                let id = self.alloc_id();
                let mut n = Note::new(id, doc);
                n.file = Some(p.clone());
                if storage::kind_of(&p) == storage::FileKind::Dnote {
                    n.title = title;
                    n.custom_title = true;
                }
                n.refresh();
                // Se a nota ativa estiver vazia e sem arquivo, substitui.
                let replace = self.active.and_then(|a| {
                    let an = self.notes.get(&a)?;
                    (an.doc.text.is_empty() && an.file.is_none()).then_some(a)
                });
                let at = replace.and_then(|r| self.dock.find_tab(&r).map(|t| t.node_path()));
                self.add_note(n, at);
                if let Some(r) = replace {
                    self.actions.push(Action::Close(r));
                }
                self.toast(format!("Aberto: {}", file_name(&p)));
            }
            Err(e) => self.toast(format!("Não foi possível abrir: {e}")),
        }
    }

    fn save_note(&mut self, id: NoteId, save_as: bool) {
        let Some(n) = self.notes.get(&id) else {
            return;
        };
        let path = match (&n.file, save_as) {
            (Some(p), false) => Some(p.clone()),
            _ => {
                let name = sanitize(&n.title);
                rfd::FileDialog::new()
                    .set_title("Salvar anotação")
                    .set_file_name(format!("{name}.dnote"))
                    .add_filter("Death Note (com formatação)", &["dnote"])
                    .add_filter("Markdown", &["md"])
                    .add_filter("Texto simples", &["txt"])
                    .save_file()
            }
        };
        let Some(path) = path else { return };
        let n = self.notes.get_mut(&id).unwrap();
        match storage::write_file(&path, &n.title, &n.doc) {
            Ok(()) => {
                n.file = Some(path.clone());
                n.file_dirty = false;
                n.refresh();
                self.toast(format!("Salvo em {}", file_name(&path)));
                self.mark_dirty();
            }
            Err(e) => self.toast(format!("Erro ao salvar: {e}")),
        }
    }

    // ------------------------------------------------------- persistência

    fn session_data(&self) -> SessionData {
        let mut order: Vec<NoteId> = self.dock.iter_all_tabs().map(|(_, id)| *id).collect();
        for id in self.notes.keys() {
            if !order.contains(id) {
                order.push(*id);
            }
        }
        let window = self.ctx.input(|i| {
            let vp = i.viewport();
            let maximized = vp.maximized.unwrap_or(false);
            match (vp.outer_rect, vp.inner_rect) {
                (Some(o), Some(inner)) => Some(WindowGeom {
                    pos: [o.min.x, o.min.y],
                    size: [inner.width(), inner.height()],
                    maximized,
                }),
                _ => None,
            }
        });
        SessionData {
            version: 1,
            next_id: self.next_id,
            notes: order
                .iter()
                .filter_map(|id| self.notes.get(id))
                .map(|n| n.to_data())
                .collect(),
            dock: Some(self.dock.clone()),
            closed: self.closed.clone(),
            window,
        }
    }

    fn save_now(&mut self) {
        if self.settings.autosave_files {
            let mut errors = Vec::new();
            for n in self.notes.values_mut() {
                if let (Some(p), true) = (&n.file, n.file_dirty) {
                    match storage::write_file(p, &n.title, &n.doc) {
                        Ok(()) => n.file_dirty = false,
                        Err(e) => errors.push(e),
                    }
                }
            }
            if let Some(e) = errors.pop() {
                self.save_error = Some(e);
            }
        }
        match storage::save_session(&self.session_data()) {
            Ok(()) => {
                self.last_saved = Some(chrono::Local::now());
                self.save_error = None;
            }
            Err(e) => self.save_error = Some(e),
        }
        if self.settings_dirty {
            storage::save_settings(&self.settings);
            self.settings_dirty = false;
        }
        self.dirty_since = None;
    }

    fn autosave(&mut self, now: f64) {
        if let Some(since) = self.dirty_since {
            let idle = now - self.last_change;
            let waited = now - since;
            if idle >= 0.8 || waited >= 4.0 {
                self.save_now();
            } else {
                let wait = (0.8 - idle).min(4.0 - waited).max(0.05);
                self.ctx.request_repaint_after(Duration::from_secs_f64(wait));
            }
        }
    }

    // --------------------------------------------------------- entrada

    fn global_input(&mut self) {
        let ctx = self.ctx.clone();
        // Arquivos arrastados para a janela
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect()
        });
        for p in dropped {
            self.actions.push(Action::OpenPath(p));
        }

        let cmd = Modifiers::COMMAND;
        let cmd_shift = Modifiers::COMMAND | Modifiers::SHIFT;
        let active = self.active.or_else(|| self.focused_note());

        if take_key(&ctx, cmd_shift, Key::T) {
            if !self.closed.is_empty() {
                self.actions.push(Action::Reopen(0));
            }
        }
        if take_key(&ctx, cmd, Key::T) || take_key(&ctx, cmd, Key::N) {
            self.actions.push(Action::New);
        }
        if take_key(&ctx, cmd, Key::W) {
            if let Some(id) = active {
                self.actions.push(Action::Close(id));
            }
        }
        if take_key(&ctx, cmd_shift, Key::S) {
            if let Some(id) = active {
                self.actions.push(Action::SaveAs(id));
            }
        }
        if take_key(&ctx, cmd, Key::S) {
            if let Some(id) = active {
                self.actions.push(Action::Save(id));
            }
        }
        if take_key(&ctx, Modifiers::ALT | Modifiers::SHIFT, Key::R) {
            if let Some(id) = active {
                self.actions.push(Action::Reveal(id));
            }
        }
        if take_key(&ctx, cmd, Key::O) {
            self.actions.push(Action::Open);
        }
        if take_key(&ctx, cmd, Key::P) || take_key(&ctx, cmd, Key::K) {
            self.ui.switcher = Some(Switcher {
                query: String::new(),
                selected: 0,
                focus: true,
            });
        }
        if take_key(&ctx, cmd, Key::F) {
            if let Some(id) = active {
                let sel = self.notes.get(&id).and_then(|n| {
                    let r = editor::get_cursor(&ctx, editor_id(id))?;
                    if r.is_empty() || r.end - r.start > 80 {
                        return None;
                    }
                    let b0 = n.doc.byte_index(r.start);
                    let b1 = n.doc.byte_index(r.end);
                    Some(n.doc.text[b0..b1].to_string())
                });
                let prev = self.ui.find.take().map(|f| f.query).unwrap_or_default();
                self.ui.find = Some(FindState {
                    note: id,
                    query: sel.unwrap_or(prev),
                    matches: vec![],
                    current: 0,
                    focus: true,
                    stale: true,
                    scroll: true,
                });
            }
        }
        if take_key(&ctx, Modifiers::COMMAND, Key::Comma) {
            self.ui.settings_open = true;
        }
        if take_key(&ctx, cmd_shift, Key::Tab) {
            self.cycle_tab(-1);
        }
        if take_key(&ctx, cmd, Key::Tab) {
            self.cycle_tab(1);
        }
        let mut zoom = 0.0;
        if take_key(&ctx, cmd, Key::Equals) || take_key(&ctx, cmd, Key::Plus) {
            zoom = 1.0;
        }
        if take_key(&ctx, cmd, Key::Minus) {
            zoom = -1.0;
        }
        let wheel = ctx.input(|i| i.zoom_delta());
        if wheel != 1.0 {
            zoom = if wheel > 1.0 { 1.0 } else { -1.0 };
        }
        if zoom != 0.0 {
            self.settings.font_size = (self.settings.font_size + zoom).clamp(10.0, 40.0);
            self.settings_dirty = true;
            self.mark_dirty();
            self.toast(format!("Tamanho do texto: {:.0}", self.settings.font_size));
        }
        if take_key(&ctx, Modifiers::NONE, Key::F11) {
            let fs = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fs));
        }
    }

    fn cycle_tab(&mut self, dir: i32) {
        let Some(path) = self.dock.focused_leaf() else {
            return;
        };
        let Ok(leaf) = self.dock.leaf(path) else {
            return;
        };
        let n = leaf.len();
        if n < 2 {
            return;
        }
        let cur = leaf.active.0 as i32;
        let next = (cur + dir).rem_euclid(n as i32) as usize;
        let id = leaf.tabs()[next];
        self.focus(id);
    }

    // ------------------------------------------------------- interface

    /// Botão "+" de cada painel: logo depois da última aba, com respiro e
    /// hover arredondado (o da biblioteca fica colado e tem hover quadrado).
    fn add_tab_buttons(&mut self, ui: &mut Ui, tab_rects: &HashMap<NoteId, Rect>, c: &Colors) {
        let mut buttons = Vec::new();
        for (path, leaf) in self.dock.iter_leaves() {
            let rects: Vec<Rect> = leaf.tabs().iter().filter_map(|t| tab_rects.get(t)).copied().collect();
            let Some(first) = rects.first() else { continue };
            let right = rects.iter().fold(first.right(), |m, r| m.max(r.right()));
            let x = (right + 6.0).min(leaf.rect().right() - 34.0);
            let rect = Rect::from_min_size(pos2(x, first.center().y - 13.0), vec2(26.0, 26.0));
            buttons.push((path, rect));
        }
        for (path, rect) in buttons {
            let resp = ui
                .interact(rect, Id::new(("dn-add-tab", format!("{path:?}"))), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("Nova nota (Ctrl+T)");
            let p = ui.painter();
            if resp.hovered() {
                p.rect_filled(rect, 7.0, theme::mix(c.panel, c.text, 0.09));
            }
            let col = if resp.hovered() { c.text } else { c.dim };
            p.text(rect.center(), Align2::CENTER_CENTER, icons::PLUS, icons::font(15.0), col);
            if resp.clicked() {
                self.actions.push(Action::NewIn(path));
            }
        }
    }

    /// "Nome da nota — Death Note" na barra de título, barra de tarefas e Alt+Tab.
    fn update_window_title(&mut self) {
        let title = match self.active.and_then(|id| self.notes.get(&id)) {
            Some(n) => format!("{} — {APP_TITLE}", short_title(&n.title)),
            None => APP_TITLE.to_string(),
        };
        if title != self.window_title {
            self.ctx
                .send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.window_title = title;
        }
    }

    fn update_titlebar(&mut self, frame: &eframe::Frame) {
        let c = self.colors();
        let key = (c.panel, c.dark);
        if self.titlebar != Some(key) {
            platform::style_titlebar(frame, c.panel, c.text, c.dark);
            self.titlebar = Some(key);
        }
        if self.applied_on_top != self.settings.always_on_top {
            self.applied_on_top = self.settings.always_on_top;
            self.ctx
                .send_viewport_cmd(egui::ViewportCommand::WindowLevel(if self.applied_on_top {
                    egui::WindowLevel::AlwaysOnTop
                } else {
                    egui::WindowLevel::Normal
                }));
        }
    }

    fn dock_style(&self, c: &Colors) -> egui_dock::Style {
        let mut s = egui_dock::Style::from_egui(self.ctx.global_style().as_ref());
        s.dock_area_padding = None;
        s.main_surface_border_stroke = Stroke::NONE;
        s.main_surface_border_rounding = CornerRadius::ZERO;

        s.tab_bar.bg_fill = c.panel;
        s.tab_bar.height = 36.0;
        s.tab_bar.hline_color = c.panel;
        s.tab_bar.corner_radius = CornerRadius::ZERO;
        s.tab_bar.fill_tab_bar = false;
        // Sem barra de rolagem horizontal por cima do editor quando há muitas abas.
        s.tab_bar.show_scroll_bar_on_overflow = false;
        s.tab_bar.inner_margin = Margin {
            left: 8,
            right: 8,
            top: 4,
            bottom: 0,
        };

        let top = CornerRadius {
            nw: TAB_RADIUS as u8,
            ne: TAB_RADIUS as u8,
            sw: 0,
            se: 0,
        };
        let inactive_leaf_active = theme::mix(c.bg, c.panel, 0.45);
        s.tab.active.bg_fill = inactive_leaf_active;
        s.tab.active.text_color = c.text;
        s.tab.active.outline_color = Color32::TRANSPARENT;
        s.tab.active.corner_radius = top;
        s.tab.focused.bg_fill = c.bg;
        s.tab.focused.text_color = c.text;
        s.tab.focused.outline_color = Color32::TRANSPARENT;
        s.tab.focused.corner_radius = top;
        s.tab.inactive.bg_fill = c.panel;
        s.tab.inactive.text_color = c.dim;
        s.tab.inactive.outline_color = Color32::TRANSPARENT;
        s.tab.inactive.corner_radius = top;
        s.tab.hovered.bg_fill = theme::mix(c.panel, c.text, 0.06);
        s.tab.hovered.text_color = c.text;
        s.tab.hovered.outline_color = Color32::TRANSPARENT;
        s.tab.hovered.corner_radius = top;
        s.tab.active_with_kb_focus = s.tab.active.clone();
        s.tab.inactive_with_kb_focus = s.tab.inactive.clone();
        s.tab.focused_with_kb_focus = s.tab.focused.clone();
        s.tab.hline_below_active_tab_name = false;
        s.tab.minimum_width = Some(110.0);
        s.tab.spacing = 2.0;
        s.tab.tab_body.bg_fill = c.bg;
        s.tab.tab_body.stroke = Stroke::NONE;
        s.tab.tab_body.corner_radius = CornerRadius::ZERO;
        s.tab.tab_body.inner_margin = Margin::ZERO;

        s.separator.width = 1.0;
        // Área de pegar o divisor mais larga que o traço, e painel com largura mínima.
        s.separator.extra_interact_width = 6.0;
        s.separator.extra = 175.0;
        s.separator.color_idle = theme::mix(c.border, c.text, 0.08);
        s.separator.color_hovered = c.accent;
        s.separator.color_dragged = c.accent;

        // O "×" é desenhado em `on_tab_button` (ícone de verdade, hover arredondado);
        // o da biblioteca fica invisível, mas a área de clique dele continua valendo.
        s.buttons.close_tab_color = Color32::TRANSPARENT;
        s.buttons.close_tab_active_color = Color32::TRANSPARENT;
        s.buttons.close_tab_bg_fill = Color32::TRANSPARENT;
        s.buttons.add_tab_color = c.dim;
        s.buttons.add_tab_active_color = c.text;
        s.buttons.add_tab_bg_fill = theme::mix(c.panel, c.text, 0.08);
        s.buttons.add_tab_border_color = Color32::TRANSPARENT;
        s.buttons.add_tab_align = egui_dock::TabAddAlign::Left;

        // Arrastar uma aba para a metade de um painel já divide a tela
        // (sem precisar mirar nos ícones centrais).
        s.overlay.overlay_type = egui_dock::OverlayType::HighlightedAreas;
        s.overlay.selection_color = c.accent.gamma_multiply(0.18);
        s.overlay.selection_stroke_width = 2.0;
        s.overlay.feel.center_drop_coverage = 0.3;
        s
    }

    fn status_bar(&mut self, ui: &mut Ui, c: &Colors) {
        let active = self.active;
        // Em janelas estreitas, some primeiro o que é menos útil.
        let width = ui.available_width();
        let show_words = width >= 480.0;
        let show_pos = width >= 640.0;
        let show_chars = width >= 760.0;
        let show_file = width >= 880.0;
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 16.0;
            let small = |t: String| RichText::new(t).size(12.0).color(c.dim);
            if let Some(n) = active.and_then(|id| self.notes.get(&id)) {
                let cur = editor::get_cursor(&self.ctx, editor_id(n.id));
                if show_words {
                    ui.label(small(plural(n.words, "palavra", "palavras")));
                }
                if show_chars {
                    ui.label(small(plural(n.doc.len_chars(), "caractere", "caracteres")));
                }
                if let (Some(r), true) = (cur, show_pos) {
                    if r.is_empty() {
                        let (ln, col) = line_col(&n.doc.text, r.start);
                        ui.label(small(format!("Lin {ln}, Col {col}")));
                    } else {
                        ui.label(small(format!(
                            "{} selecionado{}",
                            plural(r.end - r.start, "caractere", "caracteres"),
                            if r.end - r.start == 1 { "" } else { "s" }
                        )));
                    }
                }
                // Estilo pendente (Ctrl+B sem seleção): mostra o que o próximo texto terá.
                if let Some((_, st)) = n.pending {
                    let mut glyphs = Vec::new();
                    for (flag, g) in [
                        (BOLD, icons::BOLD),
                        (ITALIC, icons::ITALIC),
                        (UNDERLINE, icons::UNDERLINE),
                        (STRIKE, icons::STRIKE),
                        (CODE, icons::CODE),
                    ] {
                        if st.has(flag) {
                            glyphs.push(g);
                        }
                    }
                    if !glyphs.is_empty() {
                        ui.label(
                            RichText::new(format!("Próximo texto: {}", glyphs.join(" ")))
                                .size(12.0)
                                .color(c.accent_text()),
                        )
                        .on_hover_text("O que você digitar agora sai com essa formatação");
                    }
                }
                if let (Some(f), true) = (&n.file, show_file) {
                    let dot = if n.file_dirty { " •" } else { "" };
                    ui.label(small(format!("{}  {}{dot}", icons::FILE_TEXT, file_name(f))))
                        .on_hover_text(f.display().to_string());
                }
            }

            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let menu = icon_button(ui, icons::LIST, "Menu", c);
                self.main_menu(&menu, c);
                if icon_button(ui, icons::GEAR, "Configurações (Ctrl+,)", c).clicked() {
                    self.ui.settings_open = true;
                }
                let pin_tip = if self.settings.always_on_top {
                    "Janela fixada por cima das outras (clique para soltar)"
                } else {
                    "Manter a janela por cima das outras"
                };
                let pin = icon_toggle(ui, icons::PUSH_PIN, self.settings.always_on_top, pin_tip, c);
                if pin.clicked() {
                    self.settings.always_on_top = !self.settings.always_on_top;
                    self.settings_dirty = true;
                    self.mark_dirty();
                }
                let (icon, tip) = match self.settings.theme {
                    ThemeMode::Dark => (icons::MOON, "Tema: Escuro (clique para Claro)"),
                    ThemeMode::Light => (icons::SUN, "Tema: Claro (clique para Sistema)"),
                    ThemeMode::System => (icons::CIRCLE_HALF, "Tema: Sistema (clique para Escuro)"),
                };
                if icon_button(ui, icon, tip, c).clicked() {
                    self.settings.theme = match self.settings.theme {
                        ThemeMode::Dark => ThemeMode::Light,
                        ThemeMode::Light => ThemeMode::System,
                        ThemeMode::System => ThemeMode::Dark,
                    };
                    theme::apply(&self.ctx, &self.settings);
                    self.settings_dirty = true;
                    self.mark_dirty();
                }
                ui.add_space(12.0);
                if let Some(label) = self.update_pill_label() {
                    let pill = egui::Button::new(
                        RichText::new(label).size(12.0).color(c.on_accent()),
                    )
                    .fill(c.accent)
                    .min_size(vec2(0.0, 22.0))
                    .corner_radius(CornerRadius::same(11));
                    if ui.add(pill).on_hover_text("Ver o que há de novo").clicked() {
                        self.update_open = true;
                    }
                    ui.add_space(10.0);
                }
                if let Some(e) = &self.save_error {
                    ui.label(
                        RichText::new(format!("{}  Não foi possível salvar", icons::WARNING))
                            .size(12.0)
                            .color(c.danger()),
                    )
                    .on_hover_text(format!(
                        "{e}\nSuas notas continuam abertas; o Death Note tenta de novo a cada alteração."
                    ));
                } else if self.dirty_since.is_some() {
                    ui.label(small("Salvando…".into()));
                } else if let Some(t) = self.last_saved {
                    ui.label(small(format!("{}  Salvo às {}", icons::CHECK, t.format("%H:%M"))))
                        .on_hover_text("Tudo é salvo automaticamente enquanto você escreve");
                } else {
                    ui.label(small(format!("{}  Tudo salvo", icons::CHECK)))
                        .on_hover_text("Tudo é salvo automaticamente enquanto você escreve");
                }
            });
        });
    }

    fn main_menu(&mut self, button: &egui::Response, c: &Colors) {
        let active = self.active;
        let split = self.dock.iter_leaves().count() > 1;
        egui::Popup::menu(button)
            .align(egui::RectAlign::TOP_END)
            .show(|ui| {
                ui.set_min_width(250.0);
                if menu_item(ui, c, icons::FILE_PLUS, "Nova nota", "Ctrl+T", true) {
                    self.actions.push(Action::New);
                }
                if menu_item(ui, c, icons::FOLDER_OPEN, "Abrir arquivo…", "Ctrl+O", true) {
                    self.actions.push(Action::Open);
                }
                let has = active.is_some();
                if menu_item(ui, c, icons::FLOPPY, "Salvar", "Ctrl+S", has) {
                    self.actions.push(Action::Save(active.unwrap()));
                }
                if menu_item(ui, c, icons::FLOPPY, "Salvar como…", "Ctrl+Shift+S", has) {
                    self.actions.push(Action::SaveAs(active.unwrap()));
                }
                if menu_item(ui, c, icons::PENCIL, "Renomear nota", "Clique duplo na aba", has) {
                    self.actions.push(Action::Rename(active.unwrap()));
                }
                let has_file = active
                    .and_then(|id| self.notes.get(&id))
                    .is_some_and(|n| n.file.is_some());
                if menu_item(ui, c, icons::REVEAL, "Mostrar no Explorer", "Alt+Shift+R", has_file) {
                    self.actions.push(Action::Reveal(active.unwrap()));
                }
                ui.separator();
                if menu_item(ui, c, icons::SEARCH, "Busca rápida", "Ctrl+P", true) {
                    self.ui.switcher = Some(Switcher { query: String::new(), selected: 0, focus: true });
                }
                if menu_item(ui, c, icons::SPLIT_H, "Dividir à direita", "", has) {
                    self.actions.push(Action::Split(active.unwrap(), Split::Right));
                }
                if menu_item(ui, c, icons::SPLIT_V, "Dividir abaixo", "", has) {
                    self.actions.push(Action::Split(active.unwrap(), Split::Below));
                }
                if menu_item(ui, c, icons::MERGE, "Juntar painéis", "", split) {
                    self.actions.push(Action::Unsplit);
                }
                ui.separator();
                let closed_label = (
                    RichText::new(icons::HISTORY).font(icons::font(15.0)).color(c.dim),
                    RichText::new("Fechadas recentemente"),
                );
                ui.menu_button(closed_label, |ui| {
                    ui.set_min_width(240.0);
                    if self.closed.is_empty() {
                        ui.label(RichText::new("Nenhuma nota fechada").color(c.dim));
                    }
                    for (i, d) in self.closed.iter().take(20).enumerate() {
                        let t = if d.title.is_empty() { "Nota" } else { d.title.as_str() };
                        let key = if i == 0 { "Ctrl+Shift+T" } else { "" };
                        if ui
                            .add(
                                egui::Button::new(short_title(t))
                                    .shortcut_text(RichText::new(key).color(c.dim)),
                            )
                            .clicked()
                        {
                            self.actions.push(Action::Reopen(i));
                        }
                    }
                });
                if menu_item(ui, c, icons::ARROW_CIRCLE_UP, "Verificar atualizações", "", true) {
                    updater::check(&self.update, true);
                    self.update_open = true;
                }
                if menu_item(ui, c, icons::GEAR, "Configurações", "Ctrl+,", true) {
                    self.ui.settings_open = true;
                }
                if menu_item(ui, c, icons::FOLDER, "Abrir pasta de dados", "", true) {
                    platform::open_folder(&storage::data_dir());
                }
            });
    }

    /// Mensagens de outras instâncias e estado do atualizador.
    fn poll_background(&mut self, now: f64) {
        let mut focus = false;
        if let Some(rx) = &self.inbox {
            while let Ok(ipc::Message::Open(paths)) = rx.try_recv() {
                focus = true;
                for p in paths {
                    self.actions.push(Action::OpenPath(p));
                }
            }
        }
        if focus {
            self.ctx
                .send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            self.ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }

        // Nova verificação a cada 12 h se o app ficar aberto.
        if self.settings.check_updates {
            if self.last_update_check == 0.0 {
                self.last_update_check = now.max(0.001);
            } else if now - self.last_update_check > 12.0 * 3600.0 {
                self.last_update_check = now;
                updater::check(&self.update, false);
            }
        }

        let state = self.update.lock().unwrap().clone();
        match state {
            updater::State::UpToDate { manual: true } => {
                *self.update.lock().unwrap() = updater::State::Idle;
                if self.update_open {
                    self.update_open = false;
                    self.toast(format!(
                        "Você já tem a versão mais recente (v{})",
                        updater::current_version()
                    ));
                }
            }
            updater::State::Failed { manual: false, .. } => {
                // Falha silenciosa na verificação automática (ex.: sem internet).
                *self.update.lock().unwrap() = updater::State::Idle;
            }
            updater::State::Downloading { .. } => {
                self.ctx.request_repaint_after(Duration::from_millis(250));
            }
            _ => {}
        }
    }

    fn update_pill_label(&self) -> Option<String> {
        match &*self.update.lock().unwrap() {
            updater::State::Available(r) => Some(format!("{}  Atualização v{}", icons::ARROW_CIRCLE_UP, r.version)),
            updater::State::Downloading { .. } => Some(format!("{}  Baixando atualização…", icons::DOWNLOAD)),
            updater::State::Ready { .. } => Some(format!("{}  Pronto para atualizar", icons::ARROW_CIRCLE_UP)),
            _ => None,
        }
    }

    fn update_modal(&mut self, ctx: &egui::Context, c: &Colors) {
        let state = self.update.lock().unwrap().clone();
        let installed = updater::is_installed();
        let mut close = false;
        let mut retry = false;
        let mut start_download: Option<updater::Release> = None;
        let mut install: Option<PathBuf> = None;
        let resp = modal("dn-update", c, ctx).show(ctx, |ui| {
            ui.set_width(460.0);
            if modal_header(ui, "Atualizações", c) {
                close = true;
            }
            let current = updater::current_version();
            match &state {
                updater::State::Idle
                | updater::State::Checking { .. }
                | updater::State::UpToDate { .. } => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Procurando novas versões…");
                    });
                }
                updater::State::Failed { message, .. } => {
                    ui.label(
                        RichText::new(format!("Não foi possível atualizar: {message}"))
                            .color(c.danger()),
                    );
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Tentar de novo").clicked() {
                            retry = true;
                        }
                        if ui.button("Abrir página de downloads").clicked() {
                            platform::open_url(&format!(
                                "https://github.com/{}/releases/latest",
                                updater::REPO
                            ));
                        }
                    });
                }
                updater::State::Available(r)
                | updater::State::Downloading { release: r, .. }
                | updater::State::Ready { release: r, .. } => {
                    ui.label(
                        bold(format!("Nova versão disponível: v{}", r.version), 15.0)
                            .color(c.accent_text()),
                    );
                    ui.label(
                        RichText::new(format!("Você está usando a v{current}"))
                            .size(12.0)
                            .color(c.dim),
                    );
                    ui.add_space(8.0);
                    if !r.notes.trim().is_empty() {
                        Frame::new()
                            .fill(c.bg)
                            .corner_radius(CornerRadius::same(8))
                            .inner_margin(Margin::same(10))
                            .show(ui, |ui| {
                                egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    release_notes(ui, &r.notes, c);
                                });
                            });
                        ui.add_space(10.0);
                    }
                    match &state {
                        updater::State::Available(_) => {
                            ui.horizontal(|ui| {
                                let label = if installed {
                                    "Atualizar agora"
                                } else {
                                    "Baixar instalador"
                                };
                                let b = primary_button(label, c);
                                if ui.add(b).clicked() {
                                    if installed {
                                        start_download = Some(r.clone());
                                    } else {
                                        platform::open_url(&r.page);
                                    }
                                }
                                if ui.button("Depois").clicked() {
                                    close = true;
                                }
                                if ui.button("Ver no GitHub").clicked() {
                                    platform::open_url(&r.page);
                                }
                            });
                            if !installed {
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(
                                        "Esta cópia é portátil: use o instalador para \
                                         receber atualizações automáticas.",
                                    )
                                    .size(11.5)
                                    .color(c.dim),
                                );
                            }
                        }
                        updater::State::Downloading { path, .. } => {
                            let total = r.installer.as_ref().map(|a| a.size).unwrap_or(0);
                            let got = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                            let frac = if total > 0 {
                                (got as f32 / total as f32).min(1.0)
                            } else {
                                0.0
                            };
                            ui.add(egui::ProgressBar::new(frac).text(format!(
                                "Baixando… {:.1} de {:.1} MB",
                                got as f64 / 1_048_576.0,
                                total as f64 / 1_048_576.0
                            )));
                        }
                        updater::State::Ready { installer, .. } => {
                            ui.label("Download concluído e verificado.");
                            ui.add_space(6.0);
                            let b = primary_button("Instalar e reiniciar", c);
                            if ui.add(b).clicked() {
                                install = Some(installer.clone());
                            }
                            ui.label(
                                RichText::new(
                                    "Suas notas são salvas antes; o Death Note reabre sozinho.",
                                )
                                .size(11.5)
                                .color(c.dim),
                            );
                        }
                        _ => {}
                    }
                }
            }
        });
        if resp.should_close() {
            close = true;
        }
        if retry {
            *self.update.lock().unwrap() = updater::State::Idle;
            updater::check(&self.update, true);
        }
        if let Some(r) = start_download {
            updater::download(&self.update, r);
        }
        if let Some(path) = install {
            self.save_now();
            match updater::run_installer(&path) {
                Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                Err(e) => {
                    *self.update.lock().unwrap() = updater::State::Failed {
                        message: e,
                        manual: true,
                    }
                }
            }
        }
        if close {
            self.update_open = false;
            let mut s = self.update.lock().unwrap();
            if matches!(
                *s,
                updater::State::Failed { .. } | updater::State::UpToDate { .. }
            ) {
                *s = updater::State::Idle;
            }
        }
    }

    fn empty_state(&mut self, ui: &mut Ui, c: &Colors) {
        ui.vertical_centered(|ui| {
            ui.add_space((ui.available_height() * 0.26).max(24.0));
            ui.label(RichText::new(icons::NOTEBOOK).font(icons::font(48.0)).color(c.dim));
            ui.add_space(14.0);
            ui.label(bold("Nenhuma nota aberta", 19.0).color(c.text));
            ui.add_space(4.0);
            ui.label(RichText::new("Crie uma nota ou abra um arquivo .dnote, .md ou .txt.").color(c.dim));
            ui.add_space(18.0);
            ui.horizontal(|ui| {
                // Centraliza os dois botões
                let w = 300.0;
                ui.add_space(((ui.available_width() - w) / 2.0).max(0.0));
                let new = egui::Button::new((
                    RichText::new(icons::PLUS).font(icons::font(15.0)).color(c.on_accent()),
                    RichText::new("Nova nota").color(c.on_accent()),
                ))
                .fill(c.accent)
                .min_size(vec2(140.0, 32.0));
                if ui.add(new).on_hover_text("Ctrl+T").clicked() {
                    self.actions.push(Action::New);
                }
                let open = egui::Button::new((
                    RichText::new(icons::FOLDER_OPEN).font(icons::font(15.0)).color(c.dim),
                    RichText::new("Abrir arquivo"),
                ))
                .min_size(vec2(140.0, 32.0));
                if ui.add(open).on_hover_text("Ctrl+O").clicked() {
                    self.actions.push(Action::Open);
                }
            });
            if !self.closed.is_empty() {
                ui.add_space(30.0);
                ui.label(RichText::new("Fechadas recentemente").size(12.0).color(c.dim));
                ui.add_space(6.0);
                for (i, d) in self.closed.iter().take(6).enumerate() {
                    let title = if d.title.is_empty() { "Nota" } else { d.title.as_str() };
                    if ui
                        .link(RichText::new(short_title(title)).color(c.accent_text()))
                        .clicked()
                    {
                        self.actions.push(Action::Reopen(i));
                    }
                }
            }
        });
    }

    // ---------------------------------------------------------- overlays

    fn overlays(&mut self, c: &Colors) {
        let ctx = self.ctx.clone();
        self.ui.settings_fresh = self.ui.settings_open && !self.ui.settings_was_open;
        self.ui.settings_was_open = self.ui.settings_open;
        if self.ui.settings_fresh {
            self.ui.confirm_reset = false;
        }
        if self.ui.settings_open {
            self.settings_modal(&ctx, c);
        }
        if self.ui.switcher.is_some() {
            self.switcher_modal(&ctx, c);
        }
        if self.ui.rename.is_some() {
            self.rename_modal(&ctx, c);
        }
        if self.update_open {
            self.update_modal(&ctx, c);
        }
        if let Some((msg, until)) = self.ui.toast.clone() {
            let now = ctx.input(|i| i.time);
            if now > until {
                self.ui.toast = None;
            } else {
                ctx.request_repaint_after(Duration::from_secs_f64(until - now));
                let screen = ctx.content_rect();
                let y = screen.bottom() - if self.settings.show_status_bar { 44.0 } else { 16.0 };
                egui::Area::new(Id::new("dn-toast"))
                    .order(Order::Tooltip)
                    .pivot(Align2::CENTER_BOTTOM)
                    .fixed_pos(pos2(screen.center().x, y))
                    .interactable(false)
                    .show(&ctx, |ui| {
                        Frame::new()
                            .fill(c.surface)
                            .stroke(Stroke::new(1.0, c.border))
                            .corner_radius(CornerRadius::same(10))
                            .inner_margin(Margin::symmetric(14, 8))
                            .shadow(ctx.global_style().visuals.popup_shadow)
                            .show(ui, |ui| {
                                ui.add(egui::Label::new(RichText::new(msg).color(c.text)).extend());
                            });
                    });
            }
        }
    }

    fn rename_modal(&mut self, ctx: &egui::Context, c: &Colors) {
        let Some((id, mut text, focus)) = self.ui.rename.take() else {
            return;
        };
        let input_id = Id::new("dn-rename-input");
        let mut done = false;
        let mut cancel = false;
        let resp = modal("dn-rename", c, ctx).show(ctx, |ui| {
            ui.set_width(380.0);
            if modal_header(ui, "Renomear nota", c) {
                cancel = true;
            }
            let r = ui.add(
                egui::TextEdit::singleline(&mut text)
                    .id(input_id)
                    .desired_width(f32::INFINITY)
                    .margin(Margin::symmetric(10, 7))
                    .hint_text("Deixe vazio para usar a primeira linha"),
            );
            if focus {
                r.request_focus();
            }
            if r.gained_focus() {
                // Seleciona o nome todo ao abrir: digitar substitui. (Só funciona
                // depois que o campo tem foco; sem foco o egui desfaz a seleção.)
                editor::set_cursor(ui.ctx(), input_id, 0..text.chars().count());
                ui.ctx().request_repaint();
            }
            if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                done = true;
            }
            ui.add_space(16.0);
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if ui.add(primary_button("Renomear", c)).clicked() {
                    done = true;
                }
                if ui.add(egui::Button::new("Cancelar").min_size(vec2(0.0, 30.0))).clicked() {
                    cancel = true;
                }
            });
        });
        if resp.should_close() {
            cancel = true;
        }
        if done {
            if let Some(n) = self.notes.get_mut(&id) {
                let t = text.trim();
                n.custom_title = !t.is_empty();
                n.title = t.to_string();
                n.refresh();
            }
            self.mark_dirty();
            self.ui.focus_editor = Some(id);
        } else if !cancel {
            self.ui.rename = Some((id, text, false));
        } else {
            self.ui.focus_editor = Some(id);
        }
    }

    fn switcher_modal(&mut self, ctx: &egui::Context, c: &Colors) {
        let Some(mut sw) = self.ui.switcher.take() else {
            return;
        };
        // Itens: notas abertas (ordem das abas) + fechadas.
        let q = sw.query.to_lowercase();
        let mut items: Vec<SwitchItem> = Vec::new();
        let open: Vec<NoteId> = self.dock.iter_all_tabs().map(|(_, id)| *id).collect();
        for id in open {
            if let Some(n) = self.notes.get(&id) {
                if let Some(snip) = matches_query(&n.title, &n.doc.text, &q) {
                    let badge = (Some(id) == self.active).then_some("atual");
                    items.push(SwitchItem { title: n.title.clone(), snip, badge, target: Ok(id) });
                }
            }
        }
        for (i, d) in self.closed.iter().enumerate() {
            let title = if d.title.is_empty() { "Nota" } else { &d.title };
            if let Some(snip) = matches_query(title, &d.doc.text, &q) {
                items.push(SwitchItem { title: title.to_string(), snip, badge: Some("fechada"), target: Err(i) });
            }
        }
        if sw.selected >= items.len() {
            sw.selected = items.len().saturating_sub(1);
        }
        let (up, down, enter, esc) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::Enter),
                i.consume_key(Modifiers::NONE, Key::Escape),
            )
        });
        if up {
            sw.selected = sw.selected.saturating_sub(1);
        }
        if down && sw.selected + 1 < items.len() {
            sw.selected += 1;
        }
        let mut chosen = if enter { items.get(sw.selected).map(|x| x.target) } else { None };
        let mut close = esc;

        // Ancorado perto do topo: não "pula" conforme o número de resultados muda.
        let top = ctx.content_rect().height() * 0.14;
        let area = egui::Modal::default_area(Id::new("dn-switcher"))
            .anchor(Align2::CENTER_TOP, vec2(0.0, top));
        let resp = modal("dn-switcher", c, ctx).area(area).show(ctx, |ui| {
            ui.set_width(540.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                ui.label(RichText::new(icons::SEARCH).font(icons::font(18.0)).color(c.dim));
                let r = ui.add(
                    egui::TextEdit::singleline(&mut sw.query)
                        .desired_width(f32::INFINITY)
                        .font(FontId::proportional(16.0))
                        .frame(Frame::NONE)
                        .min_size(vec2(0.0, 28.0))
                        .vertical_align(Align::Center)
                        .hint_text("Buscar em todas as notas…"),
                );
                if sw.focus {
                    r.request_focus();
                    sw.focus = false;
                }
                if r.changed() {
                    sw.selected = 0;
                }
            });
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(6.0);
            egui::ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
                if items.is_empty() {
                    ui.add_space(10.0);
                    ui.label(RichText::new("Nenhuma nota com esse texto").color(c.dim));
                    ui.add_space(10.0);
                }
                for (i, item) in items.iter().enumerate() {
                    let selected = i == sw.selected;
                    let resp = list_item(ui, item, &q, selected, c);
                    if selected && (up || down) {
                        resp.scroll_to_me(None);
                    }
                    if resp.clicked() {
                        chosen = Some(item.target);
                    }
                    if resp.hovered() && ui.input(|i| i.pointer.delta() != Vec2::ZERO) {
                        sw.selected = i;
                    }
                }
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                key_chip(ui, "↑ / ↓", c);
                ui.label(RichText::new("navegar").size(12.0).color(c.dim));
                ui.add_space(8.0);
                key_chip(ui, "Enter", c);
                ui.label(RichText::new("abrir").size(12.0).color(c.dim));
                ui.add_space(8.0);
                key_chip(ui, "Esc", c);
                ui.label(RichText::new("fechar").size(12.0).color(c.dim));
            });
        });
        if resp.should_close() {
            close = true;
        }
        match chosen {
            Some(Ok(id)) => self.actions.push(Action::Focus(id)),
            Some(Err(i)) => self.actions.push(Action::Reopen(i)),
            None if !close => self.ui.switcher = Some(sw),
            None => {}
        }
    }

    fn settings_modal(&mut self, ctx: &egui::Context, c: &Colors) {
        let before = serde_json::to_string(&self.settings).unwrap_or_default();
        let mut open = true;
        let fresh = self.ui.settings_fresh;
        let resp = modal("dn-settings", c, ctx).show(ctx, |ui| {
            ui.set_width(500.0);
            if modal_header(ui, "Configurações", c) {
                open = false;
            }
            let mut scroll = egui::ScrollArea::vertical()
                .max_height((ctx.content_rect().height() - 190.0).clamp(220.0, 620.0))
                .auto_shrink([false, true]);
            if fresh {
                scroll = scroll.vertical_scroll_offset(0.0);
            }
            scroll.show(ui, |ui| {
                // Margem à direita para a barra de rolagem não cobrir o texto.
                Frame::new().inner_margin(Margin { left: 0, right: 14, top: 0, bottom: 0 }).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    section(ui, "Aparência", c);
                    egui::Grid::new("dn-look-grid")
                        .num_columns(2)
                        .spacing([18.0, 14.0])
                        .min_col_width(150.0)
                        .show(ui, |ui| {
                            ui.label("Tema");
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                for (mode, glyph, label) in [
                                    (ThemeMode::Dark, icons::MOON, "Escuro"),
                                    (ThemeMode::Light, icons::SUN, "Claro"),
                                    (ThemeMode::System, icons::CIRCLE_HALF, "Sistema"),
                                ] {
                                    let on = self.settings.theme == mode;
                                    let b = egui::Button::selectable(
                                        on,
                                        (
                                            RichText::new(glyph).font(icons::font(15.0)),
                                            RichText::new(label),
                                        ),
                                    )
                                    .min_size(vec2(0.0, 30.0));
                                    if ui.add(b).clicked() {
                                        self.settings.theme = mode;
                                    }
                                }
                            });
                            ui.end_row();

                            ui.label("Cor de destaque");
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;
                                for (name, rgb) in ACCENT_PRESETS {
                                    let col = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                                    if swatch(ui, col, self.settings.accent == *rgb, name, 22.0).clicked() {
                                        self.settings.accent = *rgb;
                                    }
                                }
                            });
                            ui.end_row();

                            ui.label("");
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Outra cor").color(c.dim));
                                ui.color_edit_button_srgb(&mut self.settings.accent)
                                    .on_hover_text("Escolher qualquer cor de destaque");
                            });
                            ui.end_row();

                            ui.label("Fundo do editor");
                            ui.horizontal(|ui| {
                                let mut custom = self.settings.custom_bg.is_some();
                                if ui.checkbox(&mut custom, "Personalizado").changed() {
                                    self.settings.custom_bg = if custom {
                                        let b = c.bg;
                                        Some([b.r(), b.g(), b.b()])
                                    } else {
                                        None
                                    };
                                }
                                if let Some(bg) = &mut self.settings.custom_bg {
                                    ui.color_edit_button_srgb(bg);
                                }
                            });
                            ui.end_row();
                        });

                    section(ui, "Editor", c);
                    egui::Grid::new("dn-editor-grid")
                        .num_columns(2)
                        .spacing([18.0, 14.0])
                        .min_col_width(150.0)
                        .show(ui, |ui| {
                            ui.label("Tamanho do texto");
                            ui.add(
                                egui::Slider::new(&mut self.settings.font_size, 10.0..=40.0)
                                    .step_by(1.0)
                                    .fixed_decimals(0)
                                    .suffix(" pt"),
                            );
                            ui.end_row();
                            ui.label("Espaço entre linhas");
                            ui.add(
                                egui::Slider::new(&mut self.settings.line_spacing, 1.0..=2.2)
                                    .step_by(0.05)
                                    .fixed_decimals(2),
                            );
                            ui.end_row();
                            ui.label("Largura do texto");
                            ui.add(
                                egui::Slider::new(&mut self.settings.max_width, 0.0..=1600.0)
                                    .step_by(20.0)
                                    .custom_formatter(|v, _| {
                                        if v <= 0.0 { "sem limite".into() } else { format!("{v:.0} px") }
                                    }),
                            )
                            .on_hover_text("Limita a largura da coluna de texto para facilitar a leitura");
                            ui.end_row();
                        });
                    ui.add_space(10.0);
                    ui.checkbox(&mut self.settings.auto_lists, "Continuar listas e checklists ao pressionar Enter");
                    ui.checkbox(&mut self.settings.cursor_blink, "Cursor piscante");
                    ui.checkbox(&mut self.settings.show_status_bar, "Mostrar barra de status");
                    ui.checkbox(&mut self.settings.always_on_top, "Manter a janela por cima das outras");

                    section(ui, "Arquivos", c);
                    ui.checkbox(
                        &mut self.settings.autosave_files,
                        "Salvar automaticamente as notas vinculadas a arquivos",
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(
                            "Todas as notas são gravadas continuamente na pasta de dados. \
                             Se o PC desligar de repente, tudo estará lá ao reabrir.",
                        )
                        .size(12.5)
                        .color(c.dim),
                    );
                    ui.add_space(8.0);
                    let b = egui::Button::new((
                        RichText::new(icons::FOLDER).font(icons::font(15.0)).color(c.dim),
                        RichText::new("Abrir pasta de dados"),
                    ));
                    if ui.add(b).clicked() {
                        platform::open_folder(&storage::data_dir());
                    }

                    section(ui, "Atualizações", c);
                    ui.checkbox(
                        &mut self.settings.check_updates,
                        "Procurar novas versões ao abrir o app",
                    );
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("Versão instalada: v{}", updater::current_version()))
                                .color(c.dim),
                        );
                        ui.add_space(8.0);
                        if ui.button("Verificar agora").clicked() {
                            updater::check(&self.update, true);
                            self.update_open = true;
                        }
                    });

                    section(ui, "Atalhos de teclado", c);
                    let shortcuts = [
                        ("Ctrl+T / Ctrl+N", "Nova nota"),
                        ("Ctrl+W", "Fechar nota"),
                        ("Ctrl+Shift+T", "Reabrir nota fechada"),
                        ("Ctrl+Tab / Ctrl+Shift+Tab", "Próxima / anterior aba"),
                        ("Ctrl+O", "Abrir arquivo"),
                        ("Alt+Shift+R", "Mostrar o arquivo da nota no Explorer"),
                        ("Ctrl+S / Ctrl+Shift+S", "Salvar / Salvar como"),
                        ("Ctrl+F", "Buscar na nota"),
                        ("Ctrl+P / Ctrl+K", "Busca rápida entre notas"),
                        ("Ctrl+B / Ctrl+I / Ctrl+U", "Negrito / Itálico / Sublinhado"),
                        ("Ctrl+Shift+X", "Tachado"),
                        ("Ctrl+E", "Código"),
                        ("Ctrl+1 / Ctrl+2 / Ctrl+3", "Títulos"),
                        ("Ctrl+0", "Texto normal"),
                        ("Ctrl+\\", "Limpar formatação"),
                        ("Ctrl+L", "Checklist"),
                        ("Ctrl+Shift+L", "Lista com marcadores"),
                        ("F5", "Inserir data e hora"),
                        ("Ctrl+Z / Ctrl+Y", "Desfazer / Refazer"),
                        ("Ctrl+= / Ctrl+-", "Aumentar / diminuir o texto (ou Ctrl + roda)"),
                        ("Ctrl+,", "Configurações"),
                        ("F11", "Tela cheia"),
                    ];
                    egui::Grid::new("dn-shortcuts")
                        .num_columns(2)
                        .spacing([18.0, 8.0])
                        .min_col_width(150.0)
                        .show(ui, |ui| {
                            for (k, d) in shortcuts {
                                key_chip(ui, k, c);
                                ui.label(d);
                                ui.end_row();
                            }
                        });
                    ui.add_space(6.0);
                });
            });

            // Rodapé fixo
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("Death Note v{}", updater::current_version()))
                        .size(12.0)
                        .color(c.dim),
                );
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if self.ui.confirm_reset {
                        let b = egui::Button::new(
                            RichText::new("Confirmar: restaurar tudo").color(Color32::WHITE),
                        )
                        .fill(c.danger());
                        if ui.add(b).clicked() {
                            self.settings = Settings::default();
                            self.ui.confirm_reset = false;
                        }
                        if ui.button("Cancelar").clicked() {
                            self.ui.confirm_reset = false;
                        }
                    } else if ui
                        .button("Restaurar padrões…")
                        .on_hover_text("Volta tema, cores e opções do editor ao original")
                        .clicked()
                    {
                        self.ui.confirm_reset = true;
                    }
                });
            });
        });
        if resp.should_close() {
            open = false;
        }
        self.ui.settings_open = open;
        let after = serde_json::to_string(&self.settings).unwrap_or_default();
        if before != after {
            theme::apply(ctx, &self.settings);
            self.settings_dirty = true;
            self.mark_dirty();
        }
    }
}

// ================================================================ TabViewer

struct Viewer<'a> {
    notes: &'a mut HashMap<NoteId, Note>,
    settings: &'a Settings,
    colors: Colors,
    caps: FontCaps,
    ui: &'a mut UiState,
    actions: &'a mut Vec<Action>,
    changed: &'a mut bool,
    active: &'a mut Option<NoteId>,
    split: bool,
    now: f64,
    /// Abas ativas (uma por painel) e a cor de fundo de cada uma.
    tab_bg: &'a HashMap<NoteId, Color32>,
    /// Retângulo de cada aba neste frame (para posicionar o "+").
    tab_rects: &'a mut HashMap<NoteId, Rect>,
}

impl TabViewer for Viewer<'_> {
    type Tab = NoteId;

    fn id(&mut self, tab: &mut NoteId) -> Id {
        Id::new(("dn-tab", *tab))
    }

    fn title(&mut self, tab: &mut NoteId) -> WidgetText {
        match self.notes.get(tab) {
            Some(n) => {
                let dot = if n.file.is_some() && n.file_dirty { "  •" } else { "" };
                format!("{}{dot}", short_title(&n.title)).into()
            }
            None => "?".into(),
        }
    }

    fn on_tab_button(&mut self, tab: &mut NoteId, response: &egui::Response) {
        let r = response.rect;
        self.tab_rects.insert(*tab, r);
        let painter = response.ctx.layer_painter(response.layer_id);
        let c = self.colors;
        let pointer = response.ctx.pointer_hover_pos();
        let active_bg = self.tab_bg.get(tab).copied();

        // Curvas na base da aba ativa: ela "escorre" para dentro do editor,
        // como as abas do navegador.
        if let Some(bg) = active_bg {
            let clip = r.expand2(vec2(TAB_RADIUS + 1.0, 0.0));
            let fp = painter.with_clip_rect(clip);
            tab_flare(&fp, pos2(r.left(), r.bottom()), -1.0, bg);
            tab_flare(&fp, pos2(r.right(), r.bottom()), 1.0, bg);
        }

        // "×": visível na aba ativa e ao passar o mouse sobre a aba.
        let close = Rect::from_center_size(pos2(r.right() - 12.0, r.center().y), vec2(20.0, 20.0));
        let over_tab = pointer.is_some_and(|p| r.contains(p));
        let over_close = pointer.is_some_and(|p| close.contains(p));
        if active_bg.is_some() || over_tab {
            let cp = painter.with_clip_rect(r);
            let base = active_bg.unwrap_or_else(|| theme::mix(c.panel, c.text, 0.06));
            if over_close {
                cp.rect_filled(close, 6.0, theme::mix(base, c.text, 0.12));
            }
            let col = if over_close { c.text } else { c.dim };
            cp.text(close.center(), Align2::CENTER_CENTER, icons::X, icons::font(13.0), col);
        }

        // Com a tela dividida, uma linha na cor de destaque marca o painel em foco.
        if self.split && *self.active == Some(*tab) {
            let r = response.rect;
            response.ctx.layer_painter(response.layer_id).hline(
                r.x_range().shrink(8.0),
                r.top() + 1.0,
                Stroke::new(2.0, self.colors.accent),
            );
        }
        if let Some(n) = self.notes.get(tab) {
            let mut tip = n.title.clone();
            if let Some(f) = &n.file {
                tip = format!("{tip}\n{}", f.display());
            }
            if n.title.chars().count() > 28 || n.file.is_some() {
                response.clone().on_hover_text(tip);
            }
        }
        if response.double_clicked() {
            self.actions.push(Action::Rename(*tab));
        }
        if response.clicked() {
            *self.active = Some(*tab);
            self.ui.focus_editor = Some(*tab);
        }
    }

    fn context_menu(&mut self, ui: &mut Ui, tab: &mut NoteId, _path: NodePath) {
        let id = *tab;
        let c = self.colors;
        ui.set_min_width(240.0);
        if menu_item(ui, &c, icons::PENCIL, "Renomear", "Clique duplo", true) {
            self.actions.push(Action::Rename(id));
            ui.close();
        }
        if menu_item(ui, &c, icons::COPY, "Duplicar", "", true) {
            self.actions.push(Action::Duplicate(id));
            ui.close();
        }
        ui.separator();
        if menu_item(ui, &c, icons::SPLIT_H, "Dividir à direita", "", true) {
            self.actions.push(Action::Split(id, Split::Right));
            ui.close();
        }
        if menu_item(ui, &c, icons::SPLIT_V, "Dividir abaixo", "", true) {
            self.actions.push(Action::Split(id, Split::Below));
            ui.close();
        }
        if menu_item(ui, &c, icons::MERGE, "Juntar painéis", "", self.split) {
            self.actions.push(Action::Unsplit);
            ui.close();
        }
        ui.separator();
        if menu_item(ui, &c, icons::FLOPPY, "Salvar", "Ctrl+S", true) {
            self.actions.push(Action::Save(id));
            ui.close();
        }
        if menu_item(ui, &c, icons::FLOPPY, "Salvar como…", "Ctrl+Shift+S", true) {
            self.actions.push(Action::SaveAs(id));
            ui.close();
        }
        let has_file = self.notes.get(&id).is_some_and(|n| n.file.is_some());
        if menu_item(ui, &c, icons::REVEAL, "Mostrar no Explorer", "Alt+Shift+R", has_file) {
            self.actions.push(Action::Reveal(id));
            ui.close();
        }
        if menu_item(ui, &c, icons::MARKDOWN, "Copiar como Markdown", "", true) {
            if let Some(n) = self.notes.get(&id) {
                ui.ctx().copy_text(n.doc.to_markdown());
            }
            let ctx = ui.ctx().clone();
            self.ui.toast(&ctx, "Nota copiada como Markdown");
            ui.close();
        }
        ui.separator();
        if menu_item(ui, &c, icons::X, "Fechar", "Ctrl+W", true) {
            self.actions.push(Action::Close(id));
            ui.close();
        }
    }

    fn on_close(&mut self, tab: &mut NoteId) -> OnCloseResponse {
        self.actions.push(Action::Closed(*tab));
        OnCloseResponse::Close
    }

    fn on_add(&mut self, path: NodePath) {
        self.actions.push(Action::NewIn(path));
    }

    fn allowed_in_windows(&self, _tab: &mut NoteId) -> bool {
        false
    }

    fn scroll_bars(&self, _tab: &NoteId) -> [bool; 2] {
        [false, false]
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut NoteId) {
        let id = *tab;
        if !self.notes.contains_key(&id) {
            ui.label("Nota não encontrada");
            return;
        }
        let ctx = ui.ctx().clone();
        let edit_id = editor_id(id);

        // Busca: resultados calculados antes do editor; a barra é desenhada
        // depois, flutuando no canto superior direito do painel.
        let pane = ui.max_rect();
        let mut find_matches: Vec<Range<usize>> = Vec::new();
        let mut find_current = None;
        let mut scroll_to: Option<usize> = None;
        if let Some(f) = self.ui.find.as_mut().filter(|f| f.note == id) {
            let note = self.notes.get(&id).unwrap();
            if f.stale {
                f.matches = find_all(&note.doc.text, &f.query);
                if f.current >= f.matches.len() {
                    f.current = 0;
                }
                f.stale = false;
            }
            find_matches = f.matches.clone();
            if !find_matches.is_empty() {
                find_current = Some(f.current);
                if f.scroll {
                    scroll_to = Some(find_matches[f.current].start);
                    let m = find_matches[f.current].clone();
                    editor::set_cursor(&ctx, edit_id, m);
                }
            }
            f.scroll = false;
        }

        let note = self.notes.get_mut(&id).unwrap();

        if note.restore_cursor {
            note.restore_cursor = false;
            let c = note.cursor.min(note.doc.len_chars());
            editor::set_cursor(&ctx, edit_id, c..c);
        }
        if self.ui.focus_editor == Some(id) {
            self.ui.focus_editor = None;
            ctx.memory_mut(|m| m.request_focus(edit_id));
        }

        let focused = ctx.memory(|m| m.has_focus(edit_id));
        if focused {
            *self.active = Some(id);
            if editor_keys(&ctx, note, edit_id, self.settings, self.now) {
                *self.changed = true;
                if let Some(f) = &mut self.ui.find {
                    f.stale = true;
                }
            }
        }

        // ----------------------------------------------------- o editor
        let rc = RenderCtx {
            colors: self.colors,
            caps: self.caps,
            size: self.settings.font_size,
            line_spacing: self.settings.line_spacing,
            match_bg: theme::mix(self.colors.bg, Color32::from_rgb(255, 200, 0), 0.35),
            current_match_bg: theme::mix(self.colors.bg, Color32::from_rgb(255, 150, 0), 0.7),
        };
        let mut buf = RichBuffer::new(
            std::mem::take(&mut note.doc),
            std::mem::take(&mut note.history),
            note.pending.take(),
            self.now,
        );
        let matches_ref = &find_matches;
        let mut layouter = |ui: &Ui, b: &dyn TextBuffer, wrap: f32| {
            let job = match editor::downcast(b) {
                Some(rb) => editor::build_job(&rb.doc, &rc, wrap, matches_ref, find_current),
                None => editor::build_job(&Doc::from_plain(b.as_str()), &rc, wrap, &[], None),
            };
            ui.fonts_mut(|f| f.layout_job(job))
        };

        // Clicar na barra flutuante não deve tirar o foco do editor
        // (senão o egui desfaz a seleção). Mesmo truque do autocompletar do egui.
        let pointer_on_toolbar = matches!(self.ui.toolbar_rect, Some((tid, rect)) if tid == id
            && ctx.pointer_hover_pos().is_some_and(|p| rect.expand(6.0).contains(p)));
        let prev_surrender = ctx.options(|o| o.input_options.surrender_focus_on);
        if pointer_on_toolbar {
            ctx.options_mut(|o| o.input_options.surrender_focus_on = egui::SurrenderFocusOn::Never);
        }

        let max_width = self.settings.max_width;
        let avail_h = ui.available_height();
        let out = egui::ScrollArea::vertical()
            .id_salt(("dn-scroll", id))
            .auto_shrink(false)
            .show(ui, |ui| {
                let w = ui.available_width();
                let col = if max_width > 0.0 { w.min(max_width + 56.0) } else { w };
                let pad = ((w - col) / 2.0).max(0.0);
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    ui.add_space(pad);
                    let out = egui::TextEdit::multiline(&mut buf)
                        .id(edit_id)
                        .frame(Frame::new().inner_margin(Margin::symmetric(32, 22)))
                        .desired_width(col)
                        .min_size(vec2(col, avail_h))
                        .lock_focus(true)
                        .hint_text(RichText::new("Comece a escrever…").color(rc.colors.dim))
                        .layouter(&mut layouter)
                        .show(ui);
                    if let Some(ci) = scroll_to {
                        let r = out
                            .galley
                            .pos_from_cursor(egui::text::CCursor::new(ci))
                            .translate(out.galley_pos.to_vec2());
                        ui.scroll_to_rect(r.expand(60.0), Some(Align::Center));
                    }
                    out
                })
                .inner
            })
            .inner;

        ctx.options_mut(|o| o.input_options.surrender_focus_on = prev_surrender);

        if self.ui.find.as_ref().is_some_and(|f| f.note == id) {
            let mut f = self.ui.find.take().unwrap();
            let keep = egui::Area::new(Id::new(("dn-find-area", id)))
                .order(Order::Foreground)
                .pivot(Align2::RIGHT_TOP)
                .fixed_pos(pos2(pane.right() - 16.0, pane.top() + 10.0))
                .show(&ctx, |ui| find_bar(ui, &mut f, id, &self.colors))
                .inner;
            if f.stale || f.scroll {
                ctx.request_repaint();
            }
            if keep {
                self.ui.find = Some(f);
            } else {
                self.ui.focus_editor = Some(id);
            }
        }

        let note = self.notes.get_mut(&id).unwrap();
        note.doc = buf.doc;
        note.history = buf.history;
        note.pending = buf.pending;
        if buf.changed {
            note.touch();
            *self.changed = true;
            if let Some(f) = &mut self.ui.find {
                if f.note == id {
                    f.stale = true;
                }
            }
        }

        let sel = out.state.cursor.char_range().map(|r| {
            let [a, b] = r.sorted_cursors();
            a.index.0..b.index.0
        });
        if let Some(r) = &sel {
            if note.cursor != r.end {
                note.cursor = r.end;
            }
            // Estilo pendente só vale enquanto o cursor não se move.
            if let Some((p, _)) = note.pending {
                if !r.is_empty() || r.start != p {
                    note.pending = None;
                }
            }
        }

        // Cursor de mão sobre as caixinhas de checklist
        if out.response.hovered() {
            if let Some(p) = ctx.pointer_hover_pos() {
                if checkbox_at(note, &out, p).is_some() {
                    ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            }
        }

        // Clique em caixinha de checklist
        if out.response.clicked() {
            if let Some(p) = out.response.interact_pointer_pos() {
                if toggle_checkbox_at(note, &out, p) {
                    *self.changed = true;
                }
            }
        }

        // -------------------------------------------- barra flutuante
        let has_focus = out.response.has_focus();
        let pointer = ctx.input(|i| i.pointer.hover_pos());
        let dragging = ctx.input(|i| i.pointer.primary_down());
        let hot = matches!(self.ui.toolbar_rect, Some((tid, rect)) if tid == id
            && pointer.is_some_and(|p| rect.expand(6.0).contains(p)));
        let toolbar_open = self.ui.toolbar_menu != ToolbarMenu::None
            && matches!(self.ui.toolbar_rect, Some((tid, _)) if tid == id);
        if let Some(r) = sel.clone().filter(|r| !r.is_empty()) {
            if (has_focus && !dragging) || hot || (toolbar_open && has_focus) {
                let a = out
                    .galley
                    .pos_from_cursor(egui::text::CCursor::new(r.start))
                    .translate(out.galley_pos.to_vec2());
                let b = out
                    .galley
                    .pos_from_cursor(egui::text::CCursor::new(r.end))
                    .translate(out.galley_pos.to_vec2());
                let clip = out.text_clip_rect;
                if clip.intersects(Rect::from_two_pos(a.min, b.max)) {
                    let rect = show_toolbar(
                        &ctx, id, edit_id, note, r, a, b, clip, &self.colors, self.ui,
                    );
                    if let Some(changed) = rect.1 {
                        if changed {
                            *self.changed = true;
                            if let Some(f) = &mut self.ui.find {
                                f.stale = true;
                            }
                        }
                    }
                    self.ui.toolbar_rect = Some((id, rect.0));
                }
            } else if !hot && matches!(self.ui.toolbar_rect, Some((tid, _)) if tid == id) {
                self.ui.toolbar_rect = None;
                self.ui.toolbar_menu = ToolbarMenu::None;
            }
        } else if matches!(self.ui.toolbar_rect, Some((tid, _)) if tid == id) {
            self.ui.toolbar_rect = None;
            self.ui.toolbar_menu = ToolbarMenu::None;
        }
    }
}

// ============================================================ editor: teclas

fn take_key(ctx: &egui::Context, mods: Modifiers, key: Key) -> bool {
    ctx.input_mut(|i| {
        let pos = i.events.iter().position(|e| {
            matches!(e, Event::Key { key: k, pressed: true, modifiers: m, .. }
                if *k == key && m.command == mods.command && m.shift == mods.shift && m.alt == mods.alt)
        });
        if let Some(p) = pos {
            i.events.remove(p);
            true
        } else {
            false
        }
    })
}

/// Atalhos do editor (só quando ele tem foco). Retorna se o documento mudou.
fn editor_keys(
    ctx: &egui::Context,
    note: &mut Note,
    edit_id: Id,
    settings: &Settings,
    _now: f64,
) -> bool {
    let n = note.doc.len_chars();
    let cur = editor::get_cursor(ctx, edit_id).unwrap_or(n..n);
    let cmd = Modifiers::COMMAND;
    let cmd_shift = Modifiers::COMMAND | Modifiers::SHIFT;
    let mut changed = false;

    // Desfazer / refazer (substitui o desfazer do egui, que não conhece estilos)
    if take_key(ctx, cmd_shift, Key::Z) || take_key(ctx, cmd, Key::Y) {
        if let Some(c) = note.history.redo(&mut note.doc, cur.end) {
            editor::set_cursor(ctx, edit_id, c..c);
            note.touch();
            changed = true;
        }
    }
    if take_key(ctx, cmd, Key::Z) {
        if let Some(c) = note.history.undo(&mut note.doc, cur.end) {
            let c = c.min(note.doc.len_chars());
            editor::set_cursor(ctx, edit_id, c..c);
            note.touch();
            changed = true;
        }
    }

    let formats: [(Modifiers, Key, Format); 10] = [
        (cmd_shift, Key::X, Format::Flag(STRIKE)),
        (cmd, Key::B, Format::Flag(BOLD)),
        (cmd, Key::I, Format::Flag(ITALIC)),
        (cmd, Key::U, Format::Flag(UNDERLINE)),
        (cmd, Key::E, Format::Flag(CODE)),
        (cmd, Key::Num1, Format::Heading(1)),
        (cmd, Key::Num2, Format::Heading(2)),
        (cmd, Key::Num3, Format::Heading(3)),
        (cmd, Key::Num0, Format::Heading(0)),
        (cmd, Key::Backslash, Format::Clear),
    ];
    for (m, k, f) in formats {
        if take_key(ctx, m, k) {
            changed |= apply_format(note, cur.clone(), f);
        }
    }

    if take_key(ctx, cmd_shift, Key::L) {
        changed |= toggle_line_prefix(ctx, edit_id, note, cur.clone(), &format!("{BULLET} "));
    } else if take_key(ctx, cmd, Key::L) {
        changed |= toggle_line_prefix(ctx, edit_id, note, cur.clone(), &format!("{CHECK_OPEN} "));
    }

    if take_key(ctx, Modifiers::NONE, Key::F5) {
        let stamp = chrono::Local::now().format("%d/%m/%Y %H:%M").to_string();
        note.snapshot(cur.start);
        note.doc.delete(cur.clone());
        let st = note.doc.inherited_style(cur.start);
        let k = note.doc.insert(cur.start, &stamp, st);
        editor::set_cursor(ctx, edit_id, cur.start + k..cur.start + k);
        note.touch();
        changed = true;
    }

    // Enter em listas: continua o marcador (ou encerra a lista se o item estiver vazio).
    if settings.auto_lists && cur.is_empty() {
        let c = cur.start;
        let (ls, le) = note.doc.line_bounds(c, c);
        if let Some(prefix) = note.doc.list_prefix_at(ls) {
            let plen = prefix.chars().count();
            if c >= ls + plen && take_key(ctx, Modifiers::NONE, Key::Enter) {
                note.snapshot(c);
                if le == ls + plen && c == le {
                    note.doc.delete(ls..le);
                    editor::set_cursor(ctx, edit_id, ls..ls);
                } else {
                    let ins = format!("\n{}", doc::next_prefix(&prefix));
                    let k = note.doc.insert(c, &ins, CharStyle::default());
                    editor::set_cursor(ctx, edit_id, c + k..c + k);
                }
                note.touch();
                changed = true;
            }
        }
    }
    changed
}

/// Aplica formatação. Sem seleção, vira "estilo pendente" para a digitação.
fn apply_format(note: &mut Note, range: Range<usize>, fmt: Format) -> bool {
    if range.is_empty() {
        let c = range.start;
        if let Format::Heading(_) = fmt {
            let (s, e) = note.doc.line_bounds(c, c);
            if s < e {
                note.snapshot(c);
                note.doc.apply(s..e, fmt);
                note.touch();
                return true;
            }
        }
        let mut st = match note.pending {
            Some((p, s)) if p == c => s,
            _ => note.doc.inherited_style(c),
        };
        match fmt {
            Format::Flag(f) => st.flags ^= f,
            Format::Color(x) => st.color = x,
            Format::Highlight(x) => st.highlight = x,
            Format::Heading(h) => st.heading = if st.heading == h { 0 } else { h },
            Format::Clear => st = CharStyle::default(),
        }
        note.pending = Some((c, st));
        return false;
    }
    note.snapshot(range.start);
    note.doc.apply(range, fmt);
    note.touch();
    true
}

/// Liga/desliga um prefixo (checklist/marcador) nas linhas da seleção.
fn toggle_line_prefix(
    ctx: &egui::Context,
    edit_id: Id,
    note: &mut Note,
    range: Range<usize>,
    prefix: &str,
) -> bool {
    let starts = note.doc.line_starts(range.start, range.end);
    let all_have = starts.iter().all(|&s| {
        note.doc
            .list_prefix_at(s)
            .is_some_and(|p| p.trim_start() == prefix || (prefix.starts_with(CHECK_OPEN) && p.trim_start().starts_with(CHECK_DONE)))
    });
    note.snapshot(range.start);
    let plen = prefix.chars().count();
    let mut delta: isize = 0;
    for &s in starts.iter().rev() {
        let existing = note.doc.list_prefix_at(s);
        let indent = existing
            .as_ref()
            .map(|p| p.chars().take_while(|c| *c == ' ' || *c == '\t').count())
            .unwrap_or(0);
        if let Some(p) = &existing {
            let len = p.chars().count();
            note.doc.delete(s..s + len);
            delta -= len as isize;
        }
        if !all_have {
            let ins = format!("{}{prefix}", " ".repeat(indent));
            note.doc.insert(s, &ins, CharStyle::default());
            delta += (indent + plen) as isize;
        }
    }
    let end = (range.end as isize + delta).max(0) as usize;
    let end = end.min(note.doc.len_chars());
    if range.is_empty() {
        editor::set_cursor(ctx, edit_id, end..end);
    } else {
        let start = starts.first().copied().unwrap_or(0);
        editor::set_cursor(ctx, edit_id, start..end);
    }
    note.touch();
    true
}

/// Índice da caixinha de checklist (☐/☑) sob o ponteiro, se houver.
fn checkbox_at(note: &Note, out: &egui::text_edit::TextEditOutput, p: Pos2) -> Option<usize> {
    use egui::text::CCursor;
    let local = p - out.galley_pos;
    let cc = out.galley.cursor_from_pos(local);
    let i = cc.index.0;
    for idx in [i, i.saturating_sub(1)] {
        let Some(ch) = note.doc.char_at(idx) else { continue };
        if ch != CHECK_OPEN && ch != CHECK_DONE {
            continue;
        }
        let r0 = out.galley.pos_from_cursor(CCursor::new(idx));
        let r1 = out.galley.pos_from_cursor(CCursor::new(idx + 1));
        let rect = Rect::from_min_max(
            pos2(r0.left(), r0.top()),
            pos2(r1.left().max(r0.left() + 4.0), r0.bottom()),
        )
        .translate(out.galley_pos.to_vec2())
        .expand(3.0);
        if rect.contains(p) {
            return Some(idx);
        }
    }
    None
}

fn toggle_checkbox_at(note: &mut Note, out: &egui::text_edit::TextEditOutput, p: Pos2) -> bool {
    let Some(idx) = checkbox_at(note, out, p) else {
        return false;
    };
    let ch = note.doc.char_at(idx).unwrap_or(CHECK_OPEN);
    let st = note.doc.styles[idx];
    note.snapshot(idx);
    note.doc.delete(idx..idx + 1);
    let new = if ch == CHECK_OPEN { CHECK_DONE } else { CHECK_OPEN };
    note.doc.insert(idx, &new.to_string(), st);
    note.touch();
    true
}

// ======================================================= barra flutuante

#[allow(clippy::too_many_arguments)]
fn show_toolbar(
    ctx: &egui::Context,
    id: NoteId,
    edit_id: Id,
    note: &mut Note,
    range: Range<usize>,
    a: Rect,
    b: Rect,
    clip: Rect,
    c: &Colors,
    st: &mut UiState,
) -> (Rect, Option<bool>) {
    let top = a.top().min(b.top());
    let screen = ctx.content_rect();
    let above = top - 46.0 > clip.top().max(screen.top());
    let x = a.left().clamp(screen.left() + 8.0, (screen.right() - 470.0).max(screen.left() + 8.0));
    let (pivot, pos) = if above {
        (Align2::LEFT_BOTTOM, pos2(x - 6.0, top - 8.0))
    } else {
        (Align2::LEFT_TOP, pos2(x - 6.0, b.bottom() + 8.0))
    };

    let mut fmt: Option<Format> = None;
    let mut list: Option<String> = None;
    let mut copy_md = false;

    let resp = egui::Area::new(Id::new(("dn-toolbar", id)))
        .order(Order::Foreground)
        .pivot(pivot)
        .fixed_pos(pos)
        .constrain(true)
        .show(ctx, |ui| {
            Frame::new()
                .fill(c.surface)
                .stroke(Stroke::new(1.0, c.border))
                .corner_radius(CornerRadius::same(10))
                .inner_margin(Margin::same(4))
                .shadow(ctx.global_style().visuals.popup_shadow)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = vec2(2.0, 4.0);
                    let d = &note.doc;
                    let r = range.clone();
                    let cur_color = d.styles.get(r.start).map(|s| s.color).unwrap_or(0);
                    let cur_hl = d.styles.get(r.start).map(|s| s.highlight).unwrap_or(0);
                    ui.horizontal(|ui| match st.toolbar_menu {
                        ToolbarMenu::None => {
                            let flags: [(u8, &str, &str); 5] = [
                                (BOLD, icons::BOLD, "Negrito (Ctrl+B)"),
                                (ITALIC, icons::ITALIC, "Itálico (Ctrl+I)"),
                                (UNDERLINE, icons::UNDERLINE, "Sublinhado (Ctrl+U)"),
                                (STRIKE, icons::STRIKE, "Tachado (Ctrl+Shift+X)"),
                                (CODE, icons::CODE, "Código (Ctrl+E)"),
                            ];
                            for (flag, glyph, tip) in flags {
                                if tool_icon(ui, c, d.range_has(r.clone(), flag), glyph, tip).clicked() {
                                    fmt = Some(Format::Flag(flag));
                                }
                            }
                            divider(ui, c);
                            let h = d.range_heading(r.clone());
                            for (lvl, glyph) in [(1u8, icons::H1), (2, icons::H2), (3, icons::H3)] {
                                let tip = format!("Título {lvl} (Ctrl+{lvl})");
                                if tool_icon(ui, c, h == Some(lvl), glyph, &tip).clicked() {
                                    fmt = Some(Format::Heading(lvl));
                                }
                            }
                            divider(ui, c);
                            let color_now = c.text_color(cur_color);
                            if tool(ui, c, false, "Cor do texto", |p, rect, col| {
                                p.text(rect.center(), Align2::CENTER_CENTER, icons::TEXT_COLOR, icons::font(18.0), col);
                                swatch_dot(p, rect, color_now, c);
                            })
                            .clicked()
                            {
                                st.toolbar_menu = ToolbarMenu::Color;
                            }
                            let hl_col = if cur_hl > 0 {
                                c.highlight_color(cur_hl)
                            } else {
                                Colors::swatch_highlight(1)
                            };
                            if tool(ui, c, false, "Marca-texto", |p, rect, col| {
                                p.text(rect.center(), Align2::CENTER_CENTER, icons::HIGHLIGHTER, icons::font(18.0), col);
                                swatch_dot(p, rect, hl_col, c);
                            })
                            .clicked()
                            {
                                st.toolbar_menu = ToolbarMenu::Highlight;
                            }
                            divider(ui, c);
                            if tool_icon(ui, c, false, icons::CHECKLIST, "Checklist (Ctrl+L)").clicked() {
                                list = Some(format!("{CHECK_OPEN} "));
                            }
                            if tool_icon(ui, c, false, icons::BULLETS, "Lista com marcadores (Ctrl+Shift+L)").clicked() {
                                list = Some(format!("{BULLET} "));
                            }
                            divider(ui, c);
                            if tool_icon(ui, c, false, icons::MARKDOWN, "Copiar seleção como Markdown").clicked() {
                                copy_md = true;
                            }
                            if tool_icon(ui, c, false, icons::CLEAR_FORMAT, "Limpar formatação (Ctrl+\\)").clicked() {
                                fmt = Some(Format::Clear);
                            }
                        }
                        ToolbarMenu::Color => {
                            if tool_icon(ui, c, false, icons::CARET_LEFT, "Voltar").clicked() {
                                st.toolbar_menu = ToolbarMenu::None;
                            }
                            ui.spacing_mut().item_spacing.x = 8.0;
                            ui.label(RichText::new("Cor").size(12.5).color(c.dim));
                            ui.add_space(2.0);
                            if swatch(ui, c.text, cur_color == 0, "Padrão", 20.0).clicked() {
                                fmt = Some(Format::Color(0));
                            }
                            for (i, (name, _, _)) in TEXT_COLORS.iter().enumerate() {
                                let idx = i as u8 + 1;
                                if swatch(ui, c.text_color(idx), cur_color == idx, name, 20.0).clicked() {
                                    fmt = Some(Format::Color(idx));
                                }
                            }
                            ui.add_space(4.0);
                        }
                        ToolbarMenu::Highlight => {
                            if tool_icon(ui, c, false, icons::CARET_LEFT, "Voltar").clicked() {
                                st.toolbar_menu = ToolbarMenu::None;
                            }
                            ui.spacing_mut().item_spacing.x = 8.0;
                            ui.label(RichText::new("Marca-texto").size(12.5).color(c.dim));
                            ui.add_space(2.0);
                            if swatch(ui, Color32::TRANSPARENT, cur_hl == 0, "Sem marca-texto", 20.0).clicked() {
                                fmt = Some(Format::Highlight(0));
                            }
                            // Exibido na ordem das cores (o índice salvo no documento não muda).
                            for idx in HIGHLIGHT_ORDER {
                                let name = HIGHLIGHTS[idx as usize - 1].0;
                                if swatch(ui, c.highlight_color(idx), cur_hl == idx, name, 20.0).clicked() {
                                    fmt = Some(Format::Highlight(idx));
                                }
                            }
                            ui.add_space(4.0);
                        }
                    });
                });
        });

    let mut changed = None;
    if let Some(f) = fmt {
        let ok = apply_format(note, range.clone(), f);
        changed = Some(ok);
        if matches!(f, Format::Color(_) | Format::Highlight(_)) {
            st.toolbar_menu = ToolbarMenu::None;
        }
    }
    if let Some(prefix) = list {
        changed = Some(toggle_line_prefix(ctx, edit_id, note, range.clone(), &prefix));
    }
    if copy_md {
        let mut sub = Doc::default();
        let bs = note.doc.byte_index(range.start);
        let be = note.doc.byte_index(range.end);
        sub.text = note.doc.text[bs..be].to_string();
        sub.styles = note.doc.styles[range.clone()].to_vec();
        ctx.copy_text(sub.to_markdown());
        st.toast(ctx, "Copiado como Markdown");
    }
    if fmt.is_some() || copy_md || changed.is_some() {
        // Devolve o foco ao editor mantendo a seleção.
        ctx.memory_mut(|m| m.request_focus(edit_id));
    }
    (resp.response.rect, changed)
}

fn tool(
    ui: &mut Ui,
    c: &Colors,
    active: bool,
    tip: &str,
    draw: impl FnOnce(&egui::Painter, Rect, Color32),
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(30.0, 28.0), Sense::CLICK);
    let bg = if active {
        theme::mix(c.surface, c.accent, if c.dark { 0.35 } else { 0.2 })
    } else if resp.hovered() {
        theme::mix(c.surface, c.text, 0.09)
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 7.0, bg);
    let col = if active { c.text } else { theme::mix(c.text, c.surface, 0.12) };
    draw(ui.painter(), rect, col);
    resp.on_hover_text(tip)
}

const TAB_RADIUS: f32 = 8.0;

/// Curva côncava na base da aba (`dir` = -1 esquerda, +1 direita).
/// `corner` é o canto inferior da aba.
fn tab_flare(p: &egui::Painter, corner: Pos2, dir: f32, col: Color32) {
    let r = TAB_RADIUS;
    let center = pos2(corner.x + dir * r, corner.y - r);
    let n = 12;
    let arc: Vec<Pos2> = (0..=n)
        .map(|i| {
            let a = i as f32 / n as f32 * std::f32::consts::FRAC_PI_2;
            pos2(center.x - dir * r * a.cos(), center.y + r * a.sin())
        })
        .collect();
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(corner, col);
    for q in &arc {
        mesh.colored_vertex(*q, col);
    }
    for i in 1..=n as u32 {
        mesh.add_triangle(0, i, i + 1);
    }
    p.add(mesh);
    // Borda suavizada (o mesh não tem antisserrilhado).
    p.add(egui::Shape::line(arc, Stroke::new(1.0, col)));
}

fn tool_icon(ui: &mut Ui, c: &Colors, active: bool, glyph: &str, tip: &str) -> egui::Response {
    tool(ui, c, active, tip, |p, rect, col| {
        p.text(rect.center(), Align2::CENTER_CENTER, glyph, icons::font(18.0), col);
    })
}

/// Pontinho com a cor atual no canto do botão (cor do texto / marca-texto).
fn swatch_dot(p: &egui::Painter, rect: Rect, col: Color32, c: &Colors) {
    let center = pos2(rect.right() - 7.5, rect.bottom() - 7.5);
    p.circle_filled(center, 4.0, c.surface);
    p.circle_filled(center, 3.0, col);
}

fn divider(ui: &mut Ui, c: &Colors) {
    let (rect, _) = ui.allocate_exact_size(vec2(9.0, 28.0), Sense::hover());
    ui.painter().vline(
        rect.center().x,
        rect.y_range().shrink(6.0),
        Stroke::new(1.0, c.border),
    );
}

fn swatch(ui: &mut Ui, col: Color32, selected: bool, tip: &str, size: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::CLICK);
    let p = ui.painter();
    let vis = ui.visuals();
    let r = rect.shrink(if resp.hovered() { 0.0 } else { 1.5 });
    if col == Color32::TRANSPARENT {
        p.circle_stroke(r.center(), r.width() / 2.0, Stroke::new(1.2, vis.weak_text_color()));
        p.line_segment(
            [r.left_bottom() + vec2(4.0, -4.0), r.right_top() + vec2(-4.0, 4.0)],
            Stroke::new(1.2, Color32::from_rgb(239, 68, 68)),
        );
    } else {
        p.circle_filled(r.center(), r.width() / 2.0, col);
    }
    if selected {
        p.circle_stroke(
            rect.center(),
            rect.width() / 2.0 + 2.0,
            Stroke::new(2.0, vis.strong_text_color()),
        );
    }
    resp.on_hover_text(tip)
}

fn icon_button(ui: &mut Ui, icon: &str, tip: &str, c: &Colors) -> egui::Response {
    icon_toggle(ui, icon, false, tip, c)
}

fn icon_toggle(ui: &mut Ui, icon: &str, on: bool, tip: &str, c: &Colors) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(28.0, 24.0), Sense::click());
    let bg = if on {
        theme::mix(c.panel, c.accent, 0.35)
    } else if resp.hovered() {
        theme::mix(c.panel, c.text, 0.1)
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 6.0, bg);
    let col = if on || resp.hovered() { c.text } else { c.dim };
    ui.painter()
        .text(rect.center(), Align2::CENTER_CENTER, icon, icons::font(16.0), col);
    resp.on_hover_text(tip)
}

fn section(ui: &mut Ui, title: &str, c: &Colors) {
    ui.add_space(20.0);
    ui.label(bold(title, 14.0).color(c.text));
    ui.add_space(8.0);
}

/// Tecla de atalho desenhada como "tecla" (fundo sutil, cantos arredondados).
fn key_chip(ui: &mut Ui, keys: &str, c: &Colors) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (i, part) in keys.split(" / ").enumerate() {
            if i > 0 {
                ui.label(RichText::new("/").size(12.0).color(c.dim));
            }
            Frame::new()
                .fill(c.code_bg)
                .corner_radius(CornerRadius::same(5))
                .inner_margin(Margin::symmetric(6, 1))
                .show(ui, |ui| {
                    ui.label(RichText::new(part).size(12.0).color(c.text));
                });
        }
    });
}

/// Ordem de exibição do marca-texto (por cor); os índices salvos não mudam.
const HIGHLIGHT_ORDER: [u8; 6] = [5, 1, 2, 3, 6, 4];

/// Item de menu com ícone, rótulo e atalho alinhado à direita.
fn menu_item(ui: &mut Ui, c: &Colors, glyph: &str, label: &str, key: &str, enabled: bool) -> bool {
    let b = egui::Button::new((
        RichText::new(glyph).font(icons::font(15.0)).color(c.dim),
        RichText::new(label),
    ))
    .shortcut_text(RichText::new(key).size(12.5).color(c.dim))
    .min_size(vec2(0.0, 28.0));
    ui.add_enabled(enabled, b).clicked()
}

struct SwitchItem {
    title: String,
    snip: String,
    badge: Option<&'static str>,
    target: Result<NoteId, usize>,
}

fn list_item(ui: &mut Ui, item: &SwitchItem, query: &str, selected: bool, c: &Colors) -> egui::Response {
    let w = ui.available_width();
    let h = if item.snip.is_empty() { 34.0 } else { 52.0 };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, 8.0, theme::mix(c.surface, c.accent, if c.dark { 0.22 } else { 0.12 }));
    } else if resp.hovered() {
        p.rect_filled(rect, 8.0, theme::mix(c.surface, c.text, 0.05));
    }
    let x = rect.left() + 12.0;
    let icon = if item.target.is_ok() { icons::FILE_TEXT } else { icons::HISTORY };
    p.text(pos2(x, rect.top() + 9.0), Align2::LEFT_TOP, icon, icons::font(16.0), c.dim);
    let tx = x + 26.0;
    let title = p.text(
        pos2(tx, rect.top() + 8.0),
        Align2::LEFT_TOP,
        short_title(&item.title),
        FontId::proportional(14.5),
        c.text,
    );
    if let Some(badge) = item.badge {
        let g = p.layout_no_wrap(badge.to_string(), FontId::proportional(11.0), c.dim);
        let r = Rect::from_min_size(
            pos2(title.right() + 8.0, title.center().y - g.size().y / 2.0 - 2.0),
            g.size() + vec2(12.0, 4.0),
        );
        p.rect_filled(r, 6.0, theme::mix(c.surface, c.text, 0.08));
        p.galley(r.min + vec2(6.0, 2.0), g, c.dim);
    }
    if !item.snip.is_empty() {
        // Trecho com o termo buscado destacado.
        let mut job = egui::text::LayoutJob::default();
        let base = egui::TextFormat::simple(FontId::proportional(12.5), c.dim);
        let hi = egui::TextFormat {
            color: c.text,
            background: theme::mix(c.surface, Color32::from_rgb(255, 200, 0), 0.35),
            ..base.clone()
        };
        let lower = item.snip.to_lowercase();
        match (!query.is_empty()).then(|| lower.find(query)).flatten() {
            Some(start) if lower.len() == item.snip.len() => {
                let end = start + query.len();
                job.append(&item.snip[..start], 0.0, base.clone());
                job.append(&item.snip[start..end], 0.0, hi);
                job.append(&item.snip[end..], 0.0, base);
            }
            _ => job.append(&item.snip, 0.0, base),
        }
        job.wrap.max_width = w - 50.0;
        job.wrap.max_rows = 1;
        let g = ui.fonts_mut(|f| f.layout_job(job));
        ui.painter().galley(pos2(tx, rect.top() + 29.0), g, c.dim);
    }
    resp
}

/// Barra de busca compacta, flutuando sobre o canto superior direito do painel.
fn find_bar(ui: &mut Ui, f: &mut FindState, note_id: NoteId, c: &Colors) -> bool {
    let mut keep = true;
    Frame::new()
        .fill(c.surface)
        .stroke(Stroke::new(1.0, c.border))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin { left: 10, right: 4, top: 4, bottom: 4 })
        .shadow(ui.ctx().global_style().visuals.popup_shadow)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.label(RichText::new(icons::SEARCH).font(icons::font(15.0)).color(c.dim));
                let r = ui.add(
                    egui::TextEdit::singleline(&mut f.query)
                        .id(Id::new(("dn-find", note_id)))
                        .desired_width(200.0)
                        .frame(Frame::NONE)
                        .min_size(vec2(0.0, 24.0))
                        .vertical_align(Align::Center)
                        .hint_text("Buscar nesta nota"),
                );
                if f.focus {
                    r.request_focus();
                    f.focus = false;
                }
                if r.changed() {
                    f.stale = true;
                    f.scroll = true;
                    f.current = 0;
                }
                let total = f.matches.len();
                let (enter, shift, esc) = ui.input(|i| {
                    (i.key_pressed(Key::Enter), i.modifiers.shift, i.key_pressed(Key::Escape))
                });
                if r.lost_focus() && enter && total > 0 {
                    f.current = if shift {
                        (f.current + total - 1) % total
                    } else {
                        (f.current + 1) % total
                    };
                    f.scroll = true;
                    r.request_focus();
                }
                if esc {
                    keep = false;
                }
                // Contador com largura fixa: a barra não muda de tamanho ao digitar.
                let (label, color) = if f.query.is_empty() {
                    (String::new(), c.dim)
                } else if total == 0 {
                    ("Nada".to_string(), c.danger())
                } else {
                    (format!("{} de {}", f.current + 1, total), c.dim)
                };
                ui.add_sized(
                    vec2(64.0, 24.0),
                    egui::Label::new(RichText::new(label).size(12.0).color(color)),
                );
                let sep = ui.allocate_exact_size(vec2(1.0, 18.0), Sense::hover()).0;
                ui.painter().vline(sep.center().x, sep.y_range(), Stroke::new(1.0, c.border));
                if icon_button(ui, icons::CARET_UP, "Anterior (Shift+Enter)", c).clicked() && total > 0 {
                    f.current = (f.current + total - 1) % total;
                    f.scroll = true;
                }
                if icon_button(ui, icons::CARET_DOWN, "Próximo (Enter)", c).clicked() && total > 0 {
                    f.current = (f.current + 1) % total;
                    f.scroll = true;
                }
                if icon_button(ui, icons::X, "Fechar (Esc)", c).clicked() {
                    keep = false;
                }
            });
        });
    keep
}

/// Busca sem diferenciar maiúsculas/minúsculas. Retorna intervalos em chars.
fn find_all(text: &str, query: &str) -> Vec<Range<usize>> {
    if query.is_empty() {
        return vec![];
    }
    let fold = |c: char| c.to_lowercase().next().unwrap_or(c);
    let t: Vec<char> = text.chars().map(fold).collect();
    let q: Vec<char> = query.chars().map(fold).collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i + q.len() <= t.len() {
        if t[i..i + q.len()] == q[..] {
            out.push(i..i + q.len());
            i += q.len();
        } else {
            i += 1;
        }
        if out.len() > 5000 {
            break;
        }
    }
    out
}

fn matches_query(title: &str, text: &str, q: &str) -> Option<String> {
    let first_line = |t: &str| -> String {
        t.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .nth(1)
            .unwrap_or("")
            .chars()
            .take(70)
            .collect()
    };
    if q.is_empty() {
        return Some(first_line(text));
    }
    if title.to_lowercase().contains(q) {
        return Some(first_line(text));
    }
    for line in text.lines() {
        if line.to_lowercase().contains(q) {
            return Some(line.trim().chars().take(70).collect());
        }
    }
    None
}

// ============================================================== utilidades

/// Mostra as notas do release (Markdown simples do CHANGELOG), com recuo
/// deslocado nos itens de lista.
fn release_notes(ui: &mut Ui, notes: &str, c: &Colors) {
    let clean = |t: &str| t.replace("**", "").replace('`', "");
    ui.spacing_mut().item_spacing.y = 6.0;
    for line in notes.lines() {
        let t = line.trim_end();
        if let Some(h) = t.strip_prefix("### ").or_else(|| t.strip_prefix("## ")) {
            ui.add_space(4.0);
            ui.label(bold(clean(h), 13.5).color(c.text));
        } else if let Some(item) = t.trim_start().strip_prefix("- ").or_else(|| t.trim_start().strip_prefix("* ")) {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                ui.label(RichText::new("•").color(c.dim));
                ui.add(egui::Label::new(clean(item)).wrap());
            });
        } else if !t.trim().is_empty() {
            ui.add(egui::Label::new(clean(t)).wrap());
        }
    }
}

fn line_col(text: &str, char_idx: usize) -> (usize, usize) {
    let mut ln = 1;
    let mut col = 1;
    for c in text.chars().take(char_idx) {
        if c == '\n' {
            ln += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (ln, col)
}

fn file_name(p: &std::path::Path) -> String {
    p.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("arquivo")
        .to_owned()
}

fn sanitize(s: &str) -> String {
    let s: String = s
        .chars()
        .map(|c| if "\\/:*?\"<>|".contains(c) { '_' } else { c })
        .collect();
    let s = s.trim().trim_end_matches('…').trim().to_string();
    if s.is_empty() { "Nota".into() } else { s }
}

// ============================================================== eframe::App

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, frame: &mut eframe::Frame) {
        let ctx = self.ctx.clone();
        let now = ctx.input(|i| i.time);
        self.update_titlebar(frame);
        self.global_input();
        self.poll_background(now);
        let c = self.colors();

        if self.settings.show_status_bar {
            egui::Panel::bottom("dn-status")
                .exact_size(30.0)
                .show_separator_line(false)
                .frame(
                    Frame::new()
                        .fill(c.panel)
                        .inner_margin(Margin::symmetric(12, 3)),
                )
                .show(ui, |ui| self.status_bar(ui, &c));
        }

        egui::CentralPanel::default()
            .frame(Frame::new().fill(c.panel))
            .show(ui, |ui| {
                if self.dock.main_surface().num_tabs() == 0 {
                    ui.painter().rect_filled(ui.max_rect(), 0.0, c.bg);
                    self.empty_state(ui, &c);
                    return;
                }
                let style = self.dock_style(&c);
                let mut changed = false;
                let split = self.dock.iter_leaves().count() > 1;
                // Cor de fundo da aba ativa de cada painel (a do painel em foco é a do editor).
                let focused_leaf = self.dock.focused_leaf();
                let unfocused_bg = theme::mix(c.bg, c.panel, 0.45);
                let mut tab_bg = HashMap::new();
                for (path, leaf) in self.dock.iter_leaves() {
                    if let Some(t) = leaf.tabs().get(leaf.active.0) {
                        let bg = if Some(path) == focused_leaf || !split { c.bg } else { unfocused_bg };
                        tab_bg.insert(*t, bg);
                    }
                }
                let mut tab_rects = HashMap::new();
                let mut viewer = Viewer {
                    notes: &mut self.notes,
                    settings: &self.settings,
                    colors: c,
                    caps: self.caps,
                    ui: &mut self.ui,
                    actions: &mut self.actions,
                    changed: &mut changed,
                    active: &mut self.active,
                    split,
                    now,
                    tab_bg: &tab_bg,
                    tab_rects: &mut tab_rects,
                };
                DockArea::new(&mut self.dock)
                    .id(Id::new("dn-dock"))
                    .style(style)
                    .show_add_buttons(false)
                    .show_close_buttons(true)
                    .show_leaf_close_all_buttons(false)
                    .show_leaf_collapse_buttons(false)
                    .show_window_close_buttons(false)
                    .show_window_collapse_buttons(false)
                    .allowed_splits(AllowedSplits::All)
                    .draggable_tabs(true)
                    .show_inside(ui, &mut viewer);
                if changed {
                    self.mark_dirty();
                }
                if let Some(id) = self.dock.find_active_focused().map(|(_, t)| *t) {
                    self.active = Some(id);
                }
                self.add_tab_buttons(ui, &tab_rects, &c);
            });

        self.overlays(&c);
        self.process_actions();
        self.update_window_title();
        self.autosave(now);

        if ctx.input(|i| i.viewport().close_requested()) {
            self.save_now();
        }
    }

    #[cfg(feature = "glow")]
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_now();
    }

    #[cfg(not(feature = "glow"))]
    fn on_exit(&mut self) {
        self.save_now();
    }
}
