---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-30
updated: 2026-07-30
---

# Navegação em Álbum longo

## Pergunta

O corte anterior já provava que o modelo conserva 100 Lâminas enquanto a cena
PixiJS descarta e reconstrói nós fora da faixa residente. Faltava executar a
mesma navegação no host real, com representações do Cache produzidas a partir
do corpus fotográfico, e medir o tempo até o destino estar efetivamente
renderizado.

O critério não tinha um limiar numérico de latência previamente definido.
Portanto, esta rodada não cria um limite retroativo para favorecer o resultado.
Ela verifica conjuntamente:

- a preservação das 100 Lâminas lógicas;
- a residência gráfica estritamente menor que o Álbum;
- a reconstrução com textura real depois de saltos distantes;
- a latência observada da ação completa;
- a preservação da fluidez de Pan e Zoom.

## Instrumento

O runner executou a build `release` nas duas topologias do spike, com dois
Projetos simultâneos em cada uma. Cada Projeto recebeu 100 Lâminas. Depois do
Cache frio e do primeiro Canvas texturizado, o probe repetiu dez vezes a rota
50ª Lâmina → 100ª Lâmina → 1ª Lâmina, totalizando 30 saltos por Projeto e
60 por topologia.

Cada amostra começa imediatamente antes da ação pública
`navigateToSheet(sheetId)`. Ela termina somente quando:

1. a Lâmina de destino está centralizada pelo mesmo fluxo usado pela Grade;
2. a cena residente foi reconciliada;
3. as Fotos do destino usam as URLs reais do Cache;
4. as texturas assíncronas do destino terminaram de carregar;
5. o ticker do PixiJS executou a confirmação posterior à renderização.

O ponto final confirma a passagem pelo renderizador JavaScript do PixiJS; ele
não pretende medir a apresentação física do frame pelo compositor do Windows.
O instrumento também registra o maior número de nós de Lâmina e texturas
mantidos pelo pool durante a rota.

Os dados completos, o hardware, hashes, processos e resultados de Cache e
Exportação estão no
[artefato bruto](artifacts/0005-long-album-navigation.json) e no
[resumo gerado](artifacts/0005-long-album-navigation.md).

## Resultados

| Topologia | Projeto | Primeira resposta | Média | p50 | p95 | Máximo | Lâminas residentes | Texturas residentes |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| A — hosts independentes | 001 | 204,0 ms | 151,813 ms | 142,4 ms | 213,2 ms | 215,3 ms | 7 | 14 |
| A — hosts independentes | 002 | 192,8 ms | 157,977 ms | 144,0 ms | 246,1 ms | 255,4 ms | 7 | 14 |
| B — host multiwindow | 001 | 143,1 ms | 151,040 ms | 138,9 ms | 232,1 ms | 234,1 ms | 7 | 14 |
| B — host multiwindow | 002 | 227,0 ms | 167,657 ms | 138,2 ms | 262,9 ms | 283,2 ms | 7 | 14 |

Em ambas as topologias, cada Projeto preservou 100 Lâminas lógicas e manteve
somente sete Lâminas e 14 texturas no pico observado. Esse número resulta do
viewport, da margem de 700 unidades e das adjacências; não é um limite
codificado para o tamanho do Álbum.

Os saltos distantes tiveram média entre 151,040 e 167,657 ms. O pior p95 foi
262,9 ms e o pior valor individual foi 283,2 ms. Todas as amostras superaram
33 ms porque essa medida inclui carregamento e decodificação assíncrona da
representação, upload da textura, reconciliação e o frame final. Esses valores
são tempos de resposta de uma navegação discreta, não frames de uma animação
contínua.

Pan e Zoom continuaram separados desse custo. O pior p95 de Pan ficou entre
13,6 e 13,9 ms, e o de Zoom entre 15,1 e 20,9 ms. Uma das 960 amostras
combinadas desses gestos superou 33 ms. Assim, aumentar o modelo para 100
Lâminas não introduziu degradação generalizada nos gestos contínuos.

## Recursos e limite da conclusão

Com duas Janelas, o working set agregado foi 2.627,1 MiB na topologia A e
2.134,6 MiB na topologia B; a memória privada foi, respectivamente,
2.086,8 MiB e 1.760,5 MiB. Esses números incluem host, WebView2 e descendentes,
não apenas o pool PixiJS.

O crescimento de memória em relação à rodada anterior não é descartado nem
atribuído exclusivamente ao Canvas. Ele permanece como entrada para o critério
separado que mede perda de contexto WebGL2, limites de textura e pressão de
memória gráfica. Esta rodada demonstra residência limitada da cena PixiJS, mas
não substitui aquele gate.

Também permanecem fora desta conclusão:

- repetição em outras GPUs e quantidades de memória;
- recuperação depois de perda real do contexto WebGL2;
- comportamento sem aceleração WebGL2 utilizável;
- calibração futura da margem de pré-carregamento;
- eventual virtualização de listas DOM fora do Canvas, se suas próprias
  medições demonstrarem necessidade.

## Conclusão

O critério de virtualização do Canvas está atendido. A implementação preserva
o Álbum lógico completo, mantém a cena e as texturas limitadas à vizinhança do
viewport, reconstrói destinos distantes com representações reais e preserva a
responsividade de Pan e Zoom no cenário de 100 Lâminas medido.

Não foi necessário introduzir tiles, pirâmides, previews persistidos de Lâmina
ou um segundo modelo de composição. A política continua localizada no adaptador
do Canvas e pode ser recalibrada por novas medições sem alterar o documento do
Projeto.
