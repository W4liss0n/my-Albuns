# 24 — Remoção de imagens e Decorativos

**What to build:** permitir remover itens do Painel de imagens com consequências explícitas e consistentes em todos os seus usos, sem deixar referências quebradas ou violar um Layout travado.

**Blocked by:** 19 — Painel de imagens; 21 — Layout travado.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [ADR 0001 — vincular Arquivos externos](../../../docs/adr/0001-vincular-arquivos-externos.md).

- [ ] Com foco no Painel de imagens, `Delete` e `Remover` no menu de contexto executam a remoção sobre toda a seleção da aba ativa.
- [ ] Remover uma Foto não usada a exclui do Painel sem alterar o arquivo original.
- [ ] Remover uma seleção formada somente por Fotos não usadas retira diretamente todos os itens do Painel.
- [ ] Se ao menos uma Foto selecionada estiver em uso, um único diálogo consolidado informa o impacto e oferece `Remover tudo`, `Remover imagens e manter os Frames` ou `Cancelar` para toda a seleção.
- [ ] `Remover tudo` elimina as Fotos selecionadas e seus Frames não travados; posições de Layout travado preservam o Frame e tornam-se placeholders.
- [ ] `Remover imagens e manter os Frames` esvazia todas as ocorrências usadas das Fotos selecionadas e deixa placeholders explícitos; as selecionadas sem uso apenas saem do Painel.
- [ ] Um único Decorativo sem uso pode ser removido diretamente; um Decorativo usado ou qualquer seleção múltipla abre uma única confirmação conjunta, nunca uma confirmação por item.
- [ ] Um uso customizado removido volta ao default atual quando o Decorativo não era o próprio padrão; remover o padrão atualiza corretamente herdeiros e ausências personalizadas.
- [ ] Nenhuma operação remove ou modifica os arquivos originais no sistema.
- [ ] Cada remoção confirmada, individual ou em lote, participa de uma única ação de Undo/Redo, persiste corretamente e atualiza filtros e validação de Exportação.
