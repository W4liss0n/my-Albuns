# 29 — Limpeza de saídas órfãs

**What to build:** ao sobrescrever uma Exportação completa de imagens, remover arquivos antigos que ainda correspondam exatamente ao namespace de Nome e formato do mesmo Projeto, evitando saídas órfãs de uma versão anterior maior.

**Blocked by:** 27 — Exportação JPEG e PNG completa.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [ADR 0003 — limpeza pela nomeação](../../../docs/adr/0003-limpar-saidas-orfas-pela-nomeacao.md); [ADR 0006 — publicação com transação limitada](../../../docs/adr/0006-publicar-exportacao-com-transacao-limitada.md).

- [ ] Antes de iniciar, o sistema identifica candidatos órfãos somente por Nome, extensão e índice da convenção documentada, sem criar manifesto auxiliar.
- [ ] O namespace `{nome-do-projeto}_{índice}` não codifica o modo de Exportação; `Por lâmina` e `Por página` compartilham o mesmo conjunto de nomes.
- [ ] A lista de conflitos e órfãos é abrangida pela confirmação explícita de `Sobrescrever todos`.
- [ ] Uma Exportação completa confirmada pode substituir o conjunto anterior mesmo quando muda entre `Por lâmina` e `Por página`; o conjunto publicado passa a representar somente a nova entrega.
- [ ] Nenhum órfão é removido antes que todas as novas saídas tenham sido renderizadas, verificadas e publicadas com sucesso.
- [ ] Em qualquer falha durante a publicação, preservar todos os candidatos órfãos. Não existe rollback do conjunto; se já houve promoção de arquivos, informar a possível mistura de saídas anteriores e novas e orientar uma nova Exportação integral.
- [ ] A limpeza ocorre somente para JPEG/PNG do Álbum inteiro; Exportação parcial nunca remove saídas fora da seleção.
- [ ] Como o destino não possui manifesto, uma Exportação parcial sobre arquivos existentes deve avisar que não é possível provar o modo anterior e não declara a pasta como um conjunto completo coerente; uma Exportação completa restabelece o conjunto autoritativo.
- [ ] Arquivos com outro Nome ou extensão são preservados; a consequência aceita para arquivo manual indistinguível é documentada na confirmação.
- [ ] Reduzir, por exemplo, de 36 para 34 unidades deixa exatamente as 34 saídas atuais depois do sucesso.
- [ ] Testes cobrem sucesso, falha, intervalo parcial, outra extensão, outro Nome e colisão manual indistinguível.
