---
status: current
document: research
date: 2026-09-23
---

# Ciclo das análises de rostos

## Decisão e alcance

Os achados #128 e #129 foram aprovados para implementação após a revisão de
`25d394a2`. A seleção deve identificar a versão da foto de onde vieram os
pontos, e a detecção deve abandonar pedidos sem consumidores. Isso não altera
a busca de rostos, os limites do modelo, a composição dos olhos ou a confirmação
para substituir o original. A regra normativa está no design
[0050](../design/0050-visualizador-integrado-de-imagens.md).

## Contratos consultados

O pacote instalado é `@mediapipe/tasks-vision` **1.0.1**. Sua declaração local
e o [guia oficial do Face Landmarker Web](https://developers.google.com/edge/mediapipe/solutions/vision/face_landmarker/web_js)
documentam a inferência síncrona. Uma chamada já iniciada termina antes que o
worker possa tratar outro evento.

[postMessage](https://developer.mozilla.org/en-US/docs/Web/API/Worker/postMessage)
enfileira trabalho no destino; não interrompe a inferência.
[setTimeout no worker](https://developer.mozilla.org/en-US/docs/Web/API/WorkerGlobalScope/setTimeout)
com atraso zero devolve a execução ao ciclo de eventos, com atraso real
potencialmente maior. A implementação verifica cancelamento entre regiões
e cede por blocos de até oito passagens ou após 64 ms de trabalho. Esse orçamento
é um ponto de verificação, não um prazo garantido de cancelamento.

O Context7 já havia atingido a cota nesta conversa. A consulta usou as fontes
primárias acima e os tipos instalados, sem depender de memória sobre a API.

## Ensaio do detector real

Antes: worker de `25d394a2`. Depois: worker com cancelamento cooperativo e
orçamento de 64 ms. Edge 153 em modo headless, MediaPipe real, mesmo modelo CPU,
mesmas fotografias de teste, prévias JPEG a 90% com até 1.600 pixels no maior lado.
As fotografias originais do usuário não foram modificadas.

O ensaio aquece uma instância do modelo, analisa nove fotos e repete três vezes
a sequência: começar A, aguardar 80 ms, abandonar A e solicitar B. No caso
anterior, A termina; no novo, uma mensagem libera o pedido de A. Instrumentação
conta as chamadas ao modelo e mede o tempo dentro delas. O tempo de inferência
é tempo decorrido, não uma medição de CPU do processo.

| Medida nas três trocas | Antes | Depois |
| --- | ---: | ---: |
| Mediana da espera após trocar para B | 3.942 ms | 2.084 ms |
| Inferências de A abandonada | 292 / 292 / 292 | 8 / 16 / 16 |
| Inferências de B necessária | 288 em cada execução | 288 em cada execução |
| Rostos de B | 1 | 1 |

A espera caiu aproximadamente **47% neste ensaio**. Os pontos completos de B
foram idênticos nas seis execuções. Os nove testes individuais também conservaram
o hash de todos os pontos, além da quantidade de rostos:

| Foto de teste | Rostos antes e depois |
| --- | ---: |
| Controle | 1 |
| Destino IMG_6187 | 1 |
| Referência IMG_6186 | 1 |
| IMG_6246 | 4 |
| IMG_6252 | 6 |
| IMG_6276 | 7 |
| IMG_6300 | 4 |
| IMG_6510 | 5 |
| IMG_6498 | 9 |

Foi descartado um orçamento intermediário de 16 ms: ele interrompia trabalho
abandonado, mas acrescentava cerca de 0,5–0,65 segundo de espera fora das
inferências a cada análise. Com 64 ms, esse custo ficou entre 191 e 273 ms nas
oito fotos após o aquecimento. A busca individual continua com o mesmo trabalho
de inferência; a melhoria está em evitar que a foto atual espere por trabalho
que já não interessa. Não se promete aceleração equivalente quando a pessoa
mantém a mesma foto até a análise terminar.

## Limites e reprodução

A sondagem executa o worker real isolado, sem a janela Tauri nem o compositor
Rust. Não mede a latência total entre selecionar dois rostos e ver a correção.
Essa medição nativa continua registrada separadamente em #124. A amostra também
não comprova que o modelo detecte todos os rostos em qualquer fotografia.

Script, fontes medidas, dados completos e hashes estão em
`%TEMP%/myalbuns-analysis-lifecycle-20260923/`: `benchmark.mjs`,
`baseline.worker.js`, `baseline.json`, `after-16ms.json`, `after.json` e
`summary.json`. O script roda a partir da raiz do repositório e usa as cópias
fotográficas locais já existentes em `.scratch/face-detection-debug-20260923/inputs`.
Essas imagens não são material de teste portátil nem são versionadas.
