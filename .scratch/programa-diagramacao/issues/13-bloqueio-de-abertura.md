# 13 — Bloqueio de abertura

**What to build:** impedir edições concorrentes acidentais do mesmo Projeto, conduzindo o usuário à sessão já aberta e recuperando com segurança bloqueios deixados por encerramentos inesperados.

**Blocked by:** 12 — Movimentação e Cópia externa.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [ADR 0002 — identificar Cópias externas](../../../docs/adr/0002-identificar-copias-externas.md); [ADR 0007 — caminhos Windows e identidade física](../../../docs/adr/0007-tratar-caminhos-windows-e-identidade-fisica.md); [resolução e política de caminhos](../../../docs/design/0011-resolucao-e-politica-de-caminhos.md); [propriedade de estado e módulos do núcleo](../../../docs/design/0012-propriedade-de-estado-e-modulos-do-nucleo.md).

- [ ] Abrir novamente um Projeto ativo reutiliza ou focaliza a sessão existente em vez de criar outro editor concorrente.
- [ ] Reconhecer como a mesma sessão um arquivo aberto por unidade mapeada, UNC ou outra representação fisicamente equivalente, sem depender de comparação textual ou caixa das letras.
- [ ] Ativações posteriores do executável e aberturas pelo Explorador são encaminhadas à instância única do `MyAlbuns.exe`, que resolve cada caminho pela Identidade antes de focalizar ou iniciar uma Janela.
- [ ] Uma única ativação pode conter vários arquivos: Identidades diferentes abrem em Janelas separadas e duplicatas focalizam a sessão existente.
- [ ] O Bloqueio de abertura é associado à identidade e ao arquivo corretos sem bloquear Cópias externas independentes.
- [ ] Usar `Same`, `Different` e `Indeterminate` na comparação física; manter o bloqueio real do arquivo como proteção final e recusar uma segunda sessão editável quando a comparação for inconclusiva.
- [ ] Um bloqueio ativo de outro processo produz uma mensagem clara e não abre o Projeto para edição.
- [ ] Um bloqueio órfão é distinguido de um processo vivo e pode ser recuperado pelo fluxo definido no ticket 02.
- [ ] Encerramento normal libera o bloqueio; falha durante a liberação não corrompe o documento.
- [ ] Testes exercitam abertura repetida, alias UNC/unidade mapeada, identidade inconclusiva, duas instâncias, encerramento forçado, bloqueio órfão e Projetos copiados.
