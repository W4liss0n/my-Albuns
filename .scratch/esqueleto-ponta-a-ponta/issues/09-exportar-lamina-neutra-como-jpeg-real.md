# 09 — Exportar uma Lâmina neutra como JPEG real

**What to build:** permitir que a pessoa exporte uma Lâmina neutra do Projeto padrão como JPEG real, nas dimensões físicas e no DPI do Projeto, sem depender do Canvas, Cache ou mídia artificial.

**Blocked by:** 03 — Criar um Projeto padrão no local escolhido.

**Type:** implementation

**Status:** resolved

**Normative sources:** [ticket pai](../../programa-diagramacao/issues/08-esqueleto-ponta-a-ponta.md); [Exportação normal](../../../docs/design/0004-exportacao-normal.md); [política de caminhos](../../../docs/design/0011-resolucao-e-politica-de-caminhos.md); [propriedade de estado e módulos do núcleo](../../../docs/design/0012-propriedade-de-estado-e-modulos-do-nucleo.md); [Contrato JPEG do primeiro fluxo](../../../docs/design/0014-contrato-jpeg-do-primeiro-fluxo.md); [ADR 0006](../../../docs/adr/0006-publicar-exportacao-com-transacao-limitada.md); [decisão JPEG](../../fase-2-fluxo-persistente/issues/07-decidir-o-contrato-jpeg-do-primeiro-fluxo.md).

- [x] `Exportar Lâmina` pede um pathname `.jpg` por diálogo nativo com nome sugerido conforme o Projeto e a posição; cancelar não cria tentativa, preparação, processo auxiliar, Histórico ou arquivo.
- [x] A autorização do diálogo permanece `CreateOnly` ou `ReplaceConfirmed` até a Publicação e nunca é reinferida ou transformada em renomeação silenciosa.
- [x] A tentativa congela um `RenderSnapshot`, reutiliza o `CompositionPlan` já contido nele e extrai uma única `ComposedOutputUnit` final e imutável.
- [x] O Processador recebe somente unidade composta, DPI, preparação, descritores exatos de fontes e `RootBindingPlan`; não recebe `.myalbuns`, Projeto, Álbum inteiro, outra Lâmina ou Cache.
- [x] Todo pathname autoritativo que atravessa frontend ou processos usa o DTO nativo reversível, sem string paralela ou normalização textual.
- [x] A Lâmina neutra aceita conjunto vazio de fontes e gera Background branco opaco sem mídia fictícia, thumbnail ou Cache.
- [x] Dimensões usam a aritmética inteira e o arredondamento determinístico do contrato; eixo zero, eixo acima de `65.535`, overflow ou saída acima do guardrail são recusados antes da alocação principal.
- [x] A saída é JPEG/JFIF baseline RGB de 8 bits, opaco, qualidade máxima, com DPI coerente, `sRGB2014.icc` e sem EXIF, XMP, comentário, thumbnail ou dados após o marcador final.
- [x] O Processador sincroniza e verifica a preparação; o Host confirma independentemente arquivo regular, tamanho e SHA-256 antes de publicar.
- [x] Preparação permanece dentro do Destino; falha ou cancelamento anterior à Publicação preserva o final existente, e conflitos ou terminais inconclusivos são resultados estruturados.
- [x] Toda saída terminal devolve `OperationLease`, pausa do Cache e reserva do Processador. Exportar não altera arquivo, Revisão salva, mudanças pendentes ou Histórico.
- [x] Um caso dourado com o Projeto neutro decodifica e verifica dimensões, DPI, perfil, opacidade, marcadores e conteúdo; os comandos exercitam o Processador real sem screenshot do Canvas nem hash comprimido permanente.

## Comments

- `Exportar Lâmina` usa a Lâmina corrente, abre um diálogo nativo antes de criar a tentativa e conserva a autorização `CreateOnly` ou `ReplaceConfirmed` dentro do plano validado até a publicação atômica. Cancelamento anterior à escolha não cria identificador, preparação ou processo auxiliar.
- O protocolo de imagens v14 recebe apenas a unidade composta congelada, DPI, preparação, fontes exatas e bindings nativos. Falhas conhecidas do renderizador são terminais estruturados e correlacionados; falhas de transporte continuam separadas.
- O JPEG canônico é baseline RGB opaco, qualidade 100, com JFIF/DPI e o perfil oficial `sRGB2014.icc`. O Processador sincroniza e verifica os marcadores; o Host revalida arquivo regular, tamanho e SHA-256 antes da publicação.
- `OperationLease` concentra concessão global, pausa do trabalho de Cache e reserva do Processador, com liberação por guarda em qualquer saída. O fluxo produtivo atual ainda não possui jobs de Cache; não foi criada uma fila artificial somente para satisfazer este ticket, e o lado de trabalho futuro deverá usar o mesmo gate já exercitado pelos testes concorrentes.
- O caso dourado real exporta a Lâmina neutra de 60 × 30 cm a 300 DPI como JPEG branco de 7.087 × 3.543 pixels e valida conteúdo, perfil, metadados e marcadores sem usar Canvas, screenshot, thumbnail ou Cache.
- Evidências finais: 188 testes de frontend; suítes Rust completas, incluindo 18 casos do Processador CLI; gate real de recuperação de imagens; `npm run quality:rust`; `npm run build`; contratos gerados e `git diff --check`.
- As revisões finais de padrões e especificação não deixaram achado material dentro do escopo neutro. Composição de mídias, orientação, perfis de fontes e personalização corrente permanecem concentrados no ticket 10, onde possuem critérios e casos dourados próprios.
