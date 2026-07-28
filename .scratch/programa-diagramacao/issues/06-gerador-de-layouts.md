# 06 — Gerador de Layouts

**What to build:** decidir posteriormente o contrato do algoritmo que produzirá organizações compatíveis para a quantidade atual de Frames, depois que a aplicação de Layouts estiver validada com definições de teste.

**Blocked by:** 20 — Aplicação de Layouts.

**Type:** decision

**Status:** ready-for-human

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [ADR 0008 — arranjo de reserva de Layout](../../../docs/adr/0008-garantir-layout-compativel-por-arranjo-de-reserva.md); [propriedade de estado e módulos do núcleo](../../../docs/design/0012-propriedade-de-estado-e-modulos-do-nucleo.md).

- [ ] Usar as evidências e o contrato de integração entregues pelo ticket 20 para decidir entradas, saídas, invariantes e falhas do Gerador de Layouts.
- [ ] Tratar a garantia de ao menos um candidato compatível como requisito herdado, não como decisão deste ticket: ela pertence a `LayoutRules` e ao arranjo de reserva do [ADR 0008](../../../docs/adr/0008-garantir-layout-compativel-por-arranjo-de-reserva.md). O Gerador melhora a qualidade dos candidatos e nunca é o único caminho para existir um.
- [ ] Definir diversidade, ordenação estável e prioridade do Último Layout aplicado.
- [ ] No escopo de Página, impedir Frames sobre a divisão central e centralizar globalmente cada Bloco de Frames em sua Página.
- [ ] No escopo de Lâmina, permitir que Frames e Blocos ocupem uma ou as duas Páginas.
- [ ] Definir compatibilidade por quantidade de Frames, proporção e escopo, incluindo o desempate do primeiro Layout usado nas automações.
- [ ] Comparar alternativas somente depois de medir a experiência com fixtures; não introduzir geração procedural no MVP antes dessa decisão.
- [ ] Manter o Gerador como fornecedor de candidatos para `LayoutRules`; somente criar um `LayoutEngine` separado se busca, ranqueamento, diversidade, sementes ou orçamento de tempo justificarem um seam próprio.
- [ ] Encerrar a decisão com ADR, exemplos e testes de contrato. A implementação do algoritmo será planejada em ticket posterior e não é condição de conclusão do ticket 20.
