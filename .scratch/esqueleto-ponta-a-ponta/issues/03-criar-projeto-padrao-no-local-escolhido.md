# 03 — Criar um Projeto padrão no local escolhido

**What to build:** permitir que a pessoa percorra as duas etapas mínimas de `Novo Projeto`, escolha Nome e Localização no Windows e receba um Projeto neutro v1 já aberto no seu Host real.

**Blocked by:** 02 — Abrir um Projeto v1 em um Host independente.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [ticket pai](../../programa-diagramacao/issues/08-esqueleto-ponta-a-ponta.md); [criação de Projeto](../../../docs/design/0003-criacao-de-projeto.md); [política de caminhos](../../../docs/design/0011-resolucao-e-politica-de-caminhos.md); [Contrato do Arquivo de Projeto v1](../../../docs/design/0013-contrato-do-arquivo-de-projeto-v1.md); [contrato público de persistência](../../../docs/design/0015-contrato-publico-de-persistencia-do-project-core.md); [prova de Salvamento e trava](../../fase-2-fluxo-persistente/issues/04-provar-o-salvamento-atomico-com-trava.md).

- [ ] `Novo Projeto` abre exatamente as etapas `Dimensões` e `Personalização`; neste primeiro corte elas apresentam os valores neutros válidos e permitem avançar, voltar e cancelar sem criar estado persistente.
- [ ] Os valores padrão são 60 × 30 cm, 300 DPI, duas Lâminas duplas, Sangria e segurança equivalentes a 3 mm, Background branco e ausência de Overlay, borda, Frames e mídias.
- [ ] Nome e Localização não aparecem como campos do fluxo; somente `Criar` abre o diálogo nativo do Windows, filtrado para `.myalbuns`.
- [ ] Cancelar o fluxo ou o diálogo retorna ao estado apropriado com os valores preservados e não cria arquivo, Identidade, Host, tentativa no núcleo ou entrada em Projetos recentes.
- [ ] O diálogo entrega a autorização imutável `CreateOnly` quando o destino estava livre ou `ReplaceConfirmed` quando a substituição foi confirmada pelo Windows; o núcleo nunca reinfere consentimento.
- [ ] O processo global envia ao Host uma única requisição `CreateNew` correlacionada com valores autoritativos, pathname nativo, `RootBindingPlan` e autorização congelada; somente o Host chama a criação pública do `ProjectCore`.
- [ ] A criação valida o diretório pai por handle, deriva um único filho seguro, publica o documento, confirma tipo e contenção física, adquire os bloqueios e só então devolve uma Sessão editável.
- [ ] O filho rejeita nova raiz, `.`, `..`, namespace, fluxo alternativo, curingas, nomes reservados e escape por reparse point, sem usar validação textual como prova física.
- [ ] Um objeto concorrente sob `CreateOnly` produz `DestinationConflict`; `ReplaceConfirmed` nunca substitui um Projeto protegido e nesse caso produz `ProjectInUse`.
- [ ] Sucesso cria a Identidade no núcleo, publica Revisão `0`, inicia o Host pelo bootstrap aprovado, mostra o Projeto neutro no editor e só depois inclui o caminho em Projetos recentes.
- [ ] Falha antes, durante ou depois da Publicação distingue resultado conclusivo de `CreateStateIndeterminate`, não anuncia falso sucesso e não deixa um Projeto final parcial conhecido como válido.
- [ ] Testes públicos cobrem criação, substituição confirmada, corrida no destino, cancelamentos, `ProjectInUse`, caminho seguro e abertura do documento recém-publicado no Host independente.
- [ ] O corte não introduz `Salvar como`, migração de `MyAlbuns2`, protocolo IPC genérico ou campos futuros no documento.
