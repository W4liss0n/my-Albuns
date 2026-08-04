# 10 — Exportar a Lâmina visível com personalização e estado não salvo

**What to build:** exportar exatamente a Lâmina visível, incluindo Background e Overlay configurados e o DPI corrente ainda não salvo, usando os Arquivos originais sem modificar o Projeto.

**Blocked by:** 05 — Configurar a Personalização inicial; 06 — Alterar DPI com Undo e Redo; 09 — Exportar uma Lâmina neutra como JPEG real.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [ticket pai](../../programa-diagramacao/issues/08-esqueleto-ponta-a-ponta.md); [criação de Projeto](../../../docs/design/0003-criacao-de-projeto.md); [Exportação normal](../../../docs/design/0004-exportacao-normal.md); [propriedade de estado e módulos do núcleo](../../../docs/design/0012-propriedade-de-estado-e-modulos-do-nucleo.md); [Contrato do Arquivo de Projeto v1](../../../docs/design/0013-contrato-do-arquivo-de-projeto-v1.md); [Contrato JPEG do primeiro fluxo](../../../docs/design/0014-contrato-jpeg-do-primeiro-fluxo.md).

- [ ] `Exportar Lâmina` usa a Lâmina visível que originou o comando, inclusive quando ela não é a primeira, e congela sua Identidade e o DPI na mesma Revisão visível.
- [ ] Alterar DPI sem salvar produz dimensões e metadados correspondentes ao valor na interface, enquanto os bytes do Arquivo de Projeto permanecem inalterados.
- [ ] Background e Overlay derivam do mesmo `CompositionPlan` consumido pelo Canvas, preservando geometria, escopo por lado, ordem e alfa; Frames demonstrativos da criação nunca aparecem na saída.
- [ ] A tentativa abre somente os Arquivos originais referenciados pela unidade; Cache e representações reduzidas nunca são fonte ou fallback.
- [ ] O conjunto entregue ao Processador corresponde exatamente às mídias referenciadas, sem fonte ausente, extra ou duplicada.
- [ ] Preflight valida formato, dimensões, variante, profundidade, modelo de cor, perfil e orientação antes da normalização; formatos e perfis recusados produzem códigos estáveis e tipados.
- [ ] Orientação EXIF aceita é aplicada uma única vez; Background com alfa compõe sobre branco, Overlay conserva alfa até sua composição e o JPEG final é opaco.
- [ ] Fontes sem perfil ou com perfil sRGB permitido são normalizadas para a saída controlada; perfil malformado ou não permitido é recusado antes da Publicação.
- [ ] Guardrails separam pixels da saída e soma das fontes únicas, usam aritmética verificada e alocações falíveis.
- [ ] O Processador emite exatamente um terminal correlacionado; terminal ausente, duplicado, malformado ou de outra tentativa é falha de transporte ou protocolo.
- [ ] Sucesso, falha ou cancelamento não cria Revisão, não salva, não altera Undo/Redo e não limpa mudanças pendentes.
- [ ] Um teste ponta a ponta usa Lâminas distinguíveis, seleciona uma não inicial, altera DPI sem salvar e comprova pelo JPEG que alvo, personalização e Revisão visível foram usados.
- [ ] Casos dourados cobrem escopos esquerdo, direito e `Ambos os lados`, transparência, orientação, perfis aceitos e falhas de fonte antes da Publicação.
