---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-31
updated: 2026-07-31
---

# Lote e OperationLease

## Pergunta

Este gate verifica se o primeiro corte produtivo do `BatchRunner` executa os
itens estritamente em série sob uma única instância contínua de
`OperationLease(BatchExclusive)`. A concessão global, a pausa do Cache e a
reserva do Processador precisam existir antes do primeiro item e permanecer
retidas até o terminal de todo o lote, sem liberação e nova aquisição no
intervalo entre itens.

A prova também precisa demonstrar que uma Exportação normal real recebe
conflito antes do primeiro item e entre dois itens, e que consegue executar
logo depois de sucesso, falha antes da Preparação, falha entre duas promoções
ou queda externa do processo proprietário.

Este não é um novo ensaio A/B. A globalidade do gate e a matriz terminal da
Exportação normal já foram exercitadas nas duas topologias em
[`0019`](0019-matriz-terminal-da-exportacao-normal.md). O `BatchRunner` não
possui `ProjectHost`, e o critério atual não exige repetir aquela matriz.

## Contrato implementado

- `BatchPlan` recebe itens já planejados, rejeita lote vazio, exige ao menos
  uma saída por item, mantém o Projeto consistente dentro do item e rejeita
  identificadores de requisição repetidos.
- `BatchRunner::run` adquire uma única vez o
  `OperationLease(BatchExclusive)`. `BatchRunner::execute` aceita somente uma
  referência a um lease já adquirido nesse modo.
- O loop serial inteiro usa a mesma instância do lease. Cada item possui seu
  próprio controle de execução, sem abrir uma janela de reentrada global.
- O chamador captura um único `RootBindingPlan` antes da aquisição e o entrega
  ao lote completo.
- Os fixtures carregam revisões persistidas por `ProjectCore`, sem abrir uma
  `ProjectSession`. Naquele gate, isso era evidência complementar e ainda não
  encerrava os critérios mais amplos das duas entradas de `ProjectCore`; o gate
  posterior [`0022`](0022-project-core-sessoes-e-revisoes-persistidas.md)
  fechou essa prova.
- `ExportPipeline::execute_group` prepara e verifica todas as saídas de um
  item antes de iniciar a Publicação, depois as promove serialmente.
- Uma falha de Publicação informa `promoted_outputs` e `total_outputs`, não
  emite `Completed`, não desfaz promoções anteriores e descarta preparações
  restantes por RAII quando isso é seguro.
- O caminho de uma única saída continua usando o mesmo pipeline, por delegação
  para a execução agrupada com um elemento.

O contrato reconcilia a redação anterior do critério: uma Exportação normal
adquire uma instância `NormalExport` por tentativa; o lote adquire uma única
instância `BatchExclusive` por tentativa completa. “Mesmo lease” significa o
mesmo mecanismo e o mesmo conjunto de três reservas, enquanto todos os itens
do lote compartilham também a mesma instância em tempo de execução.

## Instrumento

A evidência bruta está em
[`artifacts/0011-batch-operation-lease.json`](artifacts/0011-batch-operation-lease.json).
O script `Test-OperationGate.ps1 -Suite batch` executou, nesta ordem:

1. parser JSON fechado, incluindo rejeição de campos duplicados, inesperados,
   com capitalização incorreta e duplicados em objetos aninhados;
2. testes focados de `OperationLease`, `BatchRunner`, preparação agrupada,
   Publicação parcial e contrato do probe;
3. build `release` real do host e do Processador de Imagens;
4. quatro cenários com dois hosts Tauri independentes reais;
5. conferência de tamanhos e SHA-256 das saídas, ausência de staging e
   encerramento dos processos do ensaio.

A coleta usou o commit completo
`48a434eaac89d293e136ed95d9c60a0f0888c1ef`, Windows x64 e PowerShell
5.1. Os 204 inputs da build estavam limpos e produziram o digest
`bac0391b136f1f89c9df06885c8e864c52785426b3dee811f242746687cdf3a4`.
O artefato registra a árvore de trabalho geral como suja porque havia mudanças
fora desses inputs; isso não alterou os fontes, testes, scripts ou binários
incluídos no digest.

| Binário | Bytes | SHA-256 |
| --- | ---: | --- |
| `myalbuns-desktop.exe` | 14.267.392 | `661bd02256b38893c3a95b5e62823a3f1d1becf79c8f8950abcd5c371b3c35d0` |
| `myalbuns-imaging.exe` | 2.796.032 | `669276150685df5c53f2b68b9640de2510e960ae5a36313a9bbb694f7ce8d271` |

As 11 verificações passaram.

## Resultados

| Cenário | Forma do lote | Resultado observado |
| --- | --- | --- |
| Sucesso | 2 itens, 1 saída por item | 2 itens e 2 saídas concluídos em ordem serial |
| Antes da Preparação | 2 itens | item 0 concluído; item 1 falhou em `prepare_output`; primeira saída preservada e segunda ausente |
| Entre promoções | 1 item, 2 saídas | falha `publish_output` com 1 de 2 promoções; item e lote não concluídos |
| Queda do proprietário | 1 item | owner encerrado externamente com o lease retido e sem terminal cooperativo |

### Continuidade do lease

Em todos os cenários, `owner_ready` foi publicado somente depois da aquisição
dos três recursos e os registrou como `held`. Uma chamada real do comando de
Exportação normal no challenger recebeu `conflict` sem identificador de
operação, progresso ou cancelamento.

Nos dois cenários com dois itens, `between_items_ready` conservou o mesmo
identificador de operação de `owner_ready` e os três recursos ainda estavam
`held`. Uma segunda chamada real do challenger recebeu novamente `conflict`.
Somente depois dessa observação o segundo item foi liberado. Portanto não há
janela de liberação ou nova aquisição entre itens.

### Falhas tratáveis e Publicação limitada

Na falha anterior à Preparação, o primeiro item já havia publicado uma saída
de 39.164 bytes. O segundo item falhou ao entrar em sua preparação, não chegou
à Publicação e não criou a saída final.

Na falha entre promoções, o harness manteve a segunda saída aberta sem
compartilhamento no Windows. O pipeline preparou e verificou as duas saídas,
promoveu a primeira e falhou ao tentar promover a segunda:

| Saída | Antes | Depois |
| --- | --- | --- |
| primeira | 21 bytes — `68b0e44e1d446ebbd764286c6dd4872a52b7092d5938c02e2703444eba2a2031` | 39.164 bytes — `eb02749f7cc312f422a0e18febf5b1758eb56ac09dc5e7905d97281c51be59af` |
| segunda | 22 bytes — `b41460a0afe33af53d4f1598c3040a1388ff816bdbfeb5a690793e437c328de2` | os mesmos 22 bytes e o mesmo hash |

Essa mistura parcial é o envelope deliberado do
[`ADR 0006`](../adr/0006-publicar-exportacao-com-transacao-limitada.md), não
um sucesso incompleto. Não houve rollback do conjunto nem remoção da saída
anterior. O terminal informou uma de duas promoções, não emitiu `Completed` e
não deixou preparação temporária.

Depois de cada falha tratável, o próprio processo owner executou uma
Exportação normal produtiva. Ela readquiriu Gate, pausa do Cache e Processador
e publicou 39.164 bytes.

### Queda do proprietário

O runner encerrou externamente o processo owner enquanto `owner_ready`
mostrava os três recursos retidos e antes de liberar o primeiro item. Nenhum
`owner_terminal` foi sintetizado. O host challenger sobrevivente adquiriu o
gate global abandonado, suas próprias reservas locais e concluiu uma
Exportação normal real de 39.164 bytes. Não restou processo ou preparação do
ensaio.

## Limites da conclusão

- A execução usa somente a topologia `independent`; não repete a matriz A/B e
  não decide a topologia final.
- O probe roda em hosts Tauri reais, mas não exercita WebView, DOM ou interface
  de Exportação em lote.
- Não implementa descoberta recursiva, pré-validação completa, conflitos
  globais de destino, Tela de Problemas, Religação, ignorar Projeto nem
  revalidação da revisão.
- Não implementa checkpoint, retomada, recuperação após reinício ou
  cancelamento produtivo do lote.
- Não mede paralelismo, desempenho ou calibração. Os itens permanecem seriais.
- As múltiplas saídas PNG exercitam a fronteira transacional, não a matriz
  completa de formatos nem PDF.
- Não implementa limpeza de Saídas órfãs.
- Não implementa o guardião de abertura. O critério 37 foi encerrado depois,
  no escopo estreito da Sessão e da trava, em
  [`0021`](0021-operation-gate-e-bloqueio-de-abertura.md).
- Os fixtures persistidos e o DPI reduzido servem ao contrato operacional, não
  à avaliação de qualidade final.
- Esta evidência encerra somente o critério 36.

## Conclusão

A Exportação normal usa uma instância de `OperationLease(NormalExport)` por
tentativa. O lote usa uma única instância de
`OperationLease(BatchExclusive)` do início ao terminal, e todos os itens
executam sob a mesma concessão, pausa do Cache e reserva do Processador. Não
existe liberação entre itens.

Sucesso, falha na Preparação, falha após uma de duas promoções e morte do
proprietário não deixaram reserva presa e permitiram uma Exportação normal real
posterior. A Publicação parcial observada respeitou a transação limitada do
ADR 0006. O critério 36 está encerrado; o critério 37 foi encerrado
posteriormente em `0021`.

## Repetição

```powershell
powershell -NoProfile -ExecutionPolicy Bypass `
  -File scripts\Test-OperationGate.ps1 `
  -Suite batch `
  -OutputPath docs\research\artifacts\0011-batch-operation-lease.json
```
