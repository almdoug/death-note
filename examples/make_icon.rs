//! Gera `assets/icon.ico` a partir do desenho em `src/icon.rs`.
//! Uso: `cargo run --example make_icon`

#[path = "../src/icon.rs"]
mod icon;

fn main() {
    let bytes = icon::ico(&[16, 20, 24, 32, 40, 48, 64, 128, 256]);
    std::fs::write("assets/icon.ico", &bytes).expect("gravar assets/icon.ico");
    println!("assets/icon.ico gerado ({} bytes)", bytes.len());
}
