# 07 — Salvar e reabrir a revisão confirmada

**What to build:** permitir que a pessoa salve explicitamente a Revisão visível e, ao abrir novamente o Projeto, recupere exatamente o estado confirmado em uma nova Sessão com Histórico vazio.

**Blocked by:** 06 — Alterar DPI com Undo e Redo.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [ticket pai](../../programa-diagramacao/issues/08-esqueleto-ponta-a-ponta.md); [especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [política de caminhos](../../../docs/design/0011-resolucao-e-politica-de-caminhos.md); [propriedade de estado e módulos do núcleo](../../../docs/design/0012-propriedade-de-estado-e-modulos-do-nucleo.md); [Contrato do Arquivo de Projeto v1](../../../docs/design/0013-contrato-do-arquivo-de-projeto-v1.md); [contrato público de persistência](../../../docs/design/0015-contrato-publico-de-persistencia-do-project-core.md); [prova de Salvamento e trava](../../fase-2-fluxo-persistente/issues/04-provar-o-salvamento-atomico-com-trava.md).

- [ ] `Salvar` envia ao `ProjectCore` a Revisão esperada que está visível no momento da solicitação e o Host serializa Salvamento e comandos criativos sobre a mesma `EditableProject`.
- [ ] `ProjectStore` permanece concreto e privado; frontend e Host não recebem bytes persistidos, DTOs versionados ou a prova privada de Publicação.
- [ ] O Salvamento usa a barreira irmã estável derivada da Identidade, temporário irmão sincronizado, revalidação por handle, substituição atômica quando comprovada e nova trava antes de confirmar a candidata.
- [ ] O destino precisa continuar sendo o mesmo objeto físico com os mesmos bytes do baseline; a verificação posterior confirma identidade, pathname e bytes publicados.
- [ ] `Saved` confirma exatamente a Revisão candidata, remove as mudanças criativas pendentes e mantém o Histórico de Undo/Redo disponível na Sessão.
- [ ] `AlreadyCurrent` não reescreve o arquivo; `StaleRevision` ocorre antes de I/O e permite atualizar a projeção; `PersistedBaselineConflict` preserva a Sessão sem sobrescrever o destino.
- [ ] Falha conclusiva mantém a Sessão e seu estado anterior; `SaveStateIndeterminate` não confirma a Revisão, não tenta novamente automaticamente e invalida a Sessão de forma fechada.
- [ ] Os resultados atravessam Tauri como códigos e dados estruturados; mensagens localizadas pertencem ao adaptador de interface.
- [ ] Depois de Salvar, Undo e Redo continuam utilizáveis. Encerrar a Sessão limpa e abrir o mesmo arquivo em outro Host restaura os valores e a Revisão persistidos.
- [ ] A nova Sessão inicia com `savedRevision` igual à Revisão persistida, sem mudanças pendentes e com Undo e Redo indisponíveis.
- [ ] Os antigos atalhos para obter o documento serializado ou confirmar publicamente uma Revisão salva não integram a nova superfície produtiva.
- [ ] Testes pela fronteira pública cobrem sucesso, `AlreadyCurrent`, pedido obsoleto, conflito de baseline, falha conclusiva, estado inconclusivo, Histórico após Salvar e reabertura em outro Host.
