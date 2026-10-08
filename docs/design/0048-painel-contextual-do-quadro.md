---
status: accepted
document: design
date: 2026-09-22
updated: 2026-10-08
implementation-readiness: ready-for-agent
---

# Simplificação do painel contextual do quadro

O usuário pediu a retirada de “Posição horizontal”, das instruções permanentes
de dois cliques e dos botões largos de Espelhar e Preto e branco. Esta decisão
refina a apresentação dos designs 0021–0024 e 0047; as operações de edição
continuam iguais.

Depois do cabeçalho e da prévia da foto, acrescentada no
[refinamento de 08/10/2026](#refinamento-de-08102026), o painel começa pelos
ajustes que podem ser editados. Zoom, ângulo, opacidade
e borda deixam de mostrar frases de restauração abaixo dos sliders. Os gestos
de dois cliques permanecem disponíveis, e os limites e erros de digitação
continuam na validação por tooltip. A origem da borda e a ação para usar o
padrão do álbum continuam disponíveis porque informam a herança do design.
O Zoom da foto vai de 100% a 500%, no slider e no campo.

Espelhar horizontalmente e Preto e branco usam `PropertyToggle`, componente
neutro de UI com ícone em um botão compacto. Espelhar fica junto ao giro;
Preto e branco permanece à esquerda em Ajustes e Efeitos. Após a
aprovação do estado pressionado neutro, o usuário pediu a retirada do texto
visível. O nome da ação aparece no tooltip ao passar o mouse ou navegar pelo
teclado, além de identificar o botão para leitores de tela. O estado faz parte
do próprio botão, sem checkbox separado.

O estado desligado usa superfície neutra e borda discreta. Após o usuário
rejeitar o preenchimento azul, o estado ligado passa a parecer pressionado:
fundo cinza quente, borda neutra, sombra interna suave e ícone em grafite.
O ícone ganha um pouco de peso, de 1,4 px para 1,8 px, sem mudar suas dimensões.
A indicação permanece ao passar o mouse e ao receber foco por teclado.
Hover e foco no botão desligado mantêm a superfície clara, para não simular
ativação. O estado nunca muda a largura ou a posição dos controles.

Valores diferentes na seleção usam borda tracejada e superfície clara, sem
simular que a propriedade está ligada em todas as fotos. Os ícones vêm do
componente compartilhado do programa.

O componente reutiliza o botão compartilhado e os tokens de superfície,
borda, tipografia e espaçamento do programa. O botão mede 28 × 28 px, com
ícone centralizado de 16 px e cantos de 4 px. O tooltip reutiliza a superfície,
a borda e a sombra compartilhadas; aparece ao lado, fora do fluxo do painel,
e não desloca controles. A abertura, o fechamento e os atributos de
acessibilidade usam `useTooltipTrigger`, `useTooltip` e `useTooltipTriggerState`
(React Aria 3.50.0 e React Stately 3.48.0), conforme a
[documentação oficial](https://react-aria.adobe.com/Tooltip/useTooltipTrigger).
O tooltip é ancorado pelo CSS ao próprio controle e acompanha sua escala,
sem conversão manual de coordenadas ou posicionamento automático em portal.
O foco por teclado recebe contorno neutro ao redor do botão, separado do estado
ligado. Enter e Espaço acionam a mesma operação do clique. Nome acessível,
estado misto e bloqueio durante operações seguem o contrato existente.

A composição contextual continua responsável por selecionar as fotos e
disparar os comandos. O controle compartilhado só apresenta o estado; não
duplica regras de edição, seleção, histórico ou persistência.

A validação usa os testes existentes de orientação, efeitos, sliders e
integração do painel, além das capturas declaradas do painel individual,
seleção mista, teclado e escalas de 125% e 150%.

## Alinhamento e agrupamento dos ajustes

Os campos numéricos do quadro usam `UnitInput`, entrada reutilizável de 92 px
com a unidade integrada à área de edição. Após o usuário rejeitar a aparência
de célula, o campo passa a ter fundo transparente e apenas um sublinhado
discreto, sem contorno fechado. O sublinhado indica que o número é editável;
ao passar o mouse, ganha contraste e um fundo neutro suave. Durante a edição,
o sublinhado fica mais forte, em grafite, sem alterar as dimensões do campo.
Erros usam sublinhado vermelho e o tooltip compartilhado. Campos bloqueados
não mostram a indicação de edição. O número permanece editável e a unidade
é fixa, sem fazer parte do valor digitado. Clicar sobre a unidade também dá
foco ao campo. Zoom, ângulo, opacidade e borda compartilham esse componente;
porcentagens, graus e medidas físicas mantêm a mesma coluna. Os estados de
foco, erro, seleção mista e bloqueio preservam o contrato dos ajustes.
Entre ajustes, o espaçamento é de 12 px.

Na linha Giro, o valor atual é somente uma informação junto ao rótulo.
Ele não recebe foco nem reage a cliques ou dois cliques. A ação de restaurar
o giro foi retirada do painel e do catálogo de comandos da interface, sem
botão, ícone, tooltip ou gesto alternativo. Para voltar à posição inicial,
o usuário continua usando Girar 90° até completar a volta.

Valores diferentes exibem “—”. À direita, Girar 90° e Espelhar formam um
grupo de 92 px, com contorno contínuo e uma divisória interna, sem espaço
entre os botões. Cada ação mantém seu foco e nome acessível; o estado
pressionado neutro pertence apenas ao espelhamento. O tooltip fica voltado
para dentro do painel e não é recortado pelo grupo. O Ângulo segue
independente do giro em passos de 90°.

O nome Borda começa na mesma coluna dos demais rótulos, com a amostra de cor
ao lado. O slider ocupa toda a largura, sem recuo causado pela amostra. A
origem e a restauração do padrão do álbum dividem a linha quando há espaço.
Essas mudanças são de composição visual; comandos, seleção e histórico
mantêm seus contratos existentes.

## Padrão reutilizável dos campos de entrada

A aparência `integrated` de `TextInput` centraliza o sublinhado e os estados
de hover, foco, erro e bloqueio dos campos de entrada. `UnitInput` usa essa
aparência com unidade fixa dentro do campo, e `ValidatedTextField` permite
adotá-la sem alterar a validação ou as ações de restauração.

No painel contextual, o padrão se aplica a zoom, ângulo, opacidade, borda,
espaço entre quadros, DPI, largura e altura da lâmina, sangria e área de
segurança. As medidas continuam exibindo a unidade dentro do campo. Botões
de ação, alternadores e seletores mantêm suas apresentações próprias.

## Refinamento de 08/10/2026

O autor pediu a prévia da imagem selecionada no painel contextual. Com um
quadro com foto selecionado, a foto inteira aparece logo abaixo do cabeçalho
`Quadro selecionado` e acima de Design. É a mesma imagem de Cache da miniatura
do Painel de imagens e do visualizador, sem recorte, giro, espelhamento, preto e
branco ou opacidade: não acompanha os ajustes do painel, cujo efeito continua
visível no Canvas.

O bloco é fixo, sem título de seção nem recolhimento. A imagem ocupa toda a
largura do conteúdo do painel, com a proporção da foto, desde o
[refinamento de 08/10/2026 (3)](#refinamento-de-08102026-3); a primeira versão
tinha altura máxima de cerca de 200 px, e a segunda, um quadrado de até 360 px. Mantém a moldura branca e a
sombra curta das miniaturas; o bloco usa o mesmo respiro do conteúdo das seções
e termina com a mesma linha que as fecha, sem caixa, ícone ou divisória extra.
Quadro vazio e seleção de vários quadros não mostram prévia.

O bloco mostra só a imagem. O nome já está no cabeçalho. O design 0001 previa
também informações do arquivo para o quadro, mas o autor decidiu em 08/10/2026
não mostrar as dimensões em pixels nem outras informações abaixo da imagem.

Durante o carregamento aparece o fundo listrado da miniatura. Arquivo ausente
sem prévia retida mostra o símbolo de imagem ausente da miniatura, sem aviso ou
atalho; uma prévia anterior retida é exibida sem aviso. A troca de quadro ou de
imagem segue o [refinamento seguinte](#refinamento-de-08102026-2).

O mesmo bloco forma o contexto `Imagem selecionada`, aberto por uma imagem
selecionada no Painel de imagens. A precedência entre as seleções do Canvas e
do Painel de imagens está no
[design 0001](0001-estrutura-da-janela-do-projeto.md#contexto-de-imagem-selecionada).

## Refinamento de 08/10/2026 (2)

Depois de usar a prévia, o autor fez dois pedidos: aumentar a área, porque uma
foto vertical ficava muito pequena, e evitar a piscada que às vezes aparecia ao
trocar de uma foto para outra.

A área da prévia passou a ser um quadrado com a largura do conteúdo do
painel, limitada a 360 px (cerca de 272 × 272 px na largura padrão), com o
mesmo tamanho para qualquer proporção. O
[refinamento seguinte](#refinamento-de-08102026-3) trocou esse quadrado pela
largura inteira do painel.

Ao trocar de quadro ou de imagem, inclusive ao passar de um quadro para uma
imagem selecionada no Painel de imagens ou o contrário, a área mantém o que
exibe até a próxima prévia estar carregada e pronta para exibição. Então troca
a imagem e a proporção de uma vez, sem área vazia entre as duas. A espera é
curta: se a próxima prévia não ficar pronta em cerca de 250 ms ou não puder ser
lida, a troca acontece assim mesmo e a prévia termina de carregar na área, como
uma miniatura do Painel de imagens. Um arquivo ausente com prévia retida espera
como os demais. A troca é imediata quando a próxima imagem não tem prévia para
exibir: com a prévia ainda em preparo, aparece o fundo listrado; com o arquivo
ausente sem prévia retida, o símbolo de imagem ausente.

Durante essa espera, o cabeçalho e os controles já são os da nova seleção, e a
área continua com o que exibia: a foto anterior, o fundo listrado ou o símbolo
de imagem ausente. É a única situação em que a prévia não corresponde ao nome
do cabeçalho, e ela dura no máximo cerca de 250 ms depois de cada escolha.

Em trocas rápidas, cada troca tem a sua espera e vale a última imagem
escolhida: uma imagem intermediária nunca aparece depois de outra ter sido
escolhida. Quando o painel não mostrava prévia, como ao sair do álbum, de
Design da lâmina, de uma seleção múltipla ou de um quadro vazio, a prévia
aparece sem espera. Voltar à mesma foto, como ao passar de um quadro para a sua
imagem no Painel de imagens, não recarrega a prévia.

## Refinamento de 08/10/2026 (3)

Testando o quadrado, o autor achou a prévia ainda muito pequena e pediu que a
imagem ocupasse quase toda a largura do painel, com a margem.

A imagem passa a ocupar toda a largura do conteúdo do painel, com a margem
lateral das seções, e a altura segue a proporção da foto. Na largura padrão do
painel, uma foto horizontal 3:2 tem cerca de 272 × 181 px e uma vertical 2:3,
cerca de 272 × 408 px. Só uma foto mais alta que 60% da altura da janela fica
mais estreita, para Design continuar ao alcance. Design e os controles abaixo
passam a mudar de lugar conforme a altura da foto; a troca de imagem continua
sem piscar e troca imagem e altura de uma vez.
