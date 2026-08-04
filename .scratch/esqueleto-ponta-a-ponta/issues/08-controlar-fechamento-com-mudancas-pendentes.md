# 08 — Controlar o fechamento com mudanças pendentes

**What to build:** proteger o trabalho ao fechar uma Janela de Projeto, oferecendo `Salvar e fechar`, `Descartar e fechar` ou `Cancelar` quando existirem mudanças criativas pendentes.

**Blocked by:** 07 — Salvar e reabrir a revisão confirmada.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [ticket pai](../../programa-diagramacao/issues/08-esqueleto-ponta-a-ponta.md); [estrutura da Janela do Projeto](../../../docs/design/0001-estrutura-da-janela-do-projeto.md); [Tela de Boas-vindas](../../../docs/design/0002-tela-de-boas-vindas.md); [propriedade de estado e módulos do núcleo](../../../docs/design/0012-propriedade-de-estado-e-modulos-do-nucleo.md); [contrato público de persistência](../../../docs/design/0015-contrato-publico-de-persistencia-do-project-core.md).

- [ ] Fechar uma Janela sem mudanças criativas pendentes encerra diretamente o Host e libera sua `EditableProject`.
- [ ] Fechar com mudanças pendentes, tanto pelo comando do aplicativo quanto pelo botão nativo, apresenta exatamente `Salvar e fechar`, `Descartar e fechar` e `Cancelar`.
- [ ] Enquanto a decisão ou o Salvamento de fechamento estiver em andamento, novos comandos criativos e operações incompatíveis permanecem bloqueados.
- [ ] `Cancelar` não inicia I/O e mantém Janela, Sessão, projeção, Histórico e mudanças pendentes.
- [ ] `Descartar e fechar` consome a `EditableProject` sem chamar o `ProjectStore`; alterações não salvas permanecem fora do arquivo.
- [ ] `Salvar e fechar` usa a Revisão corrente e só consome a Sessão depois de `Saved` ou `AlreadyCurrent`.
- [ ] Conflito de baseline ou falha conclusiva mantém a Janela aberta e o estado anterior; resultado inconclusivo nunca apresenta sucesso nem mantém uma Sessão potencialmente insegura.
- [ ] Encerrar a Sessão libera lease de Identidade, trava física e recursos do Host; a morte do processo oferece a mesma liberação pelo sistema operacional.
- [ ] Fechar a última Janela torna a Tela de Boas-vindas visível novamente sem transferir estado criativo ao processo global.
- [ ] Reabrir depois de descartar recupera o último estado salvo; reabrir depois de salvar e fechar recupera a Revisão recém-confirmada; nenhum caso preserva o Histórico entre Sessões.
- [ ] Testes cobrem fechamento limpo, as três escolhas, falha de Salvamento, resultado inconclusivo, liberação da exclusividade e reabertura do estado correspondente.
