---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-31
updated: 2026-07-31
---

# OperationGate e Bloqueio de abertura

## Pergunta

Este gate verifica se duas exclusividades com ciclos diferentes permanecem
separadas:

- `OperationGate` pertence a uma tentativa de operação e pode ser liberado sem
  encerrar a Sessão do Projeto;
- o Bloqueio de abertura pertence à Sessão editável, continua retido entre
  tentativas e termina no fechamento normal dessa Sessão;
- se o processo proprietário cair enquanto possui os dois mecanismos, outro
  processo só pode recuperá-los depois que o proprietário realmente morrer.

Os dois recursos são vinculados ao processo e não persistem após sua morte. A
diferença provada aqui é o escopo durante a vida normal do processo: tentativa
para o Gate, Sessão para a trava do arquivo.

## Contrato implementado

`ProjectFileLock` encapsula a primitiva nativa do Windows em
`myalbuns-paths`. A aquisição é exclusiva e imediata, devolve conflito tipado,
não altera os bytes do Projeto e libera a região por `Drop`; o Windows também
fecha o handle quando o processo termina.

A trava ocupa um byte reservado em deslocamento alto, fora do conteúdo
suportado do Projeto. Assim, o arquivo persistido continua disponível para
leituras comuns enquanto todas as instâncias editáveis disputam a mesma região
nativa. O módulo não decide identidade física, foco, política de conflito ou
duração da posse.

O spike introduz `ProjectOpeningSession`, um escopo estreito que:

1. adquire `ProjectFileLock`;
2. lê uma revisão persistida válida;
3. abre uma `ProjectSession` por `ProjectCore::open_editable_session`;
4. mantém Sessão e trava no mesmo objeto;
5. encerra a Sessão antes de liberar a trava no fechamento normal.

Esse tipo prova o encaixe entre os ciclos sem antecipar o guardião completo de
abertura.

## Instrumento

A evidência bruta está em
[`artifacts/0012-operation-gate-project-lock.json`](artifacts/0012-operation-gate-project-lock.json).
O runner `Test-OperationGate.ps1 -Suite project_open` usa a suíte coesa
`Test-ProjectOpenGate.ps1` e executou:

1. parser JSON fechado, incluindo rejeição de campos duplicados, inesperados e
   com capitalização divergente;
2. testes focados de `ProjectFileLock`, `OperationGate`, `OperationLease` e do
   contrato do probe;
3. build `release` real do host e do Processador de Imagens;
4. fechamento normal e queda do proprietário com dois hosts Tauri
   independentes reais;
5. comparação de tamanho e SHA-256 do arquivo antes e depois de cada cenário;
6. encerramento dos processos criados pelo ensaio.

A coleta usou o commit
`cb874058b6ffe46c6327f24584846200c7dcdbf7`, Windows x64 e PowerShell 5.1.
Os 207 inputs da build estavam limpos e produziram o digest
`2e495dc9c712147b45f60897bc00e58a616f4b3e34376aeacd6a1004ecf4e389`.
A árvore geral permaneceu marcada como suja somente por mudanças fora desses
inputs.

| Binário | Bytes | SHA-256 |
| --- | ---: | --- |
| `myalbuns-desktop.exe` | 14.336.000 | `a17b4e8a194b1637f801198144b299fbc60d70ac1386b9be15f48e076a6450ed` |
| `myalbuns-imaging.exe` | 2.796.032 | `4474f5895909484c3b56835c9f1643e713d1538c3b9882d35f0865835ab9a240` |

As oito verificações passaram.

## Resultados

Os dois cenários começaram com owner e challenger em processos distintos. O
owner abriu pelo `ProjectCore` o mesmo Projeto v3 que manteve bloqueado. O
challenger recebeu conflito tipado tanto no `OperationGate` quanto no
`ProjectFileLock`.

Em seguida, o owner liberou somente seu `OperationLease`. O challenger adquiriu
uma concessão `NormalExport`, mas continuou recebendo conflito na trava do
Projeto. Depois que o challenger devolveu o Gate, o owner readquiriu a
concessão sem recriar sua Sessão nem seu Bloqueio de abertura. Essa sequência é
a observação direta de que os mecanismos não foram fundidos.

| Terminal do owner | Observação | Duração total do cenário |
| --- | --- | --- |
| Fechamento normal | `ProjectOpeningSession` foi encerrada; o processo owner permaneceu vivo e o challenger recuperou Sessão editável e Gate | 3.034 ms |
| Queda externa | não houve fechamento cooperativo; o owner morreu possuindo Sessão, trava e Gate, e o challenger só os recuperou após a morte confirmada | 3.282 ms |

No cenário normal, o evento `owner_session_closed` registrou Gate e trava como
`released`, e o owner continuou vivo depois da recuperação. No cenário de
queda, esse evento permaneceu ausente; o runner confirmou a terminação externa
antes de liberar o challenger.

O fixture possuía 372 bytes. Antes e depois dos dois cenários, seu SHA-256 foi
`1efb9ac2eca59a8194766440765bff331324731574d06dedcb46f3fc248d7b7b`.
Portanto, adquirir, liberar e abandonar os mecanismos não alterou a revisão
persistida.

## Limites da conclusão

- A execução usa dois hosts independentes e não repete a matriz A/B; a
  globalidade do `OperationGate` já foi exercitada nas duas topologias em
  [`0019`](0019-matriz-terminal-da-exportacao-normal.md).
- O gate prova a primitiva e o escopo `ProjectOpeningSession`, mas não
  implementa o guardião completo, foco/reuso de Sessão, entrada pelo Explorer,
  interface ou mensagens ao usuário.
- Não integra ainda identidade física `Same`/`Different`/`Indeterminate`,
  aliases de unidade mapeada/UNC ou Identidade persistida; esses pontos
  continuam no fluxo de abertura do ticket 13.
- Não exercita Salvamento, substituição atômica do arquivo nem transferência da
  trava para um novo handle. O `ProjectStore` e o guardião deverão coordenar
  essa transição sem abrir uma janela editável concorrente.
- A trava de região exige que todas as instâncias do MyAlbuns disputem a mesma
  faixa reservada. O gate não afirma exclusividade contra programas externos
  que ignorem esse protocolo.
- Não há matriz SMB/WAN/DFS, teste de acesso negado, recuperação persistente de
  órfãos ou medição de desempenho.

## Conclusão

`OperationGate` e Bloqueio de abertura permanecem mecanismos distintos. Uma
tentativa pode devolver o Gate enquanto a Sessão editável conserva a trava do
Projeto; outra tentativa pode usar o Gate nesse intervalo sem abrir uma
segunda Sessão editável. O fechamento normal devolve a trava com o processo
ainda vivo. Na queda, o Windows só permite a recuperação depois que o processo
proprietário deixa de existir.

O critério 37 do ticket 01 está encerrado. O guardião de abertura completo e
sua integração com identidade, foco e Salvamento continuam fora deste gate.

## Repetição

```powershell
powershell -NoProfile -ExecutionPolicy Bypass `
  -File scripts\Test-OperationGate.ps1 `
  -Suite project_open `
  -OutputPath docs\research\artifacts\0012-operation-gate-project-lock.json
```
