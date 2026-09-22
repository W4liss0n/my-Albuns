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
neutro de UI com o nome à esquerda e um indicador quadrado à direita. O
indicador vazio representa desligado, a marca representa ligado e o traço
representa valores diferentes na seleção. O azul se restringe ao indicador
ligado; o texto mantém a cor dos demais rótulos do painel.

O componente reutiliza o botão compartilhado e os tokens de superfície,
borda, tipografia e espaçamento do programa. A linha tem altura mínima de
28 px, tipografia de controle e indicador de 16 px; toda a linha é clicável.
O foco por teclado recebe contorno neutro no indicador, separado do estado
ligado. Enter e Espaço acionam a mesma operação do clique. Nome acessível,
estado misto e bloqueio durante operações seguem o contrato existente.

A composição contextual continua responsável por selecionar as fotos e
disparar os comandos. O controle compartilhado só apresenta o estado; não
duplica regras de edição, seleção, histórico ou persistência.

A validação usa os testes existentes de orientação, efeitos, sliders e
integração do painel, além das capturas declaradas do painel individual,
seleção mista, teclado e escalas de 125% e 150%.
