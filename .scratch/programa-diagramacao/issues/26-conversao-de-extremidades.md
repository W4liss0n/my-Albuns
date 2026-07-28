# 26 — Conversão de extremidades

**What to build:** permitir converter Lâminas inicial e final entre Lâmina dupla e página única, reorganizando o conteúdo para a superfície ativa de forma previsível e sem conservar conteúdo inacessível no lado desativado.

**Blocked by:** 16 — Background e Overlay; 21 — Layout travado.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [estrutura da Janela do Projeto](../../../docs/design/0001-estrutura-da-janela-do-projeto.md).

- [ ] Somente Lâminas elegíveis nas extremidades oferecem a conversão; papéis são recalculados após reordenação.
- [ ] Expor `Converter extremidade` no menu `Lâmina` para a extremidade centralizada e no menu de contexto para a extremidade clicada; desabilitar o comando em alvos internos ou inválidos.
- [ ] Manter a conversão desabilitada no Modo de edição e exigir retorno ao Canvas contínuo.
- [ ] `Design do Álbum > Estrutura` oferece controles independentes `Primeira Lâmina` e `Última Lâmina`, cada um com `Lâmina dupla` ou `Página única`, e um `Aplicar` com resumo de impacto.
- [ ] Ao converter para página única, a interface informa a perda/reorganização necessária e exige confirmação antes de descartar conteúdo do lado desativado.
- [ ] Background, Overlay e demais propriedades são reajustados para o único lado ativo conforme seus escopos e regras de herança.
- [ ] A conversão preserva Fotos, placeholders e estilos, descarta a geometria anterior e destrava a organização; havendo Frames, aplica o primeiro Layout compatível destravado, e sem Frames mantém a Lâmina sem Layout.
- [ ] O lado desativado não conserva conteúdo oculto, não aceita interação e não gera unidade na Exportação.
- [ ] Na expansão para Lâmina dupla, aplicações por lado preservam o lado existente e iniciam o novo lado em `default`; aplicações de `Ambos os lados` são reajustadas à nova superfície.
- [ ] A conversão inteira é uma ação de Undo/Redo e preserva o estado anterior se cancelada ou se nenhum Layout compatível puder ser aplicado.
- [ ] O resultado persiste após reabertura e coincide no Canvas, previews e Exportação JPEG.
