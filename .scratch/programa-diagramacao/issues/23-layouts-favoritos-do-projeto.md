# 23 — Layouts favoritos do Projeto

**What to build:** permitir favoritar um Layout dentro do Projeto como uma cópia estável e portátil, garantindo que ele viaje com o Projeto e não dependa mais do item global que o originou.

**Blocked by:** 11 — Salvar como e cópia explícita; 20 — Aplicação de Layouts; 22 — Layouts personalizados globais.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [estrutura da Janela do Projeto](../../../docs/design/0001-estrutura-da-janela-do-projeto.md).

- [ ] A estrela permite favoritar tanto Layouts automáticos quanto personalizados e copia integralmente sua definição para o Projeto atual.
- [ ] A estrela preenchida indica a cópia favorita pertencente ao Projeto e permite desfavoritar sem alterar a composição aplicada.
- [ ] Alternar a estrela é imediato e sem confirmação, marca alterações pendentes e constitui uma única ação de Undo/Redo.
- [ ] Favoritar não move a preview entre `Automáticos` e `Personalizados`; a seção continua representando a origem do Layout.
- [ ] Favoritos aparecem antes dos demais candidatos em sua seção, mas depois do Último Layout aplicado daquela categoria, sem duplicar previews.
- [ ] Editar, renomear ou excluir o Layout global de origem não altera a cópia favorita.
- [ ] Sem origem global, desfavoritar remove a preview, exceto no painel de uma Lâmina que ainda conserve sua própria cópia como Último Layout aplicado.
- [ ] O favorito sobrevive a `Salvar`, reabertura, `Salvar como` e Cópia externa do Projeto.
- [ ] Alterar favoritos de um Projeto nunca altera outro Projeto aberto.
- [ ] Favoritos participam da ordenação e compatibilidade definidas para o painel de Layouts.
- [ ] Em empate dentro da mesma prioridade, preservar a ordem persistida no Projeto e usar o identificador estável do Layout como último desempate; salvar e reabrir não pode reordenar candidatos equivalentes.
- [ ] Testes comprovam independência após edição e exclusão da origem e depois da cópia do Projeto.
