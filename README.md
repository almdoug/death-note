# Death Note

Bloco de notas em abas, rápido e leve, escrito em Rust ([egui](https://github.com/emilk/egui)).

## Recursos

- **Abas** com títulos automáticos (primeira linha) ou renomeáveis (duplo clique).
- **Tela dividida**: arraste uma aba para a metade de um painel para dividir; arraste de volta para juntar. Também pelo clique direito na aba ou pelo menu ☰ (“Desfazer divisões”).
- **Barra de formatação flutuante** que aparece sobre o texto selecionado: negrito, itálico, sublinhado, tachado, código, títulos H1–H3, cor do texto, marca-texto, checklist, lista, copiar como Markdown e limpar formatação.
- **Checklists clicáveis** (☐/☑) e continuação automática de listas ao pressionar Enter.
- **Nunca perde nada**: a sessão é gravada continuamente com escrita atômica (arquivo temporário + `fsync` + renomeação) e um backup do último estado bom. Se o PC desligar de repente, tudo estará lá ao reabrir.
- Abas fechadas vão para **Fechadas recentemente** (`Ctrl+Shift+T` reabre).
- **Arquivos**: salve/abra `.dnote` (com formatação), `.md` (Markdown) ou `.txt`. Arrastar arquivos para a janela abre-os.
- **Busca** na nota (`Ctrl+F`) e **busca rápida** entre todas as notas (`Ctrl+P`).
- **Tema** escuro, claro ou do sistema; **cor de destaque** e **cor de fundo** personalizáveis; barra de título do Windows acompanha o tema.
- Janela **sempre visível** (📌), tela cheia (`F11`), zoom do texto (`Ctrl` + roda).
- Leve: ~90 MB de RAM e 0% de CPU parado (renderização só quando algo muda).

## Instalação

Baixe o `DeathNote-Setup-X.Y.Z.exe` na página de [releases](https://github.com/almdoug/death-note/releases/latest) e execute.
Não precisa de administrador (instala em `%LOCALAPPDATA%\Programs\Death Note`). O instalador:

- cria o atalho no **Menu Iniciar** (é só pesquisar "Death Note") e, se quiser, na área de trabalho;
- adiciona **"Abrir com Death Note"** ao menu de contexto dos arquivos (no Windows 11 fica em "Mostrar mais opções");
- faz os arquivos `.dnote` abrirem no Death Note e coloca o app em "Abrir com" para `.txt` e `.md`;
- permite abrir pelo Executar (`Win+R` → `death-note`).

Abrir um arquivo com o app já aberto leva o arquivo para a janela existente.
Desinstalar pelo Windows não apaga suas notas (ficam em `%APPDATA%\DeathNote`).

Também há uma versão portátil (`DeathNote-X.Y.Z-portable.exe`), que não se atualiza sozinha.

## Atualizações

O app verifica o último release do GitHub ao abrir (dá para desligar nas configurações).
Quando há versão nova aparece um aviso na barra de status; o app baixa o instalador, confere o
SHA-256, instala em silêncio e reabre com tudo como estava.

Para publicar uma versão: suba a `version` no `Cargo.toml`, descreva as mudanças no `CHANGELOG.md`,
faça o commit e envie uma tag `vX.Y.Z`. A CI gera o instalador e publica o release.
Detalhes em [CLAUDE.md](CLAUDE.md).

## Atalhos

| Atalho | Ação |
|---|---|
| `Ctrl+T` / `Ctrl+N` | Nova nota |
| `Ctrl+W` | Fechar nota |
| `Ctrl+Shift+T` | Reabrir nota fechada |
| `Ctrl+Tab` / `Ctrl+Shift+Tab` | Próxima / anterior aba |
| `Ctrl+O` | Abrir arquivo |
| `Ctrl+S` / `Ctrl+Shift+S` | Salvar / Salvar como |
| `Ctrl+F` | Buscar na nota |
| `Ctrl+P` | Busca rápida entre notas |
| `Ctrl+B` / `Ctrl+I` / `Ctrl+U` | Negrito / Itálico / Sublinhado |
| `Ctrl+Shift+X` | Tachado |
| `Ctrl+E` | Código |
| `Ctrl+1` / `2` / `3` / `0` | Títulos / texto normal |
| `Ctrl+L` / `Ctrl+Shift+L` | Checklist / lista |
| `Ctrl+\` | Limpar formatação |
| `F5` | Inserir data e hora |
| `Ctrl+Z` / `Ctrl+Y` | Desfazer / refazer |
| `Ctrl+,` | Configurações |
| `F11` | Tela cheia |

## Compilar

```sh
cargo run --release
```

O executável fica em `target/release/death-note.exe`.

Os dados ficam em `%APPDATA%\DeathNote` (Windows), `~/Library/Application Support/DeathNote` (macOS)
ou `~/.local/share/death-note` (Linux). A variável `DEATHNOTE_DATA_DIR` permite usar outra pasta.

## Estrutura

| Arquivo | Conteúdo |
|---|---|
| `src/doc.rs` | Modelo do documento (texto + estilo por caractere), Markdown, desfazer/refazer |
| `src/editor.rs` | `TextBuffer` rico para o `TextEdit` do egui e montagem do layout formatado |
| `src/app.rs` | Abas/divisão (egui_dock), barra flutuante, atalhos, busca, configurações, autosalvamento |
| `src/storage.rs` | Sessão, configurações, arquivos `.dnote/.md/.txt`, trava de instância única |
| `src/theme.rs` | Temas claro/escuro e cores |
| `src/fonts.rs` | Fontes do sistema (negrito/itálico reais) |
| `src/platform.rs` | Integrações do Windows (barra de título, foco da janela existente) |
| `src/ipc.rs` | Repasse de arquivos para a instância já aberta |
| `src/updater.rs` | Atualização automática pelo GitHub Releases |
| `installer/death-note.iss` | Instalador (Inno Setup 6) |
