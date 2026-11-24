# Death Note — instruções para o Claude

Bloco de notas em abas para Windows, em Rust (eframe/egui 0.37 + egui_dock 0.22,
renderizador glow). Interface e textos para o usuário em **português (PT-BR)**.

## Comandos

```sh
cargo build                      # debug (tem console para logs)
cargo test --release             # testes (mesmo perfil da CI)
cargo build --release            # target/release/death-note.exe
cargo run --example make_icon    # regenera assets/icon.ico a partir de src/icon.rs
iscc /DAppVersion=X.Y.Z installer\death-note.iss   # instalador em dist/ (Inno Setup 6)
```

Ao rodar o app para testar, **sempre** use uma pasta de dados separada, para não
mexer nas notas reais do usuário (`%APPDATA%\DeathNote`):

```powershell
$env:DEATHNOTE_DATA_DIR = "<pasta temporária>"; .\target\debug\death-note.exe
```

`DEATHNOTE_FAKE_VERSION=0.0.1` faz o app se achar antigo (para testar o atualizador).

## Regras

### Versão — sempre atualizar
- A versão fica **só** no `Cargo.toml` (`version = "X.Y.Z"`, SemVer). O executável,
  o instalador e o atualizador leem dela.
- Ao terminar **qualquer** tarefa que mude o app (código, instalador, ícone):
  - Se a versão atual **já foi lançada** (existe a tag `vX.Y.Z`: `git tag -l` /
    `git ls-remote --tags origin`), incremente:
    - **PATCH** (0.2.0 → 0.2.1): correções e ajustes pequenos;
    - **MINOR** (0.2.1 → 0.3.0): funcionalidades novas;
    - **MAJOR**: só com autorização do usuário.
  - Se a versão atual **ainda não foi lançada**, mantenha-a e acumule as mudanças
    (suba de PATCH para MINOR se entrou funcionalidade nova).
  - Rode `cargo build` para atualizar o `Cargo.lock`.
- Registre a mudança no `CHANGELOG.md`, na seção `## [X.Y.Z] - AAAA-MM-DD`, em
  linguagem para o usuário final (subseções `Novidades`, `Melhorias`, `Correções`).
  Esse texto vira a nota do release e aparece dentro do app.
- Mudanças só em documentação ou CI não mudam a versão.

### Lançamento — sempre perguntar
- Ao final de toda tarefa que mudou o app, **sempre pergunte ao usuário** se ele
  quer lançar a atualização (diga a versão e um resumo do que entra).
  **Nunca lance sem um "sim" explícito.**
- Se ele confirmar, faça tudo:
  1. `git status` limpo (fora as mudanças do lançamento); `cargo test --release`
     e `cargo build --release` sem erros.
  2. Ajuste a data da seção da versão no `CHANGELOG.md` para hoje.
  3. Commit `Lançar vX.Y.Z`.
  4. `git tag -a vX.Y.Z -m "Death Note vX.Y.Z"`.
  5. `git push origin main` e `git push origin vX.Y.Z`.
  6. A CI (`.github/workflows/build.yml`) confere que a tag bate com o `Cargo.toml`,
     gera `DeathNote-Setup-X.Y.Z.exe`, `DeathNote-X.Y.Z-portable.exe` e
     `SHA256SUMS.txt` e publica o release com as notas do CHANGELOG. Os apps
     instalados mostram o aviso de atualização e se atualizam sozinhos.
  7. Passe ao usuário o link `https://github.com/almdoug/death-note/actions` (e o
     do release quando terminar).
- Nunca apague, mova ou recrie uma tag já publicada. Se um release sair com
  problema, corrija e lance uma nova versão PATCH.

### Contratos que não podem quebrar (o atualizador depende deles)
- `AppId` em `installer/death-note.iss` — mudar faz o Windows instalar uma cópia nova
  em vez de atualizar.
- Nomes dos arquivos do release: `DeathNote-Setup-*.exe` e `SHA256SUMS.txt`.
- Parâmetro `/RELAUNCH=1` do instalador e o repositório `almdoug/death-note`
  (`src/updater.rs`).
- Formato da sessão (`session.json`) e do `.dnote`: só adicione campos com
  `#[serde(default)]`, para que versões novas leiam os dados das antigas.

### Git
- **Não** adicione `Co-Authored-By` nem qualquer menção ao Claude em commits ou PRs.
- Mensagens de commit em português.
- `git push` só no lançamento ou quando o usuário pedir. Nunca `--force` sem
  autorização.

### Qualidade
- Baixo consumo de RAM e CPU é requisito: o app deve ficar em 0% de CPU parado
  (renderiza só quando algo muda). Medido em ~88 MB de RAM com glow; o wgpu
  ficou em ~255 MB, por isso não é usado.
- Nada de dependências pesadas: o atualizador usa o `curl` do Windows em vez de
  uma biblioteca HTTPS.
- Compile sem warnings e rode os testes antes de entregar.
- Verifique mudanças de interface rodando o app (com `DEATHNOTE_DATA_DIR`).

## Estrutura

| Arquivo | Conteúdo |
|---|---|
| `src/main.rs` | Entrada, instância única, janela |
| `src/app.rs` | Abas/divisão, barra flutuante, atalhos, busca, configurações, autosalvamento, janela de atualização |
| `src/doc.rs` | Documento (texto + estilo por caractere), Markdown, desfazer/refazer |
| `src/editor.rs` | `TextBuffer` rico e layout formatado |
| `src/storage.rs` | Sessão, configurações, arquivos, trava de instância |
| `src/ipc.rs` | Repasse de arquivos para a instância já aberta |
| `src/updater.rs` | Verificação, download, conferência SHA-256 e instalação de atualizações |
| `src/theme.rs` / `src/fonts.rs` / `src/icon.rs` | Tema, fontes e ícone |
| `src/platform.rs` | Integrações do Windows |
| `installer/death-note.iss` | Instalador (Menu Iniciar, menu de contexto, associações) |
| `build.rs` | Embute ícone e versão no `.exe` |
