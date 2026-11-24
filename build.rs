//! Embute ícone e informações de versão no executável do Windows
//! (aparecem no Explorer, Menu Iniciar e barra de tarefas).

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico")
            .set("ProductName", "Death Note")
            .set("FileDescription", "Death Note")
            .set("OriginalFilename", "death-note.exe")
            .set("LegalCopyright", "almdoug");
        if let Err(e) = res.compile() {
            // Sem o Windows SDK (rc.exe) o app ainda compila, só sem ícone embutido.
            println!("cargo:warning=não foi possível embutir o ícone: {e}");
        }
    }
}
