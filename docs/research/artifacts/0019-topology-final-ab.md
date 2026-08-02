---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-08-02
updated: 2026-08-02
---

# Falhas controladas e isolamento das topologias de processo

Coletado em UTC: `2026-08-02T03:53:21.3537651Z`.
Ordem de execução: `AB`.
[JSON bruto](0019-topology-final-ab.json).

| Medida | A — hosts independentes | B — host multiwindow |
|---|---:|---:|
| Hosts do Projeto | 2 | 1 |
| Janelas do Projeto | 2 | 2 |
| Processos na árvore | 14 | 8 |
| Working set agregado | 2.400,5 MiB | 2.245,0 MiB |
| Memória privada agregada | 1.847,3 MiB | 1.858,1 MiB |
| Snapshot Windows pós-probe: memória gráfica dedicada | 0,0 MiB | 0,0 MiB |
| Snapshot Windows pós-probe: memória gráfica compartilhada | 680,2 MiB | 656,3 MiB |
| Snapshot Windows pós-probe: uso dedicado + compartilhado dos processos | 680,2 MiB | 656,3 MiB |
| Duas Janelas identificadas | 3384 ms | 2632 ms |
| Duas Janelas com Cache pronto | 31336 ms | 56179 ms |
| Dois Canvas com texturas prontos | 31421 ms | 56362 ms |
| Duração de parede do Cache frio | 26887 ms | 52864 ms |
| Fotos processadas pelo Cache | 172 | 172 |
| Decorativos PNG processados pelo Cache | 2 | 2 |
| Vazão agregada dos originais | 52,1 MiB/s | 26,5 MiB/s |
| Representações reduzidas | 49,4 MiB | 49,4 MiB |
| Pan: pior p95 entre Projetos | 17 ms | 15.7 ms |
| Pan: frames acima de 33 ms | 0 | 0 |
| Zoom: pior p95 entre Projetos | 22.7 ms | 18 ms |
| Zoom: frames acima de 33 ms | 0 | 0 |
| Canvas: versão WebGL confirmada | 2 | 2 |
| Canvas: GL_MAX_TEXTURE_SIZE consultado (mínimo) | 16384 px | 16384 px |
| Canvas: GL_MAX_RENDERBUFFER_SIZE consultado (mínimo) | 16384 px | 16384 px |
| Canvas: GL_MAX_TEXTURE_IMAGE_UNITS consultado (mínimo) | 16 | 16 |
| Canvas: textura real exercitada | 1600 x 1200 px | 1600 x 1200 px |
| Canvas: contextos perdidos e restaurados | 2 perdidos / 2 restaurados | 2 perdidos / 2 restaurados |
| Canvas: maior duração observada da recuperação | 127.3 ms | 164.8 ms |
| Canvas: maior latência observada do frame restaurado | 79.1 ms | 87.1 ms |
| Navegação: pior p95 entre Projetos | 227.2 ms | 313.7 ms |
| Navegação: respostas acima de 33 ms | 60 | 60 |
| Navegação: pico de Lâminas residentes | 7 | 7 |
| Navegação: pico de texturas residentes | 14 | 14 |
| Navegação: soma dos picos de pixels residentes por Projeto | 47801600 | 47801600 |
| Navegação: estimativa RGBA8 da soma dos picos por Projeto | 182,3 MiB | 182,3 MiB |
| Exportação: duração | 2376 ms | 2458 ms |
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

- Artefato validado: `target/topology-final-comparison/imaging-recovery.json`.
- SHA-256: `3694123e77d2187b59ec36d8fcf9af8b5e89153dd1fc295c51107fa901fd7997`.
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

- Commit do código: `e5e3c8d4bc2009092a043baf359fa2dc3907fce6`
- Build concluída em UTC: `2026-08-02T03:50:20.4129000Z`
- Perfil: `release`
- Árvore de trabalho tinha mudanças alheias: sim
- Entradas da build tinham mudanças: não
- Arquivos de entrada: 226
- Digest das entradas: `fb47bba04e56af7a21e62636ffd862559308c1127d605e8832d9b3f021e84d3f`
- Hash do host: `0f5cd2b6608200c18a4290787eef7921a732b702cab5a10ddb4ff28ee0e300a5`
- Hash do Processador de Imagens: `3fa637332bc57ad6ebeeb1f0e7e9d3f4ec36ef0efd873e9700c333cffe7c61d0`
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
- O runner captura a identidade do host e de seus descendentes antes da queda, encerra toda a árvore e confirma que nenhum descendente identificado permaneceu ativo.
- O gate de falhas aplica uma intenção real, persiste atomicamente, relê pelo núcleo e só então confirma a revisão como salva.
- A recuperação do host reabre apenas a última revisão explicitamente salva; recuperação de alterações não salvas permanece fora deste gate.
- A IPC é descrita por limites, relações e contagens mínimas observáveis; nenhum escore sintético de complexidade foi inventado.
- A evidência do Processador de Imagens é validada e referenciada pelo artefato informado ao runner, sem duplicar seu mecanismo de queda.
