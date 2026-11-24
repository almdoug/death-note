# Changelog

Todas as mudanças visíveis para quem usa o Death Note. A seção de cada versão
vira o texto do release no GitHub e aparece na janela de atualização do app.

## [0.4.0] - 2026-11-23

### Novidades
- `Alt+Shift+R` mostra o arquivo da nota no Explorer (também no menu e no clique direito da aba).
- O Executar agora abre o app com `Win+R` → `dn`.

### Melhorias
- A aba ativa tem curvas na base que se unem ao texto, como as abas do navegador.
- Botão "+" com espaço depois das abas e realce arredondado.
- "×" das abas redesenhado: aparece na aba ativa e ao passar o mouse, com realce arredondado.
- Texto dos campos de busca (Ctrl+F e Ctrl+P) alinhado com o ícone.

## [0.3.0] - 2026-11-23

### Correções
- O app travava ao abrir se todas as abas tivessem sido fechadas antes. Agora abre normalmente, com as notas fechadas preservadas.
- Com muitas abas, uma barra de rolagem cobria o topo do texto; títulos longos ocupavam a barra inteira.
- O divisor da tela dividida era difícil de pegar e deixava um painel ser esmagado até sumir.
- Na janela estreita, os textos da barra de status se sobrepunham.
- Textos na cor de destaque, código no tema claro e caixas de seleção agora têm contraste suficiente.

### Melhorias
- Ícones novos e consistentes em todo o app, no lugar de emojis e símbolos.
- Barra de formatação: as cores e o marca-texto aparecem no lugar dos botões, sem cobrir o texto acima; mostra a cor atual e o marca-texto como fica aplicado.
- Busca na nota (Ctrl+F) compacta, flutuando no canto, sem empurrar o texto.
- Busca rápida (Ctrl+P) fixa no topo, com o termo destacado e a nota atual marcada.
- Configurações reorganizadas, com lista completa de atalhos e confirmação antes de restaurar os padrões.
- Renomear já abre com o nome selecionado.
- Avisos ao fechar, reabrir e copiar como Markdown; indicação do estilo pendente (ex.: negrito ativado sem seleção).
- Notas novas vazias levam a hora no nome; o título da janela mostra a nota aberta.
- Na tela dividida, uma linha colorida indica o painel em foco.
- Cursor de mão sobre as caixinhas de checklist.

## [0.2.0] - 2026-11-23

### Novidades
- Instalador para Windows: atalho no Menu Iniciar, "Abrir com Death Note" no menu de contexto, arquivos `.dnote` abrem direto no app e o Death Note aparece em "Abrir com" para `.txt` e `.md`.
- Abra o app pelo Executar: `Win+R` → `dnote`.
- Atualização automática: o app avisa quando há uma versão nova, baixa, confere e instala sozinho, reabrindo em seguida.
- Abrir um arquivo com o app já aberto agora leva o arquivo para a janela existente.
- Novo ícone.

## [0.1.0] - 2026-11-23

### Novidades
- Primeira versão: abas, tela dividida arrastando abas, barra de formatação flutuante, checklists, listas automáticas, busca, temas e salvamento automático à prova de queda de energia.
