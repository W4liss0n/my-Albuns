---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-31
updated: 2026-07-31
---

# Progresso e cancelamento da Exportação normal

## Pergunta

Este corte verifica se a Exportação normal pode conectar progresso e
cancelamento à Janela proprietária sem transferir o trabalho, o
`OperationLease` ou a exclusividade global para a interface e sem criar um
coordenador universal.

O objetivo é implementar a fronteira real da tentativa. Não é encerrar ainda
a matriz A/B de estados terminais, a Exportação em lote ou o guardião de
abertura.

## Contrato implementado

Cada chamada de `export_spike` recebe um `Channel<ExportEvent>` exclusivo. O
backend cria o identificador da operação, mantém a correlação dentro do
adaptador Tauri e envia à interface somente dois eventos públicos:

- `started`, depois que a tentativa venceu o `OperationGate`;
- `progress`, com estágio, unidades concluídas, total e disponibilidade de
  cancelamento.

O `ExportPort` não expõe Tauri nem o identificador da operação. Ele devolve um
`ExportAttempt` com uma única promessa terminal e uma operação `cancel()`
idempotente. Uma solicitação feita antes de `started` fica pendente no
adaptador e usa a correlação assim que ela chega; se a tentativa termina antes
disso, o resultado é `not_found`.

`ExportAttempts` possui somente o registro das tentativas ativas, sua Janela
proprietária e o estado de cancelamento. Ele não armazena trabalho, progresso,
Projeto, transporte ou lease. Outra Janela não pode cancelar a tentativa. A
destruição da Janela solicita o cancelamento de todas as tentativas que ela
possui por meio de um único observador instalado no host.

O `OperationLease` agora possui duas etapas:

1. `begin` resolve imediatamente o conflito global, antes de abrir o
   progresso;
2. `complete` aguarda a pausa do Cache e a reserva do Processador.

Depois de `started`, tanto a captura dos bindings quanto a conclusão do lease
disputam com o token de cancelamento. O descarte do futuro devolve qualquer
concessão ou pausa parcial.

## Fronteira de Publicação

Cancelamento e Publicação disputam uma única transição atômica:

- se o cancelamento vence, nenhum evento `publishing` é enviado, a preparação
  é descartada e a saída anterior permanece;
- se a Publicação vence, o evento `publishing` informa
  `cancellable: false`, o botão desaparece e uma solicitação posterior recebe
  `too_late`.

O `ExportPipeline` consulta novamente o token logo depois do callback dessa
fronteira. Isso impede que uma corrida já vencida pelo cancelamento publique o
arquivo preparado.

Se o host não consegue confirmar a terminação do Processador de Imagens, a
preparação continua preservada e o `ImagingProcessor` entra em quarentena. O
lease libera Cache e gate, mas uma nova reserva do Processador falha de forma
fechada e orienta reiniciar o aplicativo. Assim, uma segunda tentativa não
inicia outro sidecar enquanto o anterior talvez ainda esteja vivo.

## Interface

`ExportPreviewControl` é o único proprietário do ciclo visual da Exportação:

- o botão fica indisponível assim que a tentativa começa;
- nenhum modal aparece antes de `started`, inclusive diante de conflito
  global;
- o modal `Exportando` pertence somente à Janela do Projeto, apresenta uma
  barra geral e `X de Y`;
- `Cancelar exportação` aparece somente enquanto o backend declara a operação
  cancelável;
- solicitar cancelamento não fecha o modal; a interface aguarda o resultado
  terminal;
- sucesso fecha o progresso e mostra confirmação curta;
- cancelamento ou falha fecham o progresso e abrem um feedback separado com
  `Tentar novamente` e `Fechar`;
- mudança de Projeto ou desmontagem cancela a tentativa e ignora eventos
  obsoletos.

O controller geral do editor deixou de possuir estado, resultado ou comando
de Exportação. Ele recebe somente a informação transitória de que a interação
do Projeto está bloqueada, necessária para impedir Undo/Redo por atalho
enquanto o modal local está ativo.

## Evidência reproduzível

A implementação está no commit
`28cab9cae52cc82fcda59a027dfcce14aad35a18`.

Foram executados:

```powershell
npm run typecheck
npm test
npm run build
powershell -NoProfile -ExecutionPolicy Bypass `
  -File scripts\Test-RustQuality.ps1
powershell -NoProfile -ExecutionPolicy Bypass `
  -File scripts\Invoke-LocalCargo.ps1 test --workspace
```

Resultados:

- 24 arquivos de teste do frontend, com 118 testes aprovados;
- 158 testes Rust aprovados no workspace;
- seis testes Rust ignorados porque pertencem aos runners reais de caminhos,
  recuperação e `OperationGate`;
- contrato TypeScript, build Vite, `cargo fmt` e
  `cargo clippy --workspace --all-targets -- -D warnings` aprovados.

Os testes direcionados cobrem canal e erros tipados, correlação privada,
cancelamento idempotente antes e depois de `started`, propriedade por Janela,
destruição da proprietária, aquisição parcial, fronteira de Publicação,
quarentena do Processador, progresso acessível, retry, fechamento, foco e
eventos obsoletos.

## Limites da conclusão

- A implementação produtiva está conectada, mas este corte não automatiza uma
  Exportação real cancelada pela UI nas duas topologias.
- Sucesso, falha, cancelamento e queda do proprietário ainda não foram
  injetados conjuntamente em A e B com readquisição posterior de todos os
  recursos.
- O feedback de falha deste corte é separado do progresso e apresenta o
  motivo, mas a Tela de Problemas tabular completa pertence ao ticket de
  Exportação.
- `BatchRunner`, promoção de múltiplas saídas, `BatchExclusive` produtivo e
  Checkpoint continuam ausentes.
- Uma terminação não confirmada coloca o Processador em quarentena até o
  reinício do host; não foi introduzido um recuperador especulativo.
- O guardião de abertura e a diferença integrada entre `OperationGate` e
  Bloqueio de abertura continuam pendentes.

Por esses limites, os critérios 35, 36 e 37 do ticket 01 permanecem abertos.

## Conclusão

O progresso e o cancelamento agora atravessam a fronteira produtiva da
Exportação normal sem acoplar a UI ao Tauri, ao Processador ou ao lease. O gate
continua responsável apenas pela exclusividade, o lease continua responsável
apenas pela reserva ordenada e a tentativa continua responsável pelo seu
próprio progresso e cancelamento.

O próximo gate deve usar essa implementação para injetar os estados terminais
reais nas duas topologias; não há justificativa para antecipar o
`BatchRunner` ou o guardião dentro deste corte.
