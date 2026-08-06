---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-30
updated: 2026-07-30
---

# WebGL2: recuperação, limites e pressão gráfica observada

Coletado em UTC: `2026-07-30T21:02:20.0508410Z`.
[JSON bruto](0006-webgl2-graphics-gate.json).

| Medida | A — hosts independentes | B — host multiwindow |
|---|---:|---:|
| Hosts do Projeto | 2 | 1 |
| Janelas do Projeto | 2 | 2 |
| Processos na árvore | 14 | 8 |
| Working set agregado | 2.638,7 MiB | 1.849,9 MiB |
| Memória privada agregada | 2.083,1 MiB | 1.464,0 MiB |
| Snapshot Windows pós-probe: memória gráfica dedicada | 0,0 MiB | 0,0 MiB |
| Snapshot Windows pós-probe: memória gráfica compartilhada | 681,2 MiB | 651,8 MiB |
| Snapshot Windows pós-probe: uso dedicado + compartilhado dos processos | 681,2 MiB | 651,8 MiB |
| Duas Janelas identificadas | 1188 ms | 472 ms |
| Duas Janelas com Cache pronto | 31668 ms | 32519 ms |
| Dois Canvas com texturas prontos | 31927 ms | 32636 ms |
| Duração de parede do Cache frio | 28714 ms | 31523 ms |
| Fotos processadas pelo Cache | 172 | 172 |
| Decorativos PNG processados pelo Cache | 2 | 2 |
| Vazão agregada dos originais | 48,8 MiB/s | 44,4 MiB/s |
| Representações reduzidas | 49,4 MiB | 49,4 MiB |
| Pan: pior p95 entre Projetos | 16.5 ms | 16.9 ms |
| Pan: frames acima de 33 ms | 0 | 0 |
| Zoom: pior p95 entre Projetos | 16.6 ms | 18.5 ms |
| Zoom: frames acima de 33 ms | 0 | 0 |
| Canvas: versão WebGL confirmada | 2 | 2 |
| Canvas: GL_MAX_TEXTURE_SIZE consultado (mínimo) | 16384 px | 16384 px |
| Canvas: GL_MAX_RENDERBUFFER_SIZE consultado (mínimo) | 16384 px | 16384 px |
| Canvas: GL_MAX_TEXTURE_IMAGE_UNITS consultado (mínimo) | 16 | 16 |
| Canvas: textura real exercitada | 1600 x 1200 px | 1600 x 1200 px |
| Canvas: contextos perdidos e restaurados | 2 perdidos / 2 restaurados | 2 perdidos / 2 restaurados |
| Canvas: maior duração observada da recuperação | 116.9 ms | 154.2 ms |
| Canvas: maior latência observada do frame restaurado | 80.7 ms | 105.7 ms |
| Navegação: pior p95 entre Projetos | 256.5 ms | 267.3 ms |
| Navegação: respostas acima de 33 ms | 60 | 60 |
| Navegação: pico de Lâminas residentes | 7 | 7 |
| Navegação: pico de texturas residentes | 14 | 14 |
| Navegação: soma dos picos de pixels residentes por Projeto | 47801600 | 47801600 |
| Navegação: estimativa RGBA8 da soma dos picos por Projeto | 182,3 MiB | 182,3 MiB |
| Exportação: duração | 2486 ms | 2660 ms |
| Exportação: dimensões a 300 DPI | 7087 x 3543 px | 7087 x 3543 px |
| Exportação: volume dos originais | 22,7 MiB | 22,7 MiB |
| Exportação: tamanho do PNG | 23,3 MiB | 23,3 MiB |
| Janelas depois da queda forçada | 1 (outra Janela preservada) | 0 |

## Corpus real

- Álbuns: 2
- Fotos JPEG: 172
- Decorativos PNG: 1
- Volume dos originais: 1.401,0 MiB
- Digest do corpus: `c160ef3e3a2a1c7401f10574e3383fddb830678eb3fbc984dbca28dc59ebd01f`
- Integridade antes/depois: confirmada por SHA-256
- Política da representação: uma prévia por mídia (JPEG opaco ou PNG transparente), com aresta máxima de 1600 px

## Build medida

- Commit do código: `0d26df480c7868f6391f3b563cea6087f537f673`
- Build concluída em UTC: `2026-07-30T21:00:35.2635382Z`
- Perfil: `release`
- Árvore de trabalho tinha mudanças alheias: sim
- Entradas da build tinham mudanças: sim
- Arquivos de entrada: 174
- Digest das entradas: `1a13c9d62aac5841f55a0341ad5ec37fc949da771cd465f97eb85ffb21e3caf0`
- Hash do host: `c731d7801259068cabe4a441cf39bb0258f5fd94f5057971a1f996416eb65d9d`
- Hash do Processador de Imagens: `a64511562b28d773adbb637bee023dd41f2d3290726b8f027adb88c1b124de6f`
- Checkout atual corresponde ao manifesto: sim

## Ambiente registrado

- Sistema: Microsoft Windows 11 Pro `10.0.26200`
- Processador: 13th Gen Intel(R) Core(TM) i5-13450HX
- Memória física: 24.260,7 MiB

## Campos ainda não medidos

- recuperação persistida
- complexidade operacional da IPC
- alocação sintética em MAX_TEXTURE_SIZE ao quadrado ou indução de OOM
- pico temporal de memória gráfica e orçamento global do driver

## Observações

- O Cache foi reconstruído a frio com uma representação de até 1600 px por mídia: JPEG para Fotos opacas e PNG para o Decorativo transparente.
- Cada Canvas consultou os limites WebGL2 sem tentar alocar MAX_TEXTURE_SIZE ao quadrado nem provocar OOM.
- O gate gráfico usou o Decorativo PNG real de 1600 x 1200 px, forçou perda e restauração pelo WEBGL_lose_context e confirmou novamente as texturas reais de Foto e Decorativo.
- Pan e Zoom foram medidos separadamente sobre uma textura real de Foto depois de 24 frames de aquecimento.
- A navegação percorreu 10 vezes a primeira, a 50ª, a 100ª e de volta à primeira Lâmina, aguardando a textura real do destino e o frame renderizado pelo PixiJS.
- Pixels residentes são contados pelas dimensões das texturas materializadas; o volume RGBA8 é uma estimativa mecânica de quatro bytes por pixel, não uma leitura da memória do driver.
- As duas Janelas iniciaram o probe pelo mesmo arquivo-gate, somente depois da conclusão do Cache frio.
- A Exportação mediu a primeira Lâmina do Álbum principal a 300 DPI, lendo e verificando as Fotos JPEG e o Decorativo PNG originais.
- Cada uso valida o tamanho e o SHA-256 da mídia; o corpus completo foi recalculado depois das duas alternativas.
- A memória inclui o host e todos os processos descendentes observados.
- O snapshot de memória gráfica do Windows foi capturado depois de todos os probes de Canvas e antes de liberar a Exportação; ele não representa um pico.
- A Exportação foi liberada por um segundo gate somente depois dos dois probes de Canvas e desse snapshot gráfico.
- Os dois hosts independentes foram iniciados antes da espera pelas Janelas, usando o mesmo marco inicial da alternativa multiwindow.
- A queda só é forçada depois de validar o caminho do executável do PID alvo.
