# 07 — Transformação dimensional

**What to build:** definir quando uma mudança nas dimensões físicas do Projeto pode ser aplicada com segurança e como todo o conteúdo é transformado proporcionalmente sem perder o enquadramento das Fotos.

**Blocked by:** 01 — Plataforma e arquitetura.

**Type:** decision

**Status:** ready-for-human

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [ADR 0004 — margens dentro da dimensão exportada](../../../docs/adr/0004-manter-margens-dentro-da-dimensao-exportada.md).

- [ ] Definir um critério numérico e explicável para rejeitar mudanças de orientação ou proporção consideradas excessivamente diferentes.
- [ ] Definir transformação de posições e dimensões de Frames, Backgrounds, Overlays e demais geometrias persistidas.
- [ ] Definir a transformação de Pan, Zoom adicional, Zoom base de preenchimento e ponto focal da Foto.
- [ ] Preservar a dimensão física quando somente a Unidade de exibição for alterada.
- [ ] Definir validação prévia e aplicação atômica: uma mudança inválida ou incompleta não pode modificar parcialmente o Projeto.
- [ ] Registrar casos de referência para proporções iguais, próximas, limítrofes e rejeitadas.
- [ ] Encerrar a decisão com critério mensurável, exemplos aprovados e ADR antes da implementação no ticket 25.
