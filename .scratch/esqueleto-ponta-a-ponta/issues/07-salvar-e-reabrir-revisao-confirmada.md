# 07 — Salvar e reabrir a revisão confirmada

**What to build:** permitir que a pessoa salve explicitamente a Revisão visível e, ao abrir novamente o Projeto, recupere exatamente o estado confirmado em uma nova Sessão com Histórico vazio.

**Blocked by:** 06 — Alterar DPI com Undo e Redo.

**Type:** implementation

**Status:** resolved

**Normative sources:** [ticket pai](../../programa-diagramacao/issues/08-esqueleto-ponta-a-ponta.md); [especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [política de caminhos](../../../docs/design/0011-resolucao-e-politica-de-caminhos.md); [propriedade de estado e módulos do núcleo](../../../docs/design/0012-propriedade-de-estado-e-modulos-do-nucleo.md); [Contrato do Arquivo de Projeto v1](../../../docs/design/0013-contrato-do-arquivo-de-projeto-v1.md); [contrato público de persistência](../../../docs/design/0015-contrato-publico-de-persistencia-do-project-core.md); [prova de Salvamento e trava](../../fase-2-fluxo-persistente/issues/04-provar-o-salvamento-atomico-com-trava.md).

- [x] `Salvar` envia ao `ProjectCore` a Revisão esperada que está visível no momento da solicitação e o Host serializa Salvamento e comandos criativos sobre a mesma `EditableProject`.
- [x] `ProjectStore` permanece concreto e privado; frontend e Host não recebem bytes persistidos, DTOs versionados ou a prova privada de Publicação.
- [x] O Salvamento usa a barreira irmã estável derivada da Identidade, temporário irmão sincronizado, revalidação por handle, substituição atômica quando comprovada e nova trava antes de confirmar a candidata.
- [x] O destino precisa continuar sendo o mesmo objeto físico com os mesmos bytes do baseline; a verificação posterior confirma identidade, pathname e bytes publicados.
- [x] `Saved` confirma exatamente a Revisão candidata, remove as mudanças criativas pendentes e mantém o Histórico de Undo/Redo disponível na Sessão.
- [x] `AlreadyCurrent` não reescreve o arquivo; `StaleRevision` ocorre antes de I/O e permite atualizar a projeção; `PersistedBaselineConflict` preserva a Sessão sem sobrescrever o destino.
- [x] Falha conclusiva mantém a Sessão e seu estado anterior; `SaveStateIndeterminate` não confirma a Revisão, não tenta novamente automaticamente e invalida a Sessão de forma fechada.
- [x] Os resultados atravessam Tauri como códigos e dados estruturados; mensagens localizadas pertencem ao adaptador de interface.
- [x] Depois de Salvar, Undo e Redo continuam utilizáveis. Encerrar a Sessão limpa e abrir o mesmo arquivo em outro Host restaura os valores e a Revisão persistidos.
- [x] A nova Sessão inicia com `savedRevision` igual à Revisão persistida, sem mudanças pendentes e com Undo e Redo indisponíveis.
- [x] Os antigos atalhos para obter o documento serializado ou confirmar publicamente uma Revisão salva não integram a nova superfície produtiva.
- [x] Testes pela fronteira pública cobrem sucesso, `AlreadyCurrent`, pedido obsoleto, conflito de baseline, falha conclusiva, estado inconclusivo, Histórico após Salvar e reabertura em outro Host.

## Comments

- `Salvar` recebe a Revisão visível pela superfície pública de `EditableProject`/`ProjectCore`; o Host serializa Salvamento, Undo, Redo e intenções criativas sob o mesmo mutex e devolve resultado e projeção autoritativa sob a mesma guarda.
- `ProjectStore`, baseline persistido, bytes versionados e prova de Publicação permanecem privados. O protocolo usa barreira irmã estável, temporário irmão sincronizado, revalidação de identidade, pathname e bytes, substituição atômica comprovada e renovação da trava antes de confirmar a candidata.
- `AlreadyCurrent` não executa I/O e `StaleRevision` é recusado antes de I/O. Conflitos e falhas conclusivas preservam a Sessão; qualquer resultado fisicamente inconclusivo retorna `SaveStateIndeterminate`, invalida a Sessão de forma fechada e não é repetido automaticamente.
- O contrato Tauri transporta apenas códigos e dados estruturados. O adaptador da interface valida o envelope, localiza as mensagens em português e o botão `Salvar`/`Ctrl+S` usa o mesmo executor serial das demais mutações.
- Testes pela fronteira pública e com arquivos reais cobrem sucesso, ausência de escrita em `AlreadyCurrent`, pedido obsoleto, divergência de bytes, troca de identidade física, falha conclusiva, estado inconclusivo, Histórico após Salvamento e reabertura limpa em outro Host.
- Evidências finais: `npm run test` — 25 arquivos e 180 testes; `npm run test:rust`; `npm run check:rust`; `npm run quality:rust`; `npm run contract:check`; `npm run build`; `git diff --check`.
- As revisões independentes finais de especificação, padrões, atomicidade e simplicidade não encontraram problema material restante que justificasse ampliar a implementação.
