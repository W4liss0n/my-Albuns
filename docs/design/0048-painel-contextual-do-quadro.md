---
status: accepted
document: design
date: 2026-09-22
implementation-readiness: ready-for-agent
---

# Simplificação do painel contextual do quadro

O usuário pediu a retirada de “Posição horizontal”, das instruções permanentes
de dois cliques e dos botões largos de Espelhar e Preto e branco. Esta decisão
refina a apresentação dos designs 0021–0024 e 0047; as operações de edição
continuam iguais.

O painel começa pelos ajustes que podem ser editados. Zoom, ângulo, opacidade
e borda deixam de mostrar frases de restauração abaixo dos sliders. Os gestos
de dois cliques permanecem disponíveis, e os limites e erros de digitação
continuam na validação por tooltip. A origem da borda e a ação para usar o
padrão do álbum continuam disponíveis porque informam a herança do design.

Espelhar horizontalmente e Preto e branco usam `PropertyToggle`, componente
neutro de UI com ícone e texto em um botão compacto, alinhado à esquerda e com
largura do conteúdo. O usuário rejeitou a primeira apresentação como uma linha
com checkbox separado; o estado passa a fazer parte do próprio botão.

O estado desligado usa superfície neutra e borda discreta. Após o usuário
rejeitar o preenchimento azul, o estado ligado passa a parecer pressionado:
fundo cinza quente, borda neutra, sombra interna suave e texto em grafite.
O ícone ganha um pouco de peso, de 1,4 px para 1,8 px, sem mudar suas dimensões.
A indicação permanece ao passar o mouse e ao receber foco por teclado.
Hover e foco no botão desligado mantêm a superfície clara, para não simular
ativação. O estado nunca muda a largura ou a posição dos controles.

Valores diferentes na seleção usam borda tracejada e superfície clara, sem
simular que a propriedade está ligada em todas as fotos. Os ícones vêm do
componente compartilhado do programa.

O componente reutiliza o botão compartilhado e os tokens de superfície,
borda, tipografia e espaçamento do programa. O botão tem altura mínima de
28 px, ícone de 14 px, espaçamento interno de 8 px e cantos de 4 px.
O foco por teclado recebe contorno neutro ao redor do botão, separado do estado
ligado. Enter e Espaço acionam a mesma operação do clique. Nome acessível,
estado misto e bloqueio durante operações seguem o contrato existente.

A composição contextual continua responsável por selecionar as fotos e
disparar os comandos. O controle compartilhado só apresenta o estado; não
duplica regras de edição, seleção, histórico ou persistência.

A validação usa os testes existentes de orientação, efeitos, sliders e
integração do painel, além das capturas declaradas do painel individual,
seleção mista, teclado e escalas de 125% e 150%.
