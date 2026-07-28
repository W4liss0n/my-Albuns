# 35 — Duplicação de Lâmina

**What to build:** permitir criar, imediatamente depois da origem, uma cópia completa e independente de uma Lâmina válida, reutilizando seus vínculos externos sem sincronizar edições futuras.

**Blocked by:** 16 — Background e Overlay; 21 — Layout travado.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [estrutura da Janela do Projeto](../../../docs/design/0001-estrutura-da-janela-do-projeto.md).

- [ ] Oferecer `Duplicar Lâmina` no menu superior `Lâmina` e no menu de contexto da superfície ou da Barra da Lâmina; o menu superior usa a Lâmina mais centralizada e o menu de contexto usa a Lâmina clicada.
- [ ] Desabilitar o comando no Modo de edição e quando a origem for uma extremidade de Página única.
- [ ] Inserir a cópia imediatamente depois da origem como uma nova Lâmina independente.
- [ ] Copiar integralmente Background, Overlay, seus estados de herança ou personalização, Frames, Fotos ou placeholders, estilos, ajustes não destrutivos, Pilha visual, Último Layout aplicado e estado de Layout travado.
- [ ] Reutilizar os mesmos vínculos com Arquivos originais sem duplicar mídias ou Cache; qualquer edição posterior em uma das Lâminas não modifica a outra.
- [ ] Recalcular papéis e Numeração de Página, mantendo válidas as regras das extremidades.
- [ ] Centralizar a nova Lâmina no Canvas sem incluir essa posição de navegação no Histórico.
- [ ] Registrar toda a duplicação como uma única ação de Undo/Redo, capaz de remover ou restaurar a cópia completa.
- [ ] A cópia sobrevive a `Salvar`/reabrir e produz na Exportação a mesma composição visual da origem no instante da duplicação.
- [ ] Testes cobrem conteúdo herdado e personalizado, Fotos e placeholders, Pilha visual, Layout travado, vínculos externos, isolamento posterior, Undo/Redo e indisponibilidade em Página única.
