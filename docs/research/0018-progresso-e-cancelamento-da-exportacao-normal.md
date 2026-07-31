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

- `started`, depois que a tentativa venceu o `OperationGate`, declarando se o
  cancelamento já está disponível;
- `progress`, com estágio, disponibilidade de cancelamento e unidades
  discriminadas entre `measured`, com concluídas e total confiáveis, ou
  `unmeasured`.

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

Cancelamento e Publicação disputam uma única transição atômica, reivindicada
diretamente pelo `ExportPipeline` antes de ele anunciar o estágio
`publishing`. O adaptador Tauri somente traduz o progresso já decidido:

- se o cancelamento vence, nenhum evento `publishing` é enviado, a preparação
  é descartada e a saída anterior permanece;
- se a Publicação vence, o evento `publishing` informa
  `cancellable: false`, o botão desaparece e uma solicitação posterior recebe
  `too_late`.

Assim, o callback de progresso permanece observacional e nenhum chamador
futuro do pipeline, inclusive o lote, precisa reproduzir a fronteira para
preservar a saída anterior.

Se o host não consegue confirmar a terminação do Processador de Imagens, a
preparação continua preservada e o `ImagingProcessor` entra em quarentena. O
lease libera Cache e gate, mas uma nova reserva do Processador falha de forma
fechada e orienta reiniciar o aplicativo. Assim, uma segunda tentativa não
inicia outro sidecar enquanto o anterior talvez ainda esteja vivo.

## Interface

`ExportPreviewControl` é o único proprietário do ciclo visual da Exportação:

- o botão fica indisponível assim que a tentativa começa;
- nenhum modal aparece antes de `started`, inclusive diante de conflito
  global; uma rejeição nesse intervalo produz somente um aviso não modal;
- o modal `Exportando` pertence somente à Janela do Projeto; começa com barra
  indeterminada e sem contagem, passando a uma barra geral com `X de Y`
  somente ao receber um total confiável;
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

A implementação inicial está no commit
`28cab9cae52cc82fcda59a027dfcce14aad35a18`; a revisão final dos contratos de
progresso, ciclo visual e fronteira de Publicação está no commit
`6dd685b9cac324a9832bbc88409db0a2a400bb93`.

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

- 24 arquivos de teste do frontend, com 119 testes aprovados;
- 157 testes Rust aprovados no workspace;
- seis testes Rust ignorados porque pertencem aos runners reais de caminhos,
  recuperação e `OperationGate`;
- contrato TypeScript, build Vite, `cargo fmt` e
  `cargo clippy --workspace --all-targets -- -D warnings` aprovados.

Os testes direcionados cobrem canal e erros tipados, correlação privada,
cancelamento idempotente antes e depois de `started`, propriedade por Janela,
destruição da proprietária, aquisição parcial, os dois vencedores da fronteira
de Publicação, progresso medido e não medido, conflito anterior a `started`,
quarentena do Processador, nova tentativa, fechamento, foco e eventos
obsoletos.

## Limites da conclusão

- A implementação produtiva está conectada e a matriz backend A/B foi
  concluída posteriormente em
  `docs/research/0019-matriz-terminal-da-exportacao-normal.md`. Essa evidência
  não constitui um teste ponta a ponta pela UI, pelo WebView e pelo modal.
- A indisponibilidade proativa das ações de Exportação nas outras Janelas
  ainda não foi implementada; uma corrida concorrente continua sendo rejeitada
  pelo gate antes de `started` e informada sem modal.
- O feedback de falha deste corte é separado do progresso e apresenta o
  motivo, mas a Tela de Problemas tabular completa pertence ao ticket de
  Exportação.
- `BatchRunner`, promoção de múltiplas saídas, `BatchExclusive` produtivo e
  Checkpoint continuam ausentes.
- Uma terminação não confirmada coloca o Processador em quarentena até o
  reinício do host; não foi introduzido um recuperador especulativo.
- O guardião de abertura e a diferença integrada entre `OperationGate` e
  Bloqueio de abertura continuam pendentes.

Com a evidência complementar do 0019, o critério 35 do ticket 01 está
encerrado. Os critérios 36 e 37 permanecem abertos.

## Conclusão

O progresso e o cancelamento agora atravessam a fronteira produtiva da
Exportação normal sem acoplar a UI ao Tauri, ao Processador ou ao lease. O gate
continua responsável apenas pela exclusividade, o lease continua responsável
apenas pela reserva ordenada e a tentativa continua responsável pelo seu
próprio progresso e cancelamento.

O gate seguinte usou essa mesma implementação para injetar os estados
terminais reais nas duas topologias e está documentado no 0019. Permanecem
separados um eventual ensaio ponta a ponta da UI e a indisponibilidade
proativa nas outras Janelas; nenhum dos dois justifica antecipar o
`BatchRunner` ou o guardião dentro deste corte.
