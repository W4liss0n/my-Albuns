# 08 — Controlar o fechamento com mudanças pendentes

**What to build:** proteger o trabalho ao fechar uma Janela de Projeto, oferecendo `Salvar e fechar`, `Descartar e fechar` ou `Cancelar` quando existirem mudanças criativas pendentes.

**Blocked by:** 07 — Salvar e reabrir a revisão confirmada.

**Type:** implementation

**Status:** resolved

**Normative sources:** [ticket pai](../../programa-diagramacao/issues/08-esqueleto-ponta-a-ponta.md); [estrutura da Janela do Projeto](../../../docs/design/0001-estrutura-da-janela-do-projeto.md); [Tela de Boas-vindas](../../../docs/design/0002-tela-de-boas-vindas.md); [propriedade de estado e módulos do núcleo](../../../docs/design/0012-propriedade-de-estado-e-modulos-do-nucleo.md); [contrato público de persistência](../../../docs/design/0015-contrato-publico-de-persistencia-do-project-core.md).

- [x] Fechar uma Janela sem mudanças criativas pendentes encerra diretamente o Host e libera sua `EditableProject`.
- [x] Fechar com mudanças pendentes, tanto pelo comando do aplicativo quanto pelo botão nativo, apresenta exatamente `Salvar e fechar`, `Descartar e fechar` e `Cancelar`.
- [x] Enquanto a decisão ou o Salvamento de fechamento estiver em andamento, novos comandos criativos e operações incompatíveis permanecem bloqueados.
- [x] `Cancelar` não inicia I/O e mantém Janela, Sessão, projeção, Histórico e mudanças pendentes.
- [x] `Descartar e fechar` consome a `EditableProject` sem chamar o `ProjectStore`; alterações não salvas permanecem fora do arquivo.
- [x] `Salvar e fechar` usa a Revisão corrente e só consome a Sessão depois de `Saved` ou `AlreadyCurrent`.
- [x] Conflito de baseline ou falha conclusiva mantém a Janela aberta e o estado anterior; resultado inconclusivo nunca apresenta sucesso nem mantém uma Sessão potencialmente insegura.
- [x] Encerrar a Sessão libera lease de Identidade, trava física e recursos do Host; a morte do processo oferece a mesma liberação pelo sistema operacional.
- [x] Fechar a última Janela torna a Tela de Boas-vindas visível novamente sem transferir estado criativo ao processo global.
- [x] Reabrir depois de descartar recupera o último estado salvo; reabrir depois de salvar e fechar recupera a Revisão recém-confirmada; nenhum caso preserva o Histórico entre Sessões.
- [x] Testes cobrem fechamento limpo, as três escolhas, falha de Salvamento, resultado inconclusivo, liberação da exclusividade e reabertura do estado correspondente.

## Comments

- `ProjectHostState` representa explicitamente `Active`, `ClosePending` e `Consumed`. O comando `Arquivo > Fechar Projeto` e o botão nativo chegam ao mesmo ciclo de vida; durante pedido, decisão, Salvamento ou término, a interface e o Host recusam novas operações incompatíveis.
- O diálogo modal acessível oferece somente `Salvar e fechar`, `Descartar e fechar` e `Cancelar`. Cancelar restaura a mesma projeção e o mesmo Histórico sem I/O; descartar consome a Sessão sem persistir; salvar usa a Revisão corrente e só fecha após confirmação.
- Falhas conclusivas restauram a Sessão ativa e mantêm a Janela aberta. `SaveStateIndeterminate` consome a Sessão, nunca apresenta sucesso e exige reabertura; a cobertura combina a injeção real no núcleo, a classificação terminal do Host e o comportamento da interface.
- O encerramento instala atomicamente uma barreira por Janela contra registros tardios de Exportação, cancela tentativas existentes e aguarda a liberação efetiva dos recursos antes de destruir a Janela. Lease de Identidade e trava física são liberadas ao consumir a `EditableProject` ou, como garantia final, pela morte do processo.
- O Host inicia uma entrada global limpa, sem pathname nem estado criativo. Conforme a arquitetura normativa de Hosts isolados e entrada global descartável, não foi criado um coordenador especulativo entre Sessões.
- Reaberturas reais comprovam a revisão salva ou a última revisão confirmada após descarte, sempre com Histórico novo. O contrato IPC transporta somente escolhas, códigos, resultados e projeções tipadas; mensagens permanecem localizadas no adaptador da interface.
- Evidências finais: `npm test` com 26 arquivos e 188 testes; `npm run test:rust` com 301 testes aprovados e 5 gates externos ignorados; `npm run build`; `npm run typecheck`; `npm run check:rust`; `npm run quality:rust`; `npm run contract:check`; `git diff --check`.
- As revisões independentes finais de especificação e padrões não encontraram lacuna material restante. A duplicação mínima de `isRecord` entre dois adaptadores foi mantida para evitar acoplamento artificial; uma injeção produtiva exclusiva para o teste inconclusivo e um coordenador global de Hosts foram descartados como overengineering.
