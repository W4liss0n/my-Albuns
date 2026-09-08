---
status: current
document: research
date: 2026-09-07
platform: windows
ticket: 20
---

# Seleção múltipla e geometria do grupo

Este recorte continua a edição de Frame único integrada pela
[PR #66](https://github.com/W4liss0n/my-Albuns/pull/66). Implementa seleção,
movimento e redimensionamento conjuntos no Modo de edição da Lâmina, conforme
o [design 0001](../design/0001-estrutura-da-janela-do-projeto.md).

## Comportamento

Um clique seleciona somente o Frame atingido. `Ctrl` + clique adiciona ou remove
esse Frame, inclusive em cliques rápidos consecutivos. Quando há sobreposição,
responde somente o Frame superior. Clicar em área vazia limpa a seleção.
Não há seleção por retângulo nem alternância entre Frames encobertos.

Cada Frame selecionado mantém seu contorno. O grupo tem uma caixa única com oito
alças. Arrastar um membro selecionado move todos; arrastar um Frame fora da seleção
seleciona e move somente ele, depois do limiar de arraste da plataforma. Antes do
limiar, soltar conserva as regras de clique.

Redimensionar escala as posições e dimensões dos membros dentro da caixa.
As alças laterais alteram somente um eixo. `Shift` nos cantos conserva a proporção,
`Alt` conserva o centro e ambos respondem durante o gesto. O menor membro limita
o grupo inteiro: nenhum eixo cai abaixo de `12 mm`, ou de seu tamanho inicial
quando já era menor. O grupo para junto ao encontrar o limite da Lâmina dupla ou
da Página ativa. Não há limitação individual que deforme o arranjo.

A seleção é transitória. Não modifica a Revisão e não é salva ou desfeita. Sair
da edição a limpa. Undo/Redo removem referências a Frames que deixaram de existir;
restaurar um Frame não o seleciona novamente. O Painel contextual mostra as
quantidades de Frames, Fotos e placeholders, sem controles individuais de Foto.

## Responsabilidades e persistência

O comando de geometria recebe os IDs e retângulos iniciais de todos os membros.
O Core valida a seleção inteira antes de aplicar qualquer alteração: os Frames
devem existir, ser distintos, pertencer à mesma Lâmina e conservar a geometria
inicial. Uma falha em qualquer membro rejeita a operação inteira.

O Core calcula a caixa e a transformação. As bordas dos membros são arredondadas
em micrômetros, mantendo bordas compartilhadas coincidentes. A interface pinta
as composições devolvidas e conserva somente o gesto e a seleção transitórios.
Prévia e confirmação usam o mesmo cálculo. Uma confirmação cria uma ação de
Histórico; resultado idêntico não cria revisão. Foto, ajustes e Pilha visual
permanecem associados aos Frames. O arquivo continua no esquema v3.

## Verificação

Os testes públicos do ProjectCore cobrem as oito alças, posições relativas,
membros de tamanhos diferentes, mínimo, Frames legados menores, superfícies
ativas, rejeição atômica, ausência de alteração, Undo/Redo, Salvamento, reabertura
e composição congelada. Os testes do Canvas e do controlador cobrem seleção,
modificadores, resposta atrasada, cancelamento de todo o grupo e Desfazer
enfileirado imediatamente após soltar, inclusive quando o React apresenta os
resultados juntos.

`npm run test:frame-gestures` executa quatro gestos com Pixi e ponteiro reais no
navegador headless: movimento e redimensionamento de um Frame e de um grupo.
Nos grupos, verifica cliques, Ctrl+cliques consecutivos, sobreposição, área vazia,
preservação da seleção, movimento de cada membro e continuidade da imagem e do
cursor. Esse teste encontrou o segundo Ctrl+clique rápido sendo consumido como
tentativa de entrar novamente na edição. O tratamento de duplo clique agora se
restringe ao Modo normal. A regressão falhou antes da correção e passou depois.

O corpus `tests/fixtures/frame-group-geometry-cases.json` é produzido e conferido
pelo Core. Ele alimenta cinco cenários de aparência no manifesto de UI: seleção,
movimento, redimensionamento, proporção com centro e mínimo. Outro cenário usa o
Workspace com o painel de seleção conjunta. As capturas são resultados estáticos;
o teste de ponteiro verifica os gestos. A validação geral usa `npm run validate`.
Os testes automatizados com janelas nativas continuam suspensos.

A inspeção do grupo no mínimo identificou a indicação `Adicionar Foto`
ultrapassando um placeholder de `12 mm`. O placeholder agora mostra a indicação
somente quando o texto cabe em suas dimensões na escala atual do Canvas. O tamanho
da fonte e o contorno são preservados; aumentar o Frame torna a indicação visível
novamente. O teste público do Canvas reproduziu a falha antes dessa correção.

As decisões sobre eventos foram conferidas com o
[Pixi 8](https://pixijs.com/8.x/guides/components/events), instalado na versão
8.19.0. As ações de teclado e ponteiro seguem o
[WebDriver, versão de 2 de julho de 2026](https://www.w3.org/TR/2026/WD-webdriver2-20260702/#actions).
A seleção usa atualizações imutáveis conforme o
[Zustand 5](https://zustand.docs.pmnd.rs/guides/immutable-state-and-merging),
instalado na versão 5.0.14.

## Escopo restante

A issue #20 permanece aberta. Organização da Pilha visual, troca de conteúdo,
cópia/colagem e outros comandos coletivos ainda pertencem às próximas etapas.
Edição coletiva de estilos e ajustes de Foto também depende das capacidades
correspondentes do Core. O Travamento de Layout ainda não é um estado persistente
exposto pela projeção; este recorte preserva a restrição visual existente sem
declarar essa funcionalidade concluída.
