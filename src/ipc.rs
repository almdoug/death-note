//! Comunicação entre instâncias: quando o app já está aberto e o usuário
//! abre um arquivo (menu de contexto, "Abrir com", duplo clique), a nova
//! instância repassa os caminhos para a janela existente e encerra.
//!
//! Usa um socket TCP em 127.0.0.1 com porta aleatória e um token secreto,
//! ambos gravados em `instance.port` na pasta de dados.

use crate::storage;
use std::hash::{BuildHasher, Hasher};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

/// Contexto do egui, para acordar a interface quando chega uma mensagem.
pub static CTX: OnceLock<eframe::egui::Context> = OnceLock::new();

pub enum Message {
    /// Abrir estes arquivos (lista vazia = só trazer a janela para frente).
    Open(Vec<PathBuf>),
}

fn port_file() -> PathBuf {
    storage::data_dir().join("instance.port")
}

fn random_token() -> String {
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
    );
    h.write_u32(std::process::id());
    let a = h.finish();
    let mut h2 = std::collections::hash_map::RandomState::new().build_hasher();
    h2.write_u64(a);
    format!("{a:016x}{:016x}", h2.finish())
}

/// Inicia o servidor (só a instância principal chama isto).
pub fn serve() -> Option<Receiver<Message>> {
    let listener = TcpListener::bind("127.0.0.1:0").ok()?;
    let port = listener.local_addr().ok()?.port();
    let token = random_token();
    storage::atomic_write(&port_file(), format!("{port} {token}").as_bytes()).ok()?;
    let (tx, rx) = channel();
    std::thread::Builder::new()
        .name("dn-ipc".into())
        .spawn(move || accept_loop(listener, token, tx))
        .ok()?;
    Some(rx)
}

fn accept_loop(listener: TcpListener, token: String, tx: Sender<Message>) {
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
        let mut buf = Vec::new();
        let _ = (&mut stream).take(256 * 1024).read_to_end(&mut buf);
        let text = String::from_utf8_lossy(&buf);
        let mut lines = text.lines();
        if lines.next() != Some(token.as_str()) {
            continue;
        }
        let paths: Vec<PathBuf> = lines
            .filter(|l| !l.trim().is_empty())
            .map(PathBuf::from)
            .collect();
        let _ = stream.write_all(b"ok");
        if tx.send(Message::Open(paths)).is_err() {
            break;
        }
        if let Some(ctx) = CTX.get() {
            ctx.request_repaint();
        }
    }
}

/// Envia os caminhos para a instância principal. Retorna `true` se conseguiu.
pub fn send(paths: &[PathBuf]) -> bool {
    let Ok(content) = std::fs::read_to_string(port_file()) else {
        return false;
    };
    let mut parts = content.split_whitespace();
    let (Some(port), Some(token)) = (parts.next(), parts.next()) else {
        return false;
    };
    let Ok(port) = port.parse::<u16>() else {
        return false;
    };
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(800)) else {
        return false;
    };
    let mut msg = format!("{token}\n");
    for p in paths {
        msg.push_str(&p.to_string_lossy());
        msg.push('\n');
    }
    if stream.write_all(msg.as_bytes()).is_err() {
        return false;
    }
    let _ = stream.shutdown(std::net::Shutdown::Write);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut ack = [0u8; 2];
    stream.read_exact(&mut ack).is_ok() && &ack == b"ok"
}
