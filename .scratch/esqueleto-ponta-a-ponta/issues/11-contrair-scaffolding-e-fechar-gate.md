# 11 — Contrair o scaffolding e fechar o gate ponta a ponta

**What to build:** deixar somente o fluxo produtivo aprovado — Boas-vindas, criação, DPI, Salvamento, fechamento, reabertura e JPEG — removendo os caminhos demonstrativos da Fase 1 e consolidando uma verificação reproduzível da jornada completa.

**Blocked by:** 08 — Controlar o fechamento com mudanças pendentes; 10 — Exportar a Lâmina visível com personalização e estado não salvo.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [ticket pai](../../programa-diagramacao/issues/08-esqueleto-ponta-a-ponta.md); [especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [propriedade de estado e módulos do núcleo](../../../docs/design/0012-propriedade-de-estado-e-modulos-do-nucleo.md); [Contrato do Arquivo de Projeto v1](../../../docs/design/0013-contrato-do-arquivo-de-projeto-v1.md); [Contrato JPEG do primeiro fluxo](../../../docs/design/0014-contrato-jpeg-do-primeiro-fluxo.md); [contrato público de persistência](../../../docs/design/0015-contrato-publico-de-persistencia-do-project-core.md); [protótipo público aprovado](../../fase-2-fluxo-persistente/issues/09-provar-a-fronteira-publica-do-fluxo.md).

- [ ] A execução normal inicia Boas-vindas sem construir Projeto demonstrativo, `ProjectSession`, Canvas, Cache de Projeto ou Processador no processo global.
- [ ] Cada Projeto criado ou aberto pertence a um Host independente com uma única `EditableProject` e continua utilizável se o processo global ficar indisponível.
- [ ] Schema demonstrativo, extensão histórica, construtores e compatibilidade da Fase 1 deixam os caminhos produtivos; os atalhos públicos de serialização e confirmação manual de Revisão são removidos.
- [ ] PNG demonstrativo, destino privado de prova, escolha fixa da primeira Lâmina, DPI fixo e nomes ligados ao Álbum demonstrativo deixam o produto.
- [ ] O protocolo de imagens transporta somente unidade JPEG, fontes exatas, caminhos nativos, bindings e correlação necessários; contratos gerados, capabilities e permissions sem consumidor são removidos.
- [ ] Testes que protegiam apenas o scaffolding são substituídos pela fronteira produtiva, sem manter duas suítes completas para modelos concorrentes; o protótipo continua apenas evidência e não é mesclado.
- [ ] A jornada automatizada cria por `CreateOnly`, altera DPI, faz Undo/Redo, recusa Revisão obsoleta, salva, comprova exclusividade, fecha e reabre em outro Host com Histórico vazio.
- [ ] A mesma jornada seleciona uma Lâmina não inicial, altera novamente o DPI sem salvar, exporta JPEG e comprova que o arquivo permaneceu na Revisão salva e que Histórico e mudanças pendentes não foram alterados pela Exportação.
- [ ] Cenários separados cancelam criação e destino de Exportação antes do núcleo, comprovando ausência de Host, Sessão, tentativa, temporário ou arquivo final.
- [ ] A verificação observa PIDs distintos para processo global, Host e Processador e exige exatamente um terminal correlacionado em cada bootstrap e tentativa de imagem.
- [ ] Frontend e teste ponta a ponta atravessam somente `ProjectCore` e `ExportPipeline`, nunca suas subdivisões internas.
- [ ] Build, contratos, frontend, Rust, qualidade estática e jornada multiprocesso possuem comandos documentados e reproduzíveis; a aplicação inicia em desenvolvimento sem fixtures ocultos.
- [ ] O namespace temporário `MyAlbuns2` permanece; o gate não introduz `Salvar como`, Recuperação, Fotos em Frames, PNG/PDF, lote, store genérico, registro genérico de codecs ou coordenador universal.
