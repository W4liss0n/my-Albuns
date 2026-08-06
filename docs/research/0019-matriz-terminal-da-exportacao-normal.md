---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-31
updated: 2026-07-31
---

# Matriz terminal da Exportação normal

## Pergunta

Este corte verifica se a Exportação normal conserva a separação entre
`CacheEngine`, `ExportPipeline` e `OperationGate` quando duas Janelas disputam
a operação e a proprietária termina por sucesso, falha, cancelamento ou queda
do processo. A verificação precisa valer nas duas topologias candidatas e
demonstrar, depois de cada terminal, uma nova Exportação real.

O objetivo é encerrar a matriz deixada aberta pelos documentos
[`0017`](0017-gate-operacional-e-operation-lease.md) e
[`0018`](0018-progresso-e-cancelamento-da-exportacao-normal.md). Exportação em
lote, promoção de múltiplas saídas e guardião de abertura continuam sendo
gates distintos.

## Contrato exercitado

A alternativa A usa dois hosts independentes: o Projeto Horizonte participa
como owner e o Projeto Aurora como challenger, ambos pela Janela `main` de seu
próprio processo. A alternativa B usa um host multiwindow: `main` é o owner e
`project-b` é o challenger.

Cada owner chama o comando produtivo `export_spike` com um
`Channel<ExportEvent>` real. Ao observar `started` e o primeiro progresso
`preparing`, o probe registra que a tentativa possui `operation_gate`,
`cache_pause` e `processor_reservation` e mantém a tentativa nessa barreira.
Enquanto os três recursos estão retidos, o challenger chama o mesmo
`export_spike`. O único resultado aceito é o erro tipado `conflict`, sem
`started`, progresso, identificador de operação ou cancelamento anterior.

Os terminais tratáveis também atravessam o caminho produtivo:

- sucesso exige a sequência completa de progresso, uma saída PNG regular e
  não vazia e o resultado `success`;
- falha é injetada retirando temporariamente o executável verificado do
  Processador de Imagens e só é aceita como `failed` se a mensagem identificar
  o Processador;
- cancelamento chama `cancel_export_spike` para a operação observada, exige a
  disposição `requested` e aguarda o resultado `cancelled`;
- queda usa encerramento externo obrigatório do processo enquanto o owner
  ainda possui o lease. Esse cenário não produz `owner_terminal`.

Depois de sucesso, falha ou cancelamento, e antes de publicar
`owner_terminal`, o próprio host do owner readquire um `OperationLease`
completo e o solta. Essa prova local readquire conjuntamente Gate, pausa do
Cache e reserva do Processador; portanto o evento `released` não depende
somente de o outro host conseguir avançar. Em seguida, o challenger liberado
executa outro `export_spike` real, readquire os três recursos e precisa
publicar uma saída válida.

Na queda da alternativa A, o runner encerra somente o processo owner e o host
challenger sobrevivente torna-se o successor. Na alternativa B, a queda do
host retira as duas Janelas; o runner inicia outro host para `project-b` e só
então autoriza a tentativa sucessora. O processo morto não coopera com a
liberação e não escreve um terminal sintético.

Essas regras foram implementadas em `export_terminal_probe.rs` e orquestradas
pelo runner histórico `Test-OperationGate.ps1`. Os dois foram retirados do
produto depois da escolha da topologia; consulte o
[encerramento do harness da Fase 1](0030-encerramento-do-harness-da-fase-1.md).

## Instrumento

A execução canônica está em
[`artifacts/0010-export-terminal-matrix.json`](artifacts/0010-export-terminal-matrix.json).
O artefato usa `schemaVersion: 2` e foi coletado no commit
`1657a2b460b098ecedfb8b7fbc13d6756cc2e9e8`, no Windows
`10.0.26200.0` x64, com Windows PowerShell Desktop `5.1.26100.8875`.

Os 26 checks passaram. Eles abrangem parsers JSON fechados, testes focados de
Gate, lease, Cache, Processador, pipeline e probes, a build `release` real, o
gate básico nas duas topologias, os oito casos da matriz terminal e a
restauração byte a byte do sidecar depois das injeções de falha.

| Evidência da coleta | Valor |
| --- | ---: |
| Checks aprovados | 26 de 26 |
| `sourceInputsDirty` | `false` |
| `build.buildInputsDirty` | `false` |
| Inputs da build | 201 |
| Digest SHA-256 dos inputs | `0a1f1cb51577a4e4791ee021224403e93ae6829008e2baa9415f095b62abeb21` |
| Duração da build, com `cargoBuildJobs: 1` | 1.067.962 ms |
| Desktop `release` | 13.877.248 bytes |
| SHA-256 do desktop | `661895d5d156177a8d1e02c17383abfc55da1b59bb3137d7406aff6fec406480` |
| Processador de Imagens `release` | 2.796.032 bytes |
| SHA-256 do Processador | `669276150685df5c53f2b68b9640de2510e960ae5a36313a9bbb694f7ce8d271` |

A árvore de trabalho completa foi registrada como alterada, mas os inputs
medidos permaneceram limpos e conservaram a mesma contagem e o mesmo digest
antes da build e depois dos probes. O artefato fica, assim, vinculado ao
commit e aos dois binários identificados acima, sem atribuir a eles mudanças
alheias à build.

## Matriz A/B

Em todos os oito casos, `owner_ready` possui o estágio `preparing` e os três
recursos em estado `held`. O conflito subsequente possui terminal `conflict`,
estado `blocked`, listas de progresso e recursos vazias e nenhum identificador
de operação, cancelamento ou saída.

Todos os successors terminaram em `success`, com os recursos em
`reacquired`, saída de 39.164 bytes e esta sequência exata de progresso:
`preparing`, `loading_sources`, três ocorrências de `composing`, duas de
`encoding_output`, `verifying`, `publishing` e `completed`.

### A — hosts independentes

| Cenário | PID owner → successor | Terminal do owner | Prova antes do successor | Successor | Duração |
| --- | ---: | --- | --- | --- | ---: |
| Sucesso | 6028 → 32188 | `success`, 39.164 bytes | reaquisição local completa; `released` | `success`, `reacquired`, 39.164 bytes | 7.231 ms |
| Falha | 29692 → 33524 | `failed`, sem saída | reaquisição local completa; `released` | `success`, `reacquired`, 39.164 bytes | 4.969 ms |
| Cancelamento | 24652 → 32224 | `cancelled`, `requested`, sem saída | reaquisição local completa; `released` | `success`, `reacquired`, 39.164 bytes | 3.246 ms |
| Queda do owner | 37636 → 14448 | ausente; `ownerTerminated: true` | encerramento externo do PID 37636 | `success`, `reacquired`, 39.164 bytes | 3.032 ms |

### B — host multiwindow

| Cenário | PID owner → successor | Terminal do owner | Prova antes do successor | Successor | Duração |
| --- | ---: | --- | --- | --- | ---: |
| Sucesso | 36248 → 36248 | `success`, 39.164 bytes | reaquisição local completa; `released` | `success`, `reacquired`, 39.164 bytes | 995 ms |
| Falha | 32268 → 32268 | `failed`, sem saída | reaquisição local completa; `released` | `success`, `reacquired`, 39.164 bytes | 4.811 ms |
| Cancelamento | 12844 → 12844 | `cancelled`, `requested`, sem saída | reaquisição local completa; `released` | `success`, `reacquired`, 39.164 bytes | 1.937 ms |
| Queda do owner | 13160 → 33492 | ausente; `ownerTerminated: true` | encerramento externo do PID 13160 e novo host | `success`, `reacquired`, 39.164 bytes | 3.698 ms |

Nos sucessos do owner, a sequência e o tamanho da saída coincidem com os do
successor. Nos cenários de falha e cancelamento, o owner não ultrapassa
`preparing` e não publica saída. Na queda, a ausência de `owner_terminal` é
parte do contrato, não uma lacuna preenchida por inferência.

## Limites da conclusão

- O runner usa Janelas Tauri reais e um `Channel` real, mas chama a fronteira
  Rust a partir do probe. Ele não dirige WebView, DOM, modal de progresso ou o
  clique de cancelamento da interface; portanto não é um teste E2E da UI.
- A falha exercitada é uma indisponibilidade confirmada do executável do
  Processador. Uma terminação que não possa ser confirmada coloca o
  Processador em quarentena e, deliberadamente, não permite reaquisição; esse
  caso fail-closed não pertence à matriz de recuperação imediata.
- `BatchRunner`, `BatchExclusive` produtivo e promoções múltiplas não
  participaram desta execução. Esses contratos foram cobertos posteriormente
  em [`0020`](0020-lote-e-operation-lease.md); esse vínculo não amplia
  retroativamente a matriz A/B deste documento. Checkpoint e retomada continuam
  fora dos dois cortes.
- O guardião de abertura e a focalização de uma `ProjectSession` existente
  permanecem fora deste corte. A distinção entre `OperationGate` e Bloqueio de
  abertura foi coberta posteriormente em
  [`0021`](0021-operation-gate-e-bloqueio-de-abertura.md), sem ampliar
  retroativamente esta matriz A/B.
- A coleta foi feita em uma única máquina Windows. Ela valida os estados e a
  liberação nas duas topologias implementadas, não compara diversidade de
  hardware nem decide a topologia final.

## Conclusão

A matriz comprova, nas topologias A e B, que a Exportação normal mantém Gate,
Cache e Processador separados e sob um lease único durante a tentativa. O
challenger é recusado pelo comando real antes de `started`; sucesso, falha e
cancelamento devolvem os três recursos e permitem reaquisição local pelo
próprio owner; a queda externa abandona a concessão sem cooperação; e todos os
oito cenários permitem uma Exportação sucessora real de 39.164 bytes.

Esta evidência encerra somente o critério 35 do ticket 01. No momento desta
coleta, os critérios 36 e 37 permaneciam abertos. O critério 36 foi encerrado
posteriormente em [`0020`](0020-lote-e-operation-lease.md), e o critério 37 em
[`0021`](0021-operation-gate-e-bloqueio-de-abertura.md). O guardião de abertura
completo continua fora desses cortes.

## Repetição

Em Windows:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass `
  -File scripts\Test-OperationGate.ps1 `
  -OutputPath docs\research\artifacts\0010-export-terminal-matrix.json
```
