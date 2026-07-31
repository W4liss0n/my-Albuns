---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-30
updated: 2026-07-30
---

# Falhas controladas e isolamento das topologias de processo

Coletado em UTC: `2026-07-31T00:13:37.3437448Z`.
[JSON bruto](0007-process-failure-gate.json).

| Medida | A — hosts independentes | B — host multiwindow |
|---|---:|---:|
| Hosts do Projeto | 2 | 1 |
| Janelas do Projeto | 2 | 2 |
| Processos na árvore | 14 | 8 |
| Working set agregado | 2.224,3 MiB | 1.950,3 MiB |
| Memória privada agregada | 1.670,7 MiB | 1.562,8 MiB |
| Snapshot Windows pós-probe: memória gráfica dedicada | 0,0 MiB | 0,0 MiB |
| Snapshot Windows pós-probe: memória gráfica compartilhada | 607,7 MiB | 527,4 MiB |
| Snapshot Windows pós-probe: uso dedicado + compartilhado dos processos | 607,7 MiB | 527,4 MiB |
| Duas Janelas identificadas | 4730 ms | 2457 ms |
| Duas Janelas com Cache pronto | 33922 ms | 46020 ms |
| Dois Canvas com texturas prontos | 34206 ms | 46520 ms |
| Duração de parede do Cache frio | 27914 ms | 42278 ms |
| Fotos processadas pelo Cache | 172 | 172 |
| Decorativos PNG processados pelo Cache | 2 | 2 |
| Vazão agregada dos originais | 50,2 MiB/s | 33,1 MiB/s |
| Representações reduzidas | 49,4 MiB | 49,4 MiB |
| Pan: pior p95 entre Projetos | 14.3 ms | 47.5 ms |
| Pan: frames acima de 33 ms | 0 | 16 |
| Zoom: pior p95 entre Projetos | 12.6 ms | 51.8 ms |
| Zoom: frames acima de 33 ms | 0 | 18 |
| Canvas: versão WebGL confirmada | 2 | 2 |
| Canvas: GL_MAX_TEXTURE_SIZE consultado (mínimo) | 16384 px | 16384 px |
| Canvas: GL_MAX_RENDERBUFFER_SIZE consultado (mínimo) | 16384 px | 16384 px |
| Canvas: GL_MAX_TEXTURE_IMAGE_UNITS consultado (mínimo) | 16 | 16 |
| Canvas: textura real exercitada | 1600 x 1200 px | 1600 x 1200 px |
| Canvas: contextos perdidos e restaurados | 2 perdidos / 2 restaurados | 2 perdidos / 2 restaurados |
| Canvas: maior duração observada da recuperação | 98.3 ms | 166.8 ms |
| Canvas: maior latência observada do frame restaurado | 73.7 ms | 104.1 ms |
| Navegação: pior p95 entre Projetos | 182.2 ms | 1451 ms |
| Navegação: respostas acima de 33 ms | 60 | 60 |
| Navegação: pico de Lâminas residentes | 7 | 7 |
| Navegação: pico de texturas residentes | 14 | 14 |
| Navegação: soma dos picos de pixels residentes por Projeto | 47801600 | 47801600 |
| Navegação: estimativa RGBA8 da soma dos picos por Projeto | 182,3 MiB | 182,3 MiB |
| Exportação: duração | 2609 ms | 2512 ms |
| Exportação: dimensões a 300 DPI | 7087 x 3543 px | 7087 x 3543 px |
| Exportação: volume dos originais | 22,7 MiB | 22,7 MiB |
| Exportação: tamanho do PNG | 23,3 MiB | 23,3 MiB |
| Janelas depois da queda forçada | 1 (outra Janela preservada) | 0 |

## Falhas controladas

| Evidência | A — hosts independentes | B — host multiwindow |
|---|---:|---:|
| Janelas próprias do processo global leve | 0 | 0 |
| Working set do processo global leve | 14,1 MiB | 14,1 MiB |
| Janelas preservadas com o processo global indisponível | 2 | 2 |
| Projetos editados, salvos e relidos com o global indisponível | 2 | 2 |
| Reinício global explícito trocou o PID | sim | sim |
| Projetos editados, salvos e relidos após o reinício global | 2 | 2 |
| Janelas depois da queda do host | 1 (outra Janela preservada) | 0 |
| Última revisão salva reaberta após reinício explícito do host | sim | sim |
| Relações host → global interrompidas pela queda global | 2 | 1 |
| Comandos mínimos ao host por probe de Projeto | 4 | 4 |
| Interações correlacionadas mínimas, incluindo status global | 5 | 5 |
| Eventos de falha do probe | 0 | 0 |

Os quatro comandos mínimos por probe são `topology_fault_probe_config`, `project_state`, `apply_project_intent` e `persist_topology_fault_probe`. A quinta interação é o status tipado do host para o processo global. Polls adicionais não são estimados.

## Processador de Imagens

- Artefato validado: `docs/research/artifacts/0004-imaging-recovery.json`.
- SHA-256: `65e9dc7fce21740fda906faa8e62f357dc46129f5ac94558f5e6b8f31bf9b210`.
- Cache recuperou após um reinício explícito: sim.
- Exportação falhou com segurança até o retry explícito: sim.
- Mesmo commit da build topológica: sim.

## Corpus real

- Álbuns: 2
- Fotos JPEG: 172
- Decorativos PNG: 1
- Volume dos originais: 1.401,0 MiB
- Digest do corpus: `c160ef3e3a2a1c7401f10574e3383fddb830678eb3fbc984dbca28dc59ebd01f`
- Integridade antes/depois: confirmada por SHA-256
- Política da representação: uma prévia por mídia (JPEG opaco ou PNG transparente), com aresta máxima de 1600 px

## Build medida

- Commit do código: `65919f632feb32a53a22b16db3cbc551d82653f6`
- Build concluída em UTC: `2026-07-31T00:10:49.8738196Z`
- Perfil: `release`
- Árvore de trabalho tinha mudanças alheias: sim
- Entradas da build tinham mudanças: não
- Arquivos de entrada: 182
- Digest das entradas: `8b491979a3717ab92708fac35591a4e39cd4cb868787e555e890cb502cd1c30a`
- Hash do host: `e28e86714a9c7e0331c3de236d1dfc743d96787f3e0e9426e80333888fde3802`
- Hash do Processador de Imagens: `5b90bfd5785c84e70224733cbc5b3cb85ee304f500e22afad33c56185f40222b`
- Checkout atual corresponde ao manifesto: sim

## Ambiente registrado

- Sistema: Microsoft Windows 11 Pro `10.0.26200`
- Processador: 13th Gen Intel(R) Core(TM) i5-13450HX
- Memória física: 24.260,7 MiB

## Campos ainda não medidos

- alocação sintética em MAX_TEXTURE_SIZE ao quadrado ou indução de OOM
- pico temporal de memória gráfica e orçamento global do driver
- checkpoint automático de alterações ainda não salvas e restauração de gesto em andamento

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
- O gate de falhas aplica uma intenção real, persiste atomicamente, relê pelo núcleo e só então confirma a revisão como salva.
- A recuperação do host reabre apenas a última revisão explicitamente salva; recuperação de alterações não salvas permanece fora deste gate.
- A IPC é descrita por limites, relações e contagens mínimas observáveis; nenhum escore sintético de complexidade foi inventado.
- A evidência do Processador de Imagens é validada e referenciada pelo artefato 0004, sem duplicar seu mecanismo de queda neste runner.
