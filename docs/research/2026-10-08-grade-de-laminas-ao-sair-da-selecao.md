---
status: current
document: research
date: 2026-10-08
platform: windows-11-x64
---

# Grade de Lâminas ao sair de uma seleção

Relato de 8/10/2026: às vezes, ao sair da seleção de um Frame, a Grade de
Lâminas aparecia com as miniaturas vazias e as fotos surgiam aos poucos,
Lâmina por Lâmina. A base de código é o commit `0c60fa4d`.

## Como foi medido

- Compilação de desenvolvimento (`--debug`), com dados isolados em
  `MYALBUNS_PROCESS_GATE_DATA_ROOT`, fora dos dados do aplicativo instalado.
- Projeto de teste local com 40 fotos JPEG de 6000 × 4000 pixels, quatro por
  Lâmina, em dez Lâminas. `Informações do álbum` e `Design do álbum` ficaram
  recolhidos para a Grade aparecer inteira.
- Cada ciclo, pelo DevTools do WebView2: clicar num Frame, esperar, clicar
  fora das Lâminas e gravar os quadros da tela (`Page.startScreencast`). Um
  quadro conta como incompleto quando a área das fotos da Grade tem mais de
  10 pontos percentuais de pixels brancos acima do quadro final. Também se
  contam os pedidos ao protocolo do Cache depois do clique.
- Cache frio: uma coleta de lixo do WebView (`HeapProfiler.collectGarbage`)
  enquanto o Frame está selecionado, o que acontece naturalmente quando a
  seleção dura algum tempo.

## Resultados

| Situação | Ciclos | Quadros incompletos | Prévias pedidas de novo |
| --- | --- | --- | --- |
| Antes, volta rápida | 3 | 0 | 0 |
| Antes, com coleta de lixo | 4 | 4 a 5 por ciclo, cerca de 200 ms | 40 por ciclo |
| Depois, com coleta de lixo | 6 | 0 | 0 |

Com o cache frio, remontar a Grade custava 79 ms para buscar as 40 prévias de
1600 px (11,7 MB) no protocolo do Cache e 356 ms para decodificá-las em
paralelo; uma de cada vez, cada foto levava de 12 a 19 ms.

## Causa

A Grade só existia no contexto do Álbum. Selecionar um Frame a desmontava, e
voltar criava miniaturas novas. As prévias do Cache são servidas com
`cache-control: no-store`, então o WebView só reaproveita uma imagem enquanto
algum elemento ainda a referencia. Depois de uma coleta de lixo, cada
miniatura nova buscava e decodificava de novo sua prévia de até 1600 px, e a
Grade era desenhada antes das fotos.

## Decisão

A Grade permanece montada, apenas oculta, enquanto o Painel contextual mostra
outro contexto (`<Activity mode="hidden">` do React). Os formulários do Álbum
continuam sendo desmontados como antes. Arrastar o Zoom de um Frame com a
Grade oculta manteve 8,3 ms de mediana e 8,5 ms no percentil 95 por quadro,
sem tarefas longas.

## Limites

- A primeira exibição da Grade numa sessão ainda depende de as prévias
  chegarem e serem decodificadas.
- Cada foto da Grade ocupa cerca de 28 px, mas usa a prévia de até 1600 px.
  Em Álbuns grandes, a decodificação pode voltar a pesar se o WebView
  descartar as imagens decodificadas. Uma representação menor para as
  miniaturas contraria a representação única do design 0010 e exigiria nova
  decisão.
- As outras prévias vetoriais de Lâmina, na Barra da Lâmina e no `Design da
  lâmina`, não foram medidas.
