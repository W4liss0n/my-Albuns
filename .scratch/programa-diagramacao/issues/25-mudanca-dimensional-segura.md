# 25 — Mudança dimensional segura

**What to build:** permitir alterar dimensões físicas e orientação dentro dos limites seguros, transformando proporcionalmente toda a composição e recusando mudanças nas quais a qualidade do resultado não possa ser garantida.

**Blocked by:** 07 — Transformação dimensional; 16 — Background e Overlay; 21 — Layout travado.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [ADR 0004 — margens dentro da dimensão exportada](../../../docs/adr/0004-manter-margens-dentro-da-dimensao-exportada.md).

- [ ] A interface pré-valida a nova dimensão e explica quando a diferença de proporção ou orientação excede o limite documentado.
- [ ] Unidade converte imediatamente apenas os valores exibidos; largura, altura e DPI ficam pendentes até um único `Aplicar`, que valida e apresenta tamanho físico e resolução finais.
- [ ] Uma mudança aceita atualiza proporcionalmente posições, dimensões, estilos dependentes de medida, Backgrounds, Overlays e Frames.
- [ ] Pan, Zoom base, Zoom adicional, Giro de 90°, Ângulo fino, espelhamento e ponto focal preservam o enquadramento relativo da Foto.
- [ ] Estado do Layout aplicado, ordem e escopo permanecem coerentes após a transformação.
- [ ] Alterar apenas a Unidade converte os valores exibidos sem mudar a dimensão física nem a saída em pixels.
- [ ] Alterar apenas o DPI preserva medidas físicas e toda a composição, recalculando somente a resolução em pixels das representações derivadas e da Exportação.
- [ ] A aplicação é atômica e uma única ação de Undo/Redo; erro ou cancelamento preserva exatamente o estado anterior.
- [ ] Confirmar a aplicação altera somente a sessão aberta e mantém o Projeto pendente até o comando manual `Salvar`.
- [ ] O estado transformado sobrevive a `Salvar`/reabrir e a Exportação corresponde ao preview.
- [ ] Testes cobrem proporção igual, mudança moderada, limite aceito, limite rejeitado e conversões de Unidade.
