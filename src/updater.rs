//! Atualização automática via GitHub Releases.
//!
//! 1. Consulta `releases/latest` do repositório.
//! 2. Se a versão for maior que a atual, mostra o aviso no app.
//! 3. Ao confirmar: baixa o instalador e o `SHA256SUMS.txt`, confere o hash,
//!    executa o instalador em modo silencioso (`/RELAUNCH=1` reabre o app)
//!    e fecha o app.
//!
//! As requisições usam o `curl` do sistema (presente no Windows 10/11),
//! evitando embutir uma pilha HTTPS inteira no executável.

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

pub const REPO: &str = "almdoug/death-note";

fn repo() -> String {
    std::env::var("DEATHNOTE_UPDATE_REPO").unwrap_or_else(|_| REPO.to_string())
}

/// Versão em execução (pode ser simulada com `DEATHNOTE_FAKE_VERSION` para testes).
pub fn current_version() -> String {
    std::env::var("DEATHNOTE_FAKE_VERSION").unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string())
}

#[derive(Clone, Debug)]
pub struct Release {
    pub version: String,
    pub notes: String,
    pub page: String,
    pub installer: Option<Asset>,
    pub sums: Option<Asset>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
    #[serde(default)]
    pub size: u64,
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Clone, Debug)]
pub enum State {
    Idle,
    Checking,
    UpToDate { manual: bool },
    Available(Release),
    Downloading { release: Release, path: PathBuf },
    Ready { release: Release, installer: PathBuf },
    Failed { message: String, manual: bool },
}

pub type Shared = Arc<Mutex<State>>;

pub fn new_shared() -> Shared {
    Arc::new(Mutex::new(State::Idle))
}

/// O app foi instalado pelo instalador (e não é a versão portátil)?
pub fn is_installed() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("unins000.exe").exists()))
        .unwrap_or(false)
}

fn curl() -> Command {
    let mut c = Command::new("curl");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c.args([
        "-fsSL",
        "--retry",
        "2",
        "-H",
        "User-Agent: DeathNote-Updater",
    ]);
    c
}

fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let v = v.trim().trim_start_matches(['v', 'V']);
    let core = v.split(['-', '+']).next()?;
    let mut it = core.split('.').map(|p| p.parse::<u64>().ok());
    Some((it.next()??, it.next().flatten().unwrap_or(0), it.next().flatten().unwrap_or(0)))
}

pub fn is_newer(remote: &str, local: &str) -> bool {
    match (parse_version(remote), parse_version(local)) {
        (Some(r), Some(l)) => r > l,
        _ => false,
    }
}

fn fetch_latest() -> Result<Option<Release>, String> {
    let url = format!("https://api.github.com/repos/{}/releases/latest", repo());
    let out = curl()
        .args(["--max-time", "20", "-H", "Accept: application/vnd.github+json", &url])
        .output()
        .map_err(|e| format!("curl indisponível: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        // 404 = repositório ainda sem releases
        if err.contains("404") {
            return Ok(None);
        }
        return Err(format!("sem conexão com o GitHub ({})", err.trim()));
    }
    let api: ApiRelease =
        serde_json::from_slice(&out.stdout).map_err(|e| format!("resposta inválida: {e}"))?;
    if api.draft || api.prerelease {
        return Ok(None);
    }
    let installer = api
        .assets
        .iter()
        .find(|a| a.name.starts_with("DeathNote-Setup") && a.name.ends_with(".exe"))
        .cloned();
    let sums = api.assets.iter().find(|a| a.name == "SHA256SUMS.txt").cloned();
    Ok(Some(Release {
        version: api.tag_name.trim_start_matches('v').to_string(),
        notes: api.body.unwrap_or_default(),
        page: api.html_url,
        installer,
        sums,
    }))
}

/// Verifica em segundo plano. `manual` = o usuário pediu (mostra "já está atualizado").
pub fn check(shared: &Shared, manual: bool) {
    {
        let mut s = shared.lock().unwrap();
        if matches!(*s, State::Checking { .. } | State::Downloading { .. } | State::Ready { .. }) {
            return;
        }
        *s = State::Checking;
    }
    let shared = shared.clone();
    std::thread::spawn(move || {
        let result = fetch_latest();
        let new_state = match result {
            Ok(Some(r)) if is_newer(&r.version, &current_version()) => State::Available(r),
            Ok(_) => State::UpToDate { manual },
            Err(message) => State::Failed { message, manual },
        };
        *shared.lock().unwrap() = new_state;
        wake();
    });
}

fn wake() {
    if let Some(ctx) = crate::ipc::CTX.get() {
        ctx.request_repaint();
    }
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut h = Sha256::new();
    std::io::copy(&mut f, &mut h).map_err(|e| e.to_string())?;
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Baixa e confere o instalador em segundo plano.
pub fn download(shared: &Shared, release: Release) {
    let Some(asset) = release.installer.clone() else {
        *shared.lock().unwrap() = State::Failed {
            message: "este release não tem instalador".into(),
            manual: true,
        };
        return;
    };
    let dir = std::env::temp_dir().join("DeathNote-update");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(&asset.name);
    let _ = std::fs::remove_file(&path);
    *shared.lock().unwrap() = State::Downloading {
        release: release.clone(),
        path: path.clone(),
    };
    let shared = shared.clone();
    std::thread::spawn(move || {
        let result = (|| -> Result<(), String> {
            let ok = curl()
                .args(["--max-time", "600", "-o"])
                .arg(&path)
                .arg(&asset.browser_download_url)
                .status()
                .map_err(|e| e.to_string())?
                .success();
            if !ok {
                return Err("falha ao baixar o instalador".into());
            }
            // Confere o SHA-256 publicado junto com o release.
            if let Some(sums) = &release.sums {
                let out = curl()
                    .args(["--max-time", "30", &sums.browser_download_url])
                    .output()
                    .map_err(|e| e.to_string())?;
                let text = String::from_utf8_lossy(&out.stdout);
                let expected = text
                    .lines()
                    .find(|l| l.contains(&asset.name))
                    .and_then(|l| l.split_whitespace().next())
                    .map(|s| s.to_ascii_lowercase());
                let actual = sha256_file(&path)?;
                if expected.as_deref() != Some(actual.as_str()) {
                    let _ = std::fs::remove_file(&path);
                    return Err("o arquivo baixado não confere (SHA-256)".into());
                }
            }
            Ok(())
        })();
        *shared.lock().unwrap() = match result {
            Ok(()) => State::Ready {
                release,
                installer: path,
            },
            Err(message) => State::Failed {
                message,
                manual: true,
            },
        };
        wake();
    });
}

/// Executa o instalador em modo silencioso; ele fecha este app e o reabre.
pub fn run_installer(installer: &Path) -> Result<(), String> {
    Command::new(installer)
        .args([
            "/VERYSILENT",
            "/SUPPRESSMSGBOXES",
            "/NORESTART",
            "/CLOSEAPPLICATIONS",
            "/RELAUNCH=1",
        ])
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_compare() {
        assert!(is_newer("v0.3.0", "0.2.9"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(is_newer("0.2.10", "0.2.9"));
        assert!(!is_newer("v0.2.0", "0.2.0"));
        assert!(!is_newer("0.1.9", "0.2.0"));
        assert!(!is_newer("lixo", "0.2.0"));
    }
}
