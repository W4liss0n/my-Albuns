# 05 — Configurar a Personalização inicial

**What to build:** completar `Personalização` com uma reprodução viva e permitir que Background, Overlay e padrão dos Frames confirmados se tornem os padrões visuais do Projeto criado.

**Blocked by:** 04 — Configurar e validar as Dimensões.

**Type:** implementation

**Status:** resolved

**Normative sources:** [ticket pai](../../programa-diagramacao/issues/08-esqueleto-ponta-a-ponta.md); [criação de Projeto](../../../docs/design/0003-criacao-de-projeto.md); [Contrato do Arquivo de Projeto v1](../../../docs/design/0013-contrato-do-arquivo-de-projeto-v1.md); [Contrato JPEG do primeiro fluxo](../../../docs/design/0014-contrato-jpeg-do-primeiro-fluxo.md).

- [x] A etapa mantém uma reprodução ampla à esquerda, controles de Background, Overlay e padrão dos Frames à direita e `Voltar`, `Cancelar` e `Criar` em rodapé fixo; rolagem dos controles não retira reprodução ou rodapé da área visível.
- [x] A reprodução usa a proporção física escolhida, mas mostra sempre uma Lâmina dupla com Frames demonstrativos, independentemente das extremidades configuradas.
- [x] Background, Overlay e presença, cor e espessura da Borda atualizam a reprodução imediatamente; a Lâmina e os Frames demonstrativos nunca são copiados para o Projeto.
- [x] O estado inicial é Background branco opaco, Overlay ausente e Borda ausente; cores são `#RRGGBB` canônicas e uma Borda sólida possui espessura positiva em micrômetros.
- [x] Hover realça temporariamente esquerda, direita ou ambos pela região central; clicar fixa o escopo e somente o escopo fixado dirige os controles de Background e Overlay.
- [x] `Escolher imagem...` usa o seletor nativo e transporta o pathname reversível; a mídia provisória pode aparecer na reprodução sem ser copiada, importada ou vinculada antes da criação bem-sucedida.
- [x] Voltar conserva personalizações; cancelar o seletor, o fluxo ou o diálogo final não modifica os arquivos originais, não cria mídia no documento e não deixa cópia provisória.
- [x] A criação confirmada inclui somente os Decorativos efetivamente referenciados, preserva o escopo `bothSides` ou `perSide` e grava os padrões visuais no DTO v1.
- [x] Referências confirmadas possuem IDs únicos, preservam a ordem e não duplicam o mesmo pathname; todas as Lâminas criadas herdam esses padrões ainda sem Frames reais.
- [x] O editor e o plano canônico de composição mostram os padrões persistidos; a reprodução de criação permanece apenas estado transitório da interface.
- [x] Testes cobrem hover versus seleção fixada, atualização imediata, ambos os escopos, imagens distintas por lado, Background branco ou imagem, Overlay ausente ou imagem, Borda, cancelamentos e reabertura do Projeto personalizado.
- [x] Personalizações locais por Lâmina, transformações de Decorativos, Frames reais, Fotos e Cache de preview permanecem fora deste ticket.

## Comments

- O fluxo fecha a cadeia `token provisório → pathname nativo reversível → Core → persistência v1 → projeção → composição`, mantendo a reprodução da criação transitória e os padrões confirmados persistentes.
- Imagens provisórias e vinculadas possuem registros e ciclos de vida separados. Ambas usam um transporte opaco comum, restrito à janela correta, sem copiar arquivos, expor pathnames ou criar Cache.
- O editor lê os originais vinculados por identidade física validada. Preparações concorrentes conservam URLs estáveis, recapturam arquivos substituídos e preservam falhas tipadas até o log.
- A exportação de personalizações com mídia vinculada continua deliberadamente no Ticket 10; este ticket não antecipa essa política.
- Evidências finais: `npm run build`; 24 arquivos e 153 testes de interface; `npm run test:rust`, incluindo 128 testes do desktop, 2 testes de processo Host real e 17 testes do Imaging; `npm run quality:rust`; `git diff --check`.
- As revisões independentes finais de especificação, estrutura e simplicidade não encontraram outro problema material que justificasse ampliar a refatoração.
