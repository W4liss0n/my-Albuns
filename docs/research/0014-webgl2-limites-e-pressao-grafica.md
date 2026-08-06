---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-30
updated: 2026-07-30
---

# WebGL2, recuperação e pressão gráfica

## Pergunta

O editor exige WebGL2 acelerado por hardware. Este gate precisava demonstrar,
no Canvas PixiJS real, que:

- o contexto WebGL2 pode ser perdido e recuperado sem recriar a Aplicação, a
  Cena ou os Assets;
- os limites relevantes do driver são consultados e cobrem uma textura real do
  corpus;
- a residência de texturas e um snapshot do uso gráfico dos processos são
  registrados sem provocar uma alocação artificial perigosa;
- quando o diagnóstico não confirma aceleração utilizável, somente o editor é
  bloqueado e Boas-vindas, Configurações e Diagnóstico continuam acessíveis.

## Contrato implementado

O diagnóstico inicial cria um contexto WebGL2 descartável, exige limites
positivos e recusa ausência de contexto, backend inconclusivo e rasterizadores
de software conhecidos. Depois que o PixiJS inicia, o mesmo diagnóstico é
repetido no elemento `<canvas>` real; falha nessa segunda fronteira também
retira o editor de operação.

O Canvas mantém os listeners públicos `webglcontextlost` e
`webglcontextrestored` durante toda a sua vida. Durante a perda ele suspende a
interação transitória, mostra o estado de recuperação e inicia um timeout de
segurança. Na restauração, reutiliza a mesma Aplicação PixiJS, a mesma Cena e
os mesmos Assets, espera um frame efetivamente renderizado e só então volta ao
estado pronto.

O ensaio controlado usa `WEBGL_lose_context`, confirma
`WebGLRenderingContext.isContextLost()`, aguarda a restauração pública, força a
conclusão dos comandos com `finish()`, consulta `getError()` e comprova
novamente uma textura de Foto e o Decorativo real. Cancelamento ou erro depois
da perda executa uma restauração de emergência sem mascarar a falha original.

O shell degradado não importa nem inicia `ProjectSession`, Cache ou PixiJS. Ele
abre em Boas-vindas e permite navegar por Configurações — com as seções
Desempenho e Photoshop — e Diagnóstico. As funções completas dessas superfícies
continuam nos tickets próprios; o gate não as simula nem as antecipa.

## Instrumento e massa

A execução canônica reconstruiu os executáveis em perfil `release` e exercitou
as duas topologias:

- A — dois hosts independentes, um por Projeto;
- B — um host multiwindow com dois Projetos isolados.

Cada alternativa abriu dois Projetos de 100 Lâminas sobre o mesmo corpus de
172 Fotos JPEG e um Decorativo PNG RGBA, totalizando 1.401,0 MiB de originais.
O Cache foi reconstruído a frio e publicou uma representação de até 1600 px por
mídia. A integridade antes e depois foi confirmada pelo SHA-256
`c160ef3e3a2a1c7401f10574e3383fddb830678eb3fbc984dbca28dc59ebd01f`.

O manifesto registra 174 arquivos de entrada, digest
`1a13c9d62aac5841f55a0341ad5ec37fc949da771cd465f97eb85ffb21e3caf0` e
confirma que o estado atual corresponde exatamente ao executável medido.

## Resultados

| Evidência | A — hosts independentes | B — host multiwindow |
| --- | ---: | ---: |
| Canvas WebGL2 medidos | 2 | 2 |
| Contextos perdidos e restaurados | 2 de 2 | 2 de 2 |
| Maior duração observada da recuperação | 116,9 ms | 154,2 ms |
| Maior latência observada do primeiro frame restaurado | 80,7 ms | 105,7 ms |
| Erro WebGL depois da restauração | 0 | 0 |
| `GL_MAX_TEXTURE_SIZE` mínimo consultado | 16.384 px | 16.384 px |
| `GL_MAX_RENDERBUFFER_SIZE` mínimo consultado | 16.384 px | 16.384 px |
| `GL_MAX_TEXTURE_IMAGE_UNITS` mínimo consultado | 16 | 16 |
| Textura real exercitada | 1.600 × 1.200 px | 1.600 × 1.200 px |
| Pico de texturas residentes por Projeto | 14 | 14 |
| Pico de pixels residentes por Projeto | 23.900.800 | 23.900.800 |
| Soma dos picos de pixels dos dois Projetos | 47.801.600 | 47.801.600 |
| Estimativa RGBA8 dessa soma | 182,3 MiB | 182,3 MiB |
| Snapshot pós-probe do uso gráfico dos processos | 681,2 MiB | 651,8 MiB |

As quatro perdas foram observadas e as quatro restaurações terminaram com um
novo frame, as texturas de Foto e Decorativo novamente materializadas e
`glError = 0`.

Três magnitudes diferentes não são tratadas como equivalentes:

1. `16.384 px` é um limite consultado do driver, não uma textura alocada;
2. `1.600 × 1.200 px` é a textura real efetivamente exercitada;
3. `47.801.600 pixels` é a soma dos maiores conjuntos residentes observados
   separadamente nos dois Projetos, cuja estimativa RGBA8 de 182,3 MiB considera
   mecanicamente quatro bytes por pixel.

O snapshot do Windows é uma leitura posterior aos probes, antes da Exportação,
do uso dedicado mais compartilhado da árvore de processos. Neste equipamento
híbrido o contador atribuiu todo o volume à memória compartilhada. Ele não é a
capacidade da GPU, não é um pico temporal e não deve ser somado à estimativa
RGBA8.

Os dados brutos, hardware, hashes e métricas completas estão no
[artefato JSON](artifacts/0006-webgl2-graphics-gate.json) e no
[resumo gerado](artifacts/0006-webgl2-graphics-gate.md).

## Falha encontrada pelo próprio gate

A primeira execução real observou a perda nos dois Canvas, mas nenhuma
restauração. Ambos chegaram ao timeout de dez segundos.

Um repro determinístico mostrou que o WebView2 ignorava `restoreContext()`
quando ele era solicitado na mesma tarefa que ainda distribuía
`webglcontextlost` aos consumidores, inclusive o PixiJS. O probe passou a
aguardar a próxima tarefa do navegador antes de solicitar a restauração. Um
teste de regressão simula exatamente esse runtime e falha se a restauração for
antecipada.

A revisão posterior encontrou uma segunda borda: cancelamento depois da perda
podia abandonar o Canvas perdido até o timeout. A recuperação de emergência
agora também aguarda a próxima tarefa, tenta restaurar o contexto e preserva o
erro original. Outro teste reproduz o cancelamento nesse ponto.

## Limites da conclusão

O gate não:

- tenta alocar uma textura de `16.384 × 16.384 px`;
- induz falta de memória ou reinicialização do driver;
- mede o pico temporal de memória gráfica;
- descobre o orçamento global do driver;
- estabelece patamar de aprovação por tempo ou memória;
- usa esta única máquina para ranquear definitivamente as topologias.

Essas ações não eram necessárias para provar o comportamento do produto e uma
alocação sintética até falhar adicionaria risco sem representar o corpus real.
Repetições em outros equipamentos poderão ampliar a faixa observada sem mudar
o contrato deste gate.

## Conclusão

O critério está atendido. A implementação atual:

- confirma WebGL2 acelerado por hardware antes de abrir o editor;
- consulta limites no Canvas real e exercita uma textura do corpus;
- recupera os quatro contextos perdidos nas duas topologias sem reconstruir a
  árvore PixiJS;
- registra pressão de residência e uso gráfico com a natureza de cada medida
  explicitada;
- mantém Boas-vindas, Configurações e Diagnóstico acessíveis quando o editor
  não pode iniciar;
- bloqueia somente o editor e apresenta orientação clara ao usuário.

Não foi introduzido fallback Canvas 2D, renderização por software, política de
tiles ou alocação sintética de OOM.
