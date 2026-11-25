//! Persistência: sessão automática (à prova de quedas de energia),
//! configurações e arquivos `.dnote` / `.md` / `.txt`.

use crate::doc::{Doc, DocData};
use crate::panes::SavedTabs;
use crate::theme::Settings;
use egui_dock::DockState;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

pub type NoteId = u64;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct NoteData {
    pub id: NoteId,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub custom_title: bool,
    pub doc: DocData,
    #[serde(default)]
    pub file: Option<PathBuf>,
    #[serde(default)]
    pub file_dirty: bool,
    #[serde(default)]
    pub created: i64,
    #[serde(default)]
    pub modified: i64,
    #[serde(default)]
    pub cursor: usize,
}

#[derive(Serialize, Deserialize, Default)]
pub struct SessionData {
    pub version: u32,
    pub next_id: NoteId,
    pub notes: Vec<NoteData>,
    /// Layout das versões até 0.4 (com divisões do egui_dock). Continua sendo
    /// gravado, sem divisões, para que versões antigas abram a sessão.
    #[serde(default)]
    pub dock: Option<DockState<NoteId>>,
    /// Abas, com os grupos em tela dividida (0.5+).
    #[serde(default)]
    pub tabs: Option<SavedTabs>,
    #[serde(default)]
    pub closed: Vec<NoteData>,
    #[serde(default)]
    pub window: Option<WindowGeom>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct WindowGeom {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub maximized: bool,
}

/// Arquivo `.dnote` (JSON com formatação).
#[derive(Serialize, Deserialize)]
struct DnoteFile {
    format: String,
    version: u32,
    #[serde(default)]
    title: String,
    doc: DocData,
}

pub fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("DEATHNOTE_DATA_DIR") {
        let p = PathBuf::from(dir);
        let _ = fs::create_dir_all(&p);
        return p;
    }
    #[cfg(windows)]
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("DeathNote");
    #[cfg(target_os = "macos")]
    let base = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Library/Application Support/DeathNote");
    #[cfg(all(unix, not(target_os = "macos")))]
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("death-note");
    let _ = fs::create_dir_all(&base);
    base
}

fn session_path() -> PathBuf {
    data_dir().join("session.json")
}
fn settings_path() -> PathBuf {
    data_dir().join("settings.json")
}

/// Escrita atômica: grava num arquivo temporário, força o flush para o disco
/// e só então substitui o original. Se o PC desligar no meio, o arquivo
/// anterior continua íntegro.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension(format!(
        "{}.tmp",
        path.extension().and_then(|e| e.to_str()).unwrap_or("")
    ));
    {
        let mut f = File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)
}

pub fn save_session(s: &SessionData) -> Result<(), String> {
    let json = serde_json::to_vec(s).map_err(|e| e.to_string())?;
    atomic_write(&session_path(), &json).map_err(|e| e.to_string())
}

/// Carrega a sessão; se o arquivo principal estiver corrompido, tenta o backup.
pub fn load_session() -> Option<SessionData> {
    let main = session_path();
    let candidates = [
        main.clone(),
        main.with_extension("json.tmp"),
        main.with_extension("bak"),
    ];
    for (i, p) in candidates.iter().enumerate() {
        if let Ok(bytes) = fs::read(p) {
            if let Ok(s) = serde_json::from_slice::<SessionData>(&bytes) {
                if i == 0 {
                    // Guarda uma cópia do último estado bom conhecido.
                    let _ = atomic_write(&main.with_extension("bak"), &bytes);
                }
                return Some(s);
            }
        }
    }
    None
}

pub fn load_settings() -> Settings {
    fs::read(settings_path())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

pub fn save_settings(s: &Settings) {
    if let Ok(json) = serde_json::to_vec_pretty(s) {
        let _ = atomic_write(&settings_path(), &json);
    }
}

// ---------------------------------------------------------------- Arquivos

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Dnote,
    Markdown,
    Text,
}

pub fn kind_of(path: &Path) -> FileKind {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("dnote") => FileKind::Dnote,
        Some("md") | Some("markdown") => FileKind::Markdown,
        _ => FileKind::Text,
    }
}

pub fn write_file(path: &Path, title: &str, doc: &Doc) -> Result<(), String> {
    let bytes = match kind_of(path) {
        FileKind::Dnote => serde_json::to_vec_pretty(&DnoteFile {
            format: "death-note".into(),
            version: 1,
            title: title.to_owned(),
            doc: doc.to_data(),
        })
        .map_err(|e| e.to_string())?,
        FileKind::Markdown => doc.to_markdown().into_bytes(),
        FileKind::Text => doc.text.clone().into_bytes(),
    };
    atomic_write(path, &bytes).map_err(|e| e.to_string())
}

/// Lê um arquivo e devolve (título sugerido, documento).
pub fn read_file(path: &Path) -> Result<(String, Doc), String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Nota")
        .to_owned();
    match kind_of(path) {
        FileKind::Dnote => {
            let f: DnoteFile = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            let title = if f.title.is_empty() { stem } else { f.title };
            Ok((title, Doc::from_data(f.doc)))
        }
        FileKind::Markdown => Ok((stem, Doc::from_markdown(&decode_text(&bytes)))),
        FileKind::Text => Ok((stem, Doc::from_plain(&decode_text(&bytes)))),
    }
}

/// Texto UTF-8 sem o BOM que muitos editores do Windows gravam no início.
fn decode_text(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    String::from_utf8_lossy(bytes).into_owned()
}

// --------------------------------------------------------- Instância única

/// Mantém um arquivo de trava aberto exclusivamente enquanto o app roda,
/// para impedir que duas instâncias sobrescrevam a mesma sessão.
pub struct InstanceLock {
    _file: File,
}

pub fn acquire_lock() -> Option<InstanceLock> {
    let path = data_dir().join("instance.lock");
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .share_mode(0)
            .open(path)
            .ok()
            .map(|f| InstanceLock { _file: f })
    }
    #[cfg(not(windows))]
    {
        fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .ok()
            .map(|f| InstanceLock { _file: f })
    }
}
