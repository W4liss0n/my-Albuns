---
status: accepted
document: design
date: 2026-09-09
ticket: 26
---

# Layout travado e schema v9

Este documento registra a implementação do travamento já definido na
[SPEC](../specs/programa-de-diagramacao-de-albuns.md#layouts), sobre o
[contrato de aplicação](0026-contrato-do-gerador-e-da-aplicacao-de-layouts.md).

## Aplicar, travar e destravar

O cadeado de cada preview confirma a mesma consulta preparada pelo Core:
aplica a geometria e trava a Lâmina em uma única ação de Histórico. A preview
atual fica destacada, com cadeado fechado, e as demais ficam desabilitadas.
Destravar altera somente esse estado, sem diálogo. Os dois comandos têm
Undo/Redo e passam pela fila compartilhada de mutações, inclusive quando
Salvar ou Desfazer chegam enquanto uma confirmação está pendente.

Por padrão, as sugestões usam a quantidade e as orientações dos Frames
existentes. O controle `Posições` permite pedir uma quantidade maior, até o
limite de 30 posições do Gerador. `Novas posições` informa explicitamente a
orientação das posições adicionais. Elas só podem ser confirmadas pelo
cadeado; clicar no corpo de uma preview não cria estruturas adicionais.
Os IDs dos placeholders são reservados na consulta e reutilizados pela prévia
e pelo comando. Seu estilo é herdado do Álbum. Nenhuma Foto, estilo ou ajuste
dos Frames existentes é descartado.

## Estrutura e conteúdo

O Core impede mudar quantidade, identidade, posição ou dimensões dos Frames
de uma Lâmina travada. Adicionar, colar, mover, redimensionar, aplicar outro
Layout, Trocar lados e soltar uma Foto em área vazia ficam indisponíveis.
Seleção, Pilha visual, Borda, Opacidade e os ajustes da Foto continuam editáveis.
Apagar remove somente as Fotos; placeholders vazios não criam Histórico.
Duplo clique preenche o próximo placeholder, e arraste sobre Frame preenchido
substitui sua Foto. Sem placeholder, o duplo clique orienta a substituição
explícita por arraste.

Enquanto travada, a preview destacada acompanha a Pilha visual corrente.
O Último Layout aplicado mantém sua definição original. Destravar preserva
ambos os estados. Converter uma extremidade muda a superfície ativa,
destrava e reorganiza em uma única ação reversível, conforme a SPEC.

## Persistência e Exportação

O escritor passa a emitir `schemaVersion: 9`. Cada Lâmina exige o booleano
`layoutLocked`; o campo ausente ou de tipo inválido é rejeitado. Um travamento
persistido exige Frames e Último Layout com a mesma quantidade de posições.
Os demais campos permanecem no contrato da v8. Leitores históricos continuam
fechados: v1 a v8 abrem destravados, sem reescrever o original. Somente Salvar
publica a migração; uma correção de identidade preserva o schema original.

A validação do snapshot congelado recebe a seleção de Lâminas e identifica
cada placeholder por Lâmina e posição, inclusive os criados manualmente e os
preservados após destravar. Uma Lâmina sem Frames continua exportável.
O Host executa essa validação
antes de abrir o destino e o Core a repete ao preparar a saída. O diálogo
compartilhado apresenta `Projeto`, `Motivo` e `Abrir Projeto`; a ação devolve
o foco à Janela do Projeto. Preencher as posições libera a mesma seleção.
A composição física usada pelo Canvas permanece a fonte da Exportação JPEG.

## Verificação

Os testes públicos exercitam bloqueios estruturais, conteúdo permitido,
prévia exata, placeholders herdados, conversões, Histórico, migração e
reabertura. O teste nativo de exportação trava uma composição com posição
adicional, comprova o bloqueio, preenche, salva, reabre e publica pelo
processador JPEG real. O corpus visual é produzido pelo Core e reproduzido
pelos cenários `workspace-layout-*` e `layout-export-problems` do manifesto.
