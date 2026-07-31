---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-31
updated: 2026-07-31
---

# Gate operacional e OperationLease

## Pergunta

Este corte verifica se operações que compartilham recursos caros podem usar
uma exclusividade realmente comum às Janelas do aplicativo sem criar um
coordenador universal:

- uma segunda operação concorrente recebe conflito imediato, sem entrar em
  fila;
- a mesma fronteira funciona entre hosts independentes e entre duas Janelas
  do mesmo host;
- concessão global, pausa do Cache e reserva do Processador são adquiridas por
  um único `OperationLease`;
- sucesso, falha, cancelamento da aquisição e queda do processo proprietário
  não deixam a concessão presa;
- `CacheEngine`, `ImagingProcessor`, `ExportPipeline` e `OperationGate`
  continuam com responsabilidades distintas;
- progresso e cancelamento permanecem pertencendo à tentativa, não ao gate.

## Contrato implementado

`OperationGate` usa um mutex nomeado do Windows. Seu nome é derivado da raiz
local de `AppPaths`, portanto hosts e Janelas que pertencem ao mesmo namespace
de dados na mesma sessão do Windows disputam a mesma concessão. A espera usa
tempo zero: outra proprietária produz `OperationGateError::Conflict` e não
forma fila.

Os modos `NormalExport`, `BatchExclusive` e `CacheMaintenance` compartilham a
mesma fronteira pequena de exclusão. Os três modos e toda a matriz de conflitos
estão representados, mas somente o fluxo real de Exportação normal participa
deste corte.

A aquisição do `OperationLease` segue uma única ordem:

1. concessão do `OperationGate`;
2. pausa do `CacheEngine`, depois que trabalhos já ativos alcançam o ponto
   seguro;
3. reserva exclusiva do `ImagingProcessor`.

O descarte libera na ordem inversa: Processador, Cache e gate. Guardas RAII
também devolvem os recursos já obtidos se o futuro for cancelado durante uma
aquisição parcial ou se a tentativa falhar por unwind.

O comando de Exportação normal agora constrói o lease antes de executar o
`ExportPipeline` e conserva o lease até o estado terminal. O transporte Tauri
do Processador exige uma referência à reserva, impedindo que esse caminho
produtivo invoque o sidecar sem a reserva correspondente. O trabalho normal do
Cache usa sua própria guarda de atividade e a mesma reserva do Processador,
sem adquirir uma concessão de Exportação.

O mutex é adquirido e liberado na mesma thread nativa. Se o processo morre, o
Windows abandona a concessão e o próximo processo pode recuperá-la. O gate não
armazena seleção, progresso, cancelamento, jobs de Cache ou estado criativo.

## Instrumento

A execução canônica está em
[`artifacts/0009-operation-gate-lease.json`](artifacts/0009-operation-gate-lease.json).
Ela foi coletada no commit
`bbf646045ed304284a921e3a0651ca9ed7d2e6f1`, no Windows
`10.0.26200.0` x64, com Windows PowerShell Desktop
`5.1.26100.8875`.

Os 196 inputs da build tinham o digest
`0b69b56dddd3be7f1540f2926659e2070fa86090cced0c5fa087d137df22a7c0`
antes e depois da execução. Tanto `sourceInputsDirty` quanto
`buildInputsDirty` terminaram falsos. O checkout continha alterações alheias à
build, registradas separadamente no artefato.

O runner:

- executa testes focados do gate, do lease, da pausa do Cache, da reserva do
  Processador e do contrato do probe;
- mata um processo real enquanto ele possui um `OperationLease` completo;
- exercita separadamente sucesso, falha, cancelamento e progresso do
  `ExportPipeline`;
- compila o desktop real em perfil `release`;
- abre dois hosts Tauri independentes para a alternativa A;
- abre duas Janelas reais no mesmo host Tauri para a alternativa B;
- valida eventos JSON fechados, inclusive chaves duplicadas equivalentes por
  escape Unicode, além de PIDs, topologia, Janela e modo;
- confirma no fim que os inputs não mudaram e que nenhum processo iniciado
  pelo ensaio ficou ativo.

Todos os 15 checks passaram. O executável medido tinha 13.596.160 bytes e
SHA-256
`29b259b382613aa302e6571264d3820b6935ef87001e4fda1b2ae20b718bdf1a`.

## Resultados

### Exclusividade nas duas topologias

| Evidência | A — hosts independentes | B — host multiwindow |
| --- | ---: | ---: |
| Processos do desktop | 2 | 1 |
| Janelas participantes | 2 | 2 |
| Duração do cenário | 2.708 ms | 842 ms |
| Proprietária adquiriu o lease | sim | sim |
| Desafiante recebeu conflito tipado | sim | sim |
| Proprietária liberou | sim | sim |
| Desafiante adquiriu depois | sim | sim |

Na alternativa A, os PIDs `27552` e `9928` disputaram a mesma concessão. Na B,
as Janelas `main` e `project-b` disputaram o mesmo gate dentro do PID `4860`.
Nos dois casos, o desafiante foi recusado imediatamente enquanto a
proprietária mantinha o lease e adquiriu um novo lease completo depois da
liberação.

### Ciclo de vida do lease

Os testes cobrem a matriz dos três modos, espera por trabalho ativo do Cache,
bloqueio da reserva do Processador, liberação conjunta, unwind e cancelamento
do futuro enquanto ele aguarda o Cache ou o Processador. Depois de cada
injeção, outra tentativa adquire normalmente gate, pausa e reserva.

No teste entre processos, o filho possui um `OperationLease` completo com seus
recursos locais. Enquanto ele vive, outro processo recebe conflito. Depois da
terminação forçada, o sucessor adquire outro lease completo. Isso comprova a
recuperação da concessão global abandonada; as guardas locais de Cache e
Processador desaparecem com o processo e não são recursos compartilhados entre
hosts.

### Estados terminais da Exportação

Os testes do `ExportPipeline` continuam comprovando sucesso com progresso,
falha, cancelamento antes do início e cancelamento em voo. A Exportação normal
do desktop usa o novo lease e conserva a reserva até o fim da execução.

Essas duas evidências são complementares, mas não equivalem ainda a um
cancelamento iniciado pela interface. Na coleta do artefato `0009`, o comando
ainda criava um token local sem entrada externa. A implementação posterior
desse contrato está registrada em
[`0018-progresso-e-cancelamento-da-exportacao-normal.md`](0018-progresso-e-cancelamento-da-exportacao-normal.md);
ela não altera retroativamente o alcance da execução A/B deste documento.

## Limites da conclusão

- Os cenários A/B exercitam conflito, liberação normal e nova aquisição de
  `NormalExport`; eles não injetam falha, cancelamento, progresso ou queda
  dentro de cada uma das duas topologias.
- `BatchRunner` ainda não existe. Não foi demonstrado que cada item do lote
  adquire o mesmo lease, nem falha entre duas promoções de saída.
- `BatchExclusive` e `CacheMaintenance` possuem contrato e conflitos, mas
  ainda não têm fluxos produtivos conectados.
- O artefato `0009` não exercita a entrada de cancelamento nem a Janela de
  progresso conectadas posteriormente; a matriz terminal A/B continua aberta.
- O guardião de abertura e a focalização de uma `ProjectSession` existente não
  pertencem a esta coleta. A diferença entre o gate operacional e a trava de
  arquivo foi demonstrada posteriormente, no escopo estreito de uma Sessão, em
  [`0021`](0021-operation-gate-e-bloqueio-de-abertura.md); o guardião completo
  permanece fora dos dois cortes.
- No momento desta coleta, os critérios 35, 36 e 37 permaneciam abertos. Eles
  foram encerrados, respectivamente, em `0019`, `0020` e `0021`.

## Conclusão

O corte estabelece as fronteiras estruturais necessárias sem antecipar as
operações que ainda não existem. O gate possui somente exclusividade global,
o lease possui somente ordem e garantia de liberação, e cada recurso continua
com seu próprio estado. A Exportação normal já atravessa o caminho único de
reserva. A conexão posterior do cancelamento e do progresso está documentada
em `0018`, a matriz terminal A/B em `0019`, o lote em `0020` e a separação do
Bloqueio de abertura em `0021`. Manutenção total e o guardião de abertura
completo continuam como gates explícitos.

## Repetição

Em Windows:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass `
  -File scripts\Test-OperationGate.ps1 `
  -OutputPath docs\research\artifacts\0009-operation-gate-lease.json
```
