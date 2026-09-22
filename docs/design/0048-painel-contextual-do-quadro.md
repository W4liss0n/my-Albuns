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

Os campos numéricos do quadro compartilham a mesma coluna: entrada de 64 px
e espaço de 24 px para a unidade. Assim, porcentagens, graus e medidas físicas
mantêm a mesma posição. Entre ajustes, o espaçamento é de 12 px.

Na linha Giro, a ação de girar 90° e o valor atual formam um controle único,
com separação interna discreta. O próprio valor atual acompanha o ícone de
restauração; não existe outro botão avulso com “0°”. Restaurar continua
indisponível quando todas as fotos já estão em zero, mantendo o valor legível
e a geometria estável. Valores diferentes exibem “—”. O espelhamento fica ao
lado desse grupo, com o tooltip voltado para dentro do painel. O Ângulo segue
independente do giro em passos de 90°.

O nome Borda começa na mesma coluna dos demais rótulos, com a amostra de cor
ao lado. O slider ocupa toda a largura, sem recuo causado pela amostra. A
origem e a restauração do padrão do álbum dividem a linha quando há espaço.
Essas mudanças são de composição visual; comandos, seleção e histórico
mantêm seus contratos existentes.
