---
status: current
document: research
date: 2026-09-07
platform: windows
ticket: 20
---

# Movimento e redimensionamento de um Frame

Este é o primeiro recorte da [issue #20](https://github.com/W4liss0n/my-Albuns/issues/20):
selecionar, mover e redimensionar um único Frame no Modo de edição da Lâmina.
A entrega continua a importação e a composição com Foto já integradas em `main`.
Seleção múltipla, grupos, organização da Pilha visual, troca de conteúdo e
cópia/colagem continuam no escopo restante da issue.

## Comportamento

Entre na edição da Lâmina com `Enter` no Canvas ou com clique duplo na Lâmina.
O enquadramento considera largura e altura disponíveis e centraliza a Lâmina,
mantendo suas bordas e alças acessíveis também em um Canvas mais estreito.
Arraste o corpo de um Frame para movê-lo. As oito alças redimensionam a seleção:
as laterais afetam um eixo; os cantos afetam os dois. `Shift` nos cantos mantém
a proporção; `Alt` mantém o centro; ambos respondem durante o gesto.

O movimento começa depois do limiar de arraste do Windows, convertido da escala
do monitor para as coordenadas do ponteiro. Antes disso, soltar executa somente
a seleção. `Esc` cancela o gesto em curso; uma nova pressão sai da edição.
Perda de foco ou de captura, troca de Projeto, alteração concorrente do Frame
e operações que bloqueiam a edição descartam a prévia ainda não confirmada.

O ProjectCore calcula a geometria dentro da Lâmina dupla ou da Página ativa.
A interface conserva somente o estado transitório do ponteiro e pinta a
composição devolvida pelo núcleo. Há no máximo uma consulta de prévia pendente;
movimentos seguintes substituem a consulta aguardando envio. A resposta de um
gesto cancelado nunca volta a aparecer.

Soltar enfileira imediatamente uma intenção com o retângulo inicial e o
deslocamento final. O retângulo inicial protege contra uma alteração concorrente
da geometria, enquanto outras alterações do Projeto são preservadas. Uma edição
confirmada cria uma ação de Undo/Redo; um resultado idêntico ao inicial não altera
Revisão, Histórico ou estado de Salvamento. Salvar e Desfazer usam a fila comum
do Projeto, inclusive quando a confirmação do gesto ainda está pendente.

O Frame conserva sua Foto, ajustes e posição na Pilha visual. A composição
recalcula o Preenchimento, preservando Pan e Zoom do usuário. A geometria já
pertence ao esquema v3; a entrega não exige nova migração do arquivo.

## Calibração do mínimo

A primeira captura em `1600 × 900`, com Canvas de `1287 × 624`, mostrou que
`5 mm` aproximava demais as oito alças e encobria quase toda a Foto. O mínimo
foi ajustado para `12 mm`: na mesma captura, o Frame ocupa aproximadamente
`22 × 22 px`, com alças distintas e conteúdo central visível. O valor está
registrado no design 0001, proprietário dessa política.

Esse mínimo é físico, independente de DPI de exportação. Frames que já tenham
um eixo menor conservam a possibilidade de manter ou aumentar esse eixo. A
calibração não equivale a verificar todas as combinações de monitor, tamanho de
Lâmina e ampliação; esses contextos continuam sujeitos à validação manual da
interface nativa.

## Fronteiras de verificação

Os testes públicos do ProjectCore cobrem as oito âncoras, modificadores,
limites da superfície e do tamanho, deltas extremos, prévia sem mutação,
rejeição de geometria obsoleta, preservação de alterações adjacentes, Undo/Redo,
Salvamento e reabertura. O corpus `tests/fixtures/frame-geometry-cases.json`
é conferido contra essas prévias e alimenta os cinco estados renderizados do
Canvas: seleção, movimento, redimensionamento, proporção com centro e mínimo.

Os testes do Canvas exercitam a captura do ponteiro, cancelamento, respostas
atrasadas, a posição final da soltura e a retomada dos cliques após cancelamento
por mudança de Projeto, modo, bloqueio ou geometria confirmada. O clique residual
do arraste continua suprimido somente durante o término do gesto. O controlador
usa a fila de mutações
real para verificar edição seguida de Salvamento, tanto no sucesso quanto na
falha. O teste do Host com o Processador real redimensiona, desfaz, refaz, salva,
reabre e publica JPEG; confere a geometria reaberta, os pixels dentro e fora do
novo Frame e a integridade do Original.

A validação geral usa `npm run validate`, sem janelas visíveis. As capturas
usam os cenários declarados no manifesto de UI; registram aparência dos
resultados, enquanto os testes de interação verificam o gesto. Elas não
representam uma execução com ponteiro no WebView nativo. A suspensão dos testes
com janelas permanece conforme a política de validação.

## Continuidade da imagem e do cursor

O teste manual identificou duas piscadas: o Frame voltava brevemente à geometria
inicial ao soltar, e o cursor alternava entre a ação e o padrão durante o gesto.
A reprodução no navegador com Pixi real registrou, no redimensionamento, larguras
`289,166 → 240 → 289,166` unidades do Canvas após a soltura. Durante o arraste,
registrou `auto → ew-resize → auto → ew-resize`; o movimento alternava entre
`auto` e `move`.

A confirmação do comando encerrava a prévia antes da próxima apresentação da
projeção pelo React. Agora a porta da interação devolve o `ComposedFrame` confirmado
pelo Core, e o Canvas o conserva até receber a projeção correspondente. Isso
também cobre uma soltura diferente da última prévia, falha e resultado sem
alteração. A fila de mutações continua sendo a mesma de Salvar e Desfazer.
O fim dessa ponte também observa a identidade da composição apresentada: se mover
e desfazer forem apresentados juntos pelo React, o novo snapshot encerra a prévia.
O Core restaura tanto a geometria quanto o número da revisão no Desfazer;
comparar somente esses valores não distingue a nova apresentação. A regressão usa o
controlador, a fila e o estado React reais, com respostas determinísticas da porta.

O cursor de hover do Pixi depende do objeto atingido em cada evento. Uma alça
pode ficar para trás enquanto a prévia assíncrona acompanha o ponteiro. Durante
a captura, o gesto conserva o cursor do alvo inicial e o libera ao terminar ou
cancelar. A regra CSS temporária prevalece sobre o cursor inline de hover;
nenhum cursor direcional é duplicado fora do proprietário das alças.

Essas decisões foram conferidas contra a documentação de
[eventos do Pixi 8](https://pixijs.com/8.x/guides/components/events), o código
instalado do Pixi 8.19.0, as etapas de
[renderização e confirmação do React 19.2](https://react.dev/learn/render-and-commit)
e a [precedência de estilos CSS](https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Values/important).

O comando `npm test -- src/components/AlbumCanvas.frameGeometry.test.tsx -t "does not flash"`
falhou antes da correção: esperava posição `40,20`, mas recebeu `0,0`. O teste
permanente `npm run test:frame-gestures` exercita movimento e a alça direita com
ponteiro real no navegador headless, amostra o cursor calculado e a geometria
entre os quadros e verifica a liberação do cursor. Exige a confirmação do comando,
a apresentação da projeção e um quadro posterior a ela. Conserva capturas, logs,
proveniência e resultado em `.scratch/frame-gesture-evidence/` ou no diretório
fornecido como argumento. A fixture usa a cena produtiva e uma porta com atraso
determinístico; ela verifica apresentação, sem substituir os testes da política
geométrica do Core ou a validação manual no WebView nativo.
