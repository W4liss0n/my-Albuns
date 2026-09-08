---
status: current
document: research
date: 2026-09-07
platform: windows
ticket: 20
---

# Ordenação da Pilha visual de Frames

Este recorte continua a seleção múltipla integrada pela
[PR #67](https://github.com/W4liss0n/my-Albuns/pull/67) e implementa os quatro
comandos de ordenação do [design 0001](../design/0001-estrutura-da-janela-do-projeto.md).

## Comportamento

No Modo de edição, `Editar > Organizar` e o menu contextual do Frame permitem
trazer para frente, avançar uma posição, recuar uma posição e enviar para trás.
`Ctrl+]` avança e `Ctrl+[` recua. Os atalhos respeitam campos de entrada, menus,
diálogos e operações que bloqueiam a interação. Sem seleção, os comandos do menu
Editar ficam desabilitados.

O clique direito em um membro selecionado preserva o grupo. Em outro Frame,
seleciona somente o alvo atingido. Na sobreposição, responde o Frame superior.
O menu contextual compartilha com o menu da Lâmina o limite da janela, navegação
por setas, foco inicial e fechamento por Escape ou clique externo.

Avançar e recuar movem cada bloco contíguo selecionado através de um único
vizinho não selecionado. Um bloco que já está no limite não impede os demais.
Trazer para frente e enviar para trás reúnem a seleção no extremo correspondente.
As quatro operações preservam a ordem relativa dos selecionados e dos não
selecionados, independentemente da ordem dos cliques de seleção.

Fotos e placeholders participam igualmente. A ordenação mantém a geometria e
os ajustes de Foto. A seleção permanece transitória e não muda com o comando.
Cada resultado diferente gera uma única ação de Undo/Redo. Um comando sem efeito
preserva a Revisão e o ramo de Redo.

## Responsabilidades e persistência

O comando recebe IDs e direção. O Core valida que a seleção é não vazia, sem
duplicatas e da mesma Lâmina antes de modificar o documento. O cálculo usa a
pilha atual quando a fila executa o comando; a interface captura apenas os IDs
selecionados e a ação desejada.

A ordem da lista persistente de Frames é a autoridade da pilha. A projeção
deriva `zIndex` dessa lista e a composição usa a mesma ordem para o Canvas,
miniaturas, resolução do alvo de Foto e composição congelada de Exportação.
Salvar e reabrir conservam o resultado, sem mudança do esquema v3.

## Verificação

Os testes públicos do Core cobrem blocos separados e contíguos, extremos,
rejeição atômica, placeholders, Undo/Redo, ausência de efeito, alvo superior,
Salvamento, reabertura e composição congelada. Os testes do Workspace, Canvas
e controlador cobrem menus, atalhos, seleção do clique direito e Save/Undo
disparados enquanto a ordenação está pendente, em sucesso e falha.

O corpus `tests/fixtures/frame-stack-cases.json` é produzido e conferido pelo
Core. A prévia do Workspace reproduz esses resultados através dos componentes
de produção. O manifesto de UI exercita os dois menus, as quatro operações,
Undo e cliques reais no Canvas após trocar o Frame superior. O menu contextual
da Lâmina permanece na verificação visual por compartilhar a superfície.
Capturas e julgamentos visuais ficam nos artefatos externos da execução.

O ponteiro real encontrou duas falhas cobertas por regressões: clicar em
Organizar depois de o hover abri-lo fechava o submenu; e o evento `contextmenu`
emitido após a abertura pelo Pixi fechava o menu recém-criado. O clique agora
mantém o submenu aberto, e a superfície contextual distingue o evento de
abertura de um novo clique externo.

Os eventos de clique direito foram conferidos na documentação do
[Pixi 8](https://pixijs.com/8.x/guides/components/events), versão instalada 8.19.0,
e nos tipos de eventos da dependência. A validação geral usa `npm run validate`.
Os testes automatizados com janelas nativas continuam suspensos.

## Escopo restante

A issue #20 permanece aberta para inclusão manual de placeholders, exclusão,
troca de conteúdo, cópia/colagem e outros comandos. Travamento de Layout ainda
não é exposto como estado persistente pelo Core; a ordenação não depende desse
bloqueio geométrico e este recorte não acrescenta o travamento.
