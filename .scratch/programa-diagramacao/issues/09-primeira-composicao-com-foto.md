# 09 — Primeira composição com Foto

**What to build:** permitir que o usuário importe um JPEG vinculado, coloque-o em um Frame, ajuste o enquadramento básico e obtenha a mesma composição depois de salvar, reabrir e exportar.

**Blocked by:** 03 — Mídias externas e Cache; 08 — Esqueleto ponta a ponta.

**Type:** implementation

**Status:** resolved

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [ADR 0001 — vincular Arquivos externos](../../../docs/adr/0001-vincular-arquivos-externos.md); [estrutura da Janela do Projeto](../../../docs/design/0001-estrutura-da-janela-do-projeto.md).

- [x] Um controle mínimo de Fotos importa um JPEG por diálogo nativo e guarda somente seu vínculo externo.
- [x] Adicionar a Foto à Lâmina cria um Frame retangular e aplica Preenchimento do Frame sem área vazada.
- [x] Arrastar sobre um Frame preenche ou substitui somente o alvo explícito; dois cliques preenchem primeiro o placeholder mais à esquerda e, sem placeholder, criam um novo Frame segundo o modo atual.
- [x] Em sobreposição, usar somente o Frame mais acima cujo retângulo contém o ponteiro, independentemente de conteúdo ou Opacidade, sem gesto para alternar o alvo.
- [x] No Modo de edição, soltar em área vazia cria Frame proporcional centralizado na soltura e deslocado para dentro dos limites sem redução; no modo normal, o primeiro Layout compatível define a geometria.
- [x] Durante o arraste, mostrar apenas destaque do Frame ou da Lâmina alvo, sem preview da Foto, do novo Frame ou do Layout; `Esc`, alvo inválido e soltura externa não modificam o Projeto.
- [x] Ordenar placeholders do duplo clique pela borda esquerda e, em empate, pela borda superior.
- [x] Após inserir, preencher ou substituir, selecionar somente o Frame afetado e atualizar o Painel contextual, sem entrada adicional de Undo/Redo.
- [x] O Zoom base mínimo é calculado separadamente do ajuste de Zoom do usuário.
- [x] Fora do Modo de edição, o usuário aplica Pan básico por `Alt` + arraste sobre o Frame e Zoom por `Alt` + roda, com Undo/Redo e sem mover a geometria.
- [x] Frame, vínculo, Zoom e Pan sobrevivem a `Salvar` e reabertura sem alterar o original.
- [x] A Exportação JPEG lê o arquivo original e corresponde visualmente ao editor, mesmo que o Cache esteja vazio.
- [x] Um original ausente impede a Exportação com mensagem acionável; a existência de Cache não produz falso sucesso.

## Resolução

- Implementação local concluída no commit `b617b646c9eaa4bf915ddd769cb8e7e7b44dd8fa`, a partir do ponto fixo `b24faf2b61d13bc9e4b069f20131cd952cafa236`.
- Evidência produtiva: `docs/research/artifacts/0037-issue-17-first-photo-composition.json`, 13/13 verificações, `sourceInputsDirty=false` e limpeza concluída.
- Revisões independentes finais Standards e Spec: `CLEAN`.
