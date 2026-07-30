---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-30
updated: 2026-07-30
---

# Navegação em Álbum longo com imagens reais

Coletado em UTC: `2026-07-30T19:41:19.8951964Z`.
[JSON bruto](0005-long-album-navigation.json).

| Medida | A — hosts independentes | B — host multiwindow |
|---|---:|---:|
| Hosts do Projeto | 2 | 1 |
| Janelas do Projeto | 2 | 2 |
| Processos na árvore | 14 | 8 |
| Working set agregado | 2.385,4 MiB | 2.223,2 MiB |
| Memória privada agregada | 1.833,3 MiB | 1.841,8 MiB |
| Memória gráfica compartilhada | 793,5 MiB | 650,8 MiB |
| Duas Janelas identificadas | 1554 ms | 535 ms |
| Duas Janelas com Cache pronto | 29797 ms | 27395 ms |
| Dois Canvas com texturas prontos | 30111 ms | 27629 ms |
| Duração de parede do Cache frio | 27158 ms | 26435 ms |
| Fotos processadas pelo Cache | 172 | 172 |
| Decorativos PNG processados pelo Cache | 2 | 2 |
| Vazão agregada dos originais | 51,6 MiB/s | 53,0 MiB/s |
| Representações reduzidas | 49,4 MiB | 49,4 MiB |
| Pan: pior p95 entre Projetos | 15.3 ms | 13.9 ms |
| Pan: frames acima de 33 ms | 0 | 0 |
| Zoom: pior p95 entre Projetos | 13.7 ms | 14.7 ms |
| Zoom: frames acima de 33 ms | 0 | 0 |
| Navegação: pior p95 entre Projetos | 202.5 ms | 240.8 ms |
| Navegação: respostas acima de 33 ms | 60 | 60 |
| Navegação: pico de Lâminas residentes | 7 | 7 |
| Navegação: pico de texturas residentes | 14 | 14 |
| Exportação: duração | 2471 ms | 3307 ms |
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

- Commit do código: `cb0208171b2f87ea9021826721ff2049498e572d`
- Build concluída em UTC: `2026-07-30T19:39:48.2230113Z`
- Perfil: `release`
- Árvore de trabalho tinha mudanças alheias: sim
- Entradas da build tinham mudanças: sim
- Arquivos de entrada: 169
- Digest das entradas: `b2130e2b14d1b9937161e7481da64f913c965337f4e97b951aa3cf7f03db26c5`
- Hash do host: `4afa8cf1cf1af2d8e21698cd53be013adee3302bacd925468ee3084af16c3c34`
- Hash do Processador de Imagens: `a64511562b28d773adbb637bee023dd41f2d3290726b8f027adb88c1b124de6f`
- Checkout atual corresponde ao manifesto: sim

## Ambiente registrado

- Sistema: Microsoft Windows 11 Pro `10.0.26200`
- Processador: 13th Gen Intel(R) Core(TM) i5-13450HX
- Memória física: 24.260,7 MiB

## Campos ainda não medidos

- recuperação persistida
- complexidade operacional da IPC

## Observações

- O Cache foi reconstruído a frio com uma representação de até 1600 px por mídia: JPEG para Fotos opacas e PNG para o Decorativo transparente.
- Pan e Zoom foram medidos separadamente sobre uma textura real de Foto, e o mesmo Canvas confirmou a textura PNG real do Decorativo, depois de 24 frames de aquecimento.
- A navegação percorreu 10 vezes a primeira, a 50ª, a 100ª e de volta à primeira Lâmina, aguardando a textura real do destino e o frame renderizado pelo PixiJS.
- As duas Janelas iniciaram o probe pelo mesmo arquivo-gate, somente depois da conclusão do Cache frio.
- A Exportação mediu a primeira Lâmina do Álbum principal a 300 DPI, lendo e verificando as Fotos JPEG e o Decorativo PNG originais.
- Cada uso valida o tamanho e o SHA-256 da mídia; o corpus completo foi recalculado depois das duas alternativas.
- A memória inclui o host e todos os processos descendentes observados.
- A Exportação foi liberada por um segundo gate somente depois dos dois probes de Canvas.
- Os dois hosts independentes foram iniciados antes da espera pelas Janelas, usando o mesmo marco inicial da alternativa multiwindow.
- A queda só é forçada depois de validar o caminho do executável do PID alvo.
