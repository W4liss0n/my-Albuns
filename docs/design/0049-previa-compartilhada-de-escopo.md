---
status: accepted
document: design
date: 2026-09-22
implementation-readiness: ready-for-agent
---

# Prévia compartilhada de seleção de páginas

O autor pediu um único componente para a prévia dos padrões gerais e a prévia
da lâmina real. Esta decisão substitui as composições separadas e as diferenças
de interação preservadas anteriormente no design 0041.

`VisualScopePreview` concentra a superfície, proporção, seleção, hover, foco,
áreas clicáveis e disponibilidade das páginas. Novo projeto, Design do álbum e
Design da lâmina usam esse mesmo componente. O conteúdo possui duas variantes:

- `general`: mostra fundos, sobreposições e quadros de exemplo dos padrões.
  A criação pode incluir as guias técnicas.
- `sheet`: mostra a composição real recebida do domínio, com fotos, recortes,
  efeitos e ajustes locais. A página inativa permanece neutra e sem interação.

`PersonalizationPreview` e `SheetPreview` continuam responsáveis somente pelo
desenho desses conteúdos. O primeiro recebe a intensidade visual dos exemplos;
não calcula seleção, hover ou foco. Foram removidos `PersonalizationScopeSurface`,
`SheetScopePreview` e o controle parcial `VisualScopeControls`.

## Interação e apresentação comuns

Nos painéis, a faixa central de Ambos os lados ocupa 20% da largura. Cada lado
possui 40% da área clicável, sem sobreposição. O realce e o foco representam a
página inteira, independentemente da largura do alvo. Na criação, Ambos os lados
continua disponível fora da lâmina; dentro dela, os alvos são as duas metades.
Essa configuração representa o fluxo de criação, sem criar outro seletor.

O hover conserva o tom neutro quente com opacidade de 12%, sem contorno ou
sombra. Seleção usa a indicação azul interna já existente no programa; o lado
não selecionado é suavizado. Foco de teclado usa a indicação pontilhada espacial
da prévia de criação nas duas variantes, separado do hover e da seleção. O foco
externo de Ambos os lados na criação também usa a mesma apresentação.

A superfície usa borda discreta e sombra de miniatura compartilhadas. A largura
disponível continua pertencendo ao painel ou à área de criação, e a proporção
física é preservada. Nenhum estado acrescenta texto ou desloca controles.

Hover e foco não alteram o escopo confirmado. Clique, Enter e Espaço comunicam a
escolha ao consumidor. A seleção continua transitória e controlada pelo fluxo;
não cria comando do domínio, histórico ou gravação. Em página única, apenas o
lado ativo é selecionável. Trocar a lâmina reinicia os estados transitórios da
prévia, preservando o escopo mantido pelo contexto.

## Verificação

Os testes dos três fluxos verificam os consumidores reais. A cobertura comum
exercita as duas variantes, independência entre hover, foco e seleção, seleção
por teclado, atualização controlada, páginas únicas de ambos os lados e o foco
externo de Ambos os lados.

A aceitação visual cobre os cenários afetados de criação e dos dois painéis,
incluindo conteúdos decorativos, página única, foco, hover e escalas ampliadas.
As evidências ficam fora do controle de versão.

## Contrato externo

React instalado: 19.2.8. A consulta ao Context7 ficou indisponível por cota.
A documentação oficial de [composição por propriedades](https://react.dev/learn/passing-props-to-a-component),
[estado controlado](https://react.dev/learn/sharing-state-between-components) e
[eventos de foco](https://react.dev/reference/react-dom/components/common#onfocus)
fundamenta a composição. A mudança conserva as APIs já usadas nesta versão,
sem dependências novas.
