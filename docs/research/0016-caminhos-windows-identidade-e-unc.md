---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-31
updated: 2026-07-31
---

# Caminhos Windows, identidade física e UNC

## Pergunta

Este gate verifica se a fundação de caminhos do aplicativo consegue operar no
Windows sem tratar a forma textual de um caminho como identidade:

- aceitar caminhos locais, UNC, unidades mapeadas e as formas Verbatim
  suportadas;
- preservar a escolha lógica do usuário, mas congelar uma raiz operacional
  para toda a tentativa;
- impedir que um remapeamento redirecione trabalho já iniciado;
- diferenciar ausência, indisponibilidade, acesso negado e evidência de
  identidade inconclusiva;
- comparar objetos existentes por handles;
- transportar o mesmo plano ao host e ao Processador de Imagens sem conversão
  textual com perda;
- manter Cache local e staging de Exportação no Destino;
- retirar da thread da interface qualquer captura que possa alcançar a rede.

## Contrato exercitado

`OperationPathContext` pertence ao planejador. Ele captura cada raiz lógica
uma única vez e produz um `RootBindingPlan` imutável. Uma unidade mapeada é
consultada por `WNetGetUniversalNameW`; o plano conserva a forma lógica
escolhida pelo usuário e grava separadamente a raiz UNC operacional. Os
participantes recebem somente o plano congelado. Raiz não capturada produz
`UnboundRoot`, e outra tentativa precisa criar explicitamente outro contexto.

Os caminhos do plano usam representação nativa opaca. No Windows, a
serialização do protocolo 9 transporta unidades UTF-16, inclusive uma sequência
com surrogate não pareado exercitada pelo teste de round-trip. A escolha do
formato persistido do Projeto continua reservada ao ticket 02.

Objetos existentes são abertos por handle. A resolução confirma o tipo
esperado e a identidade física usa `FILE_ID_INFO`, combinando o identificador
do volume e o identificador do arquivo. A comparação retorna somente `Same`,
`Different` ou `Indeterminate`; falha de acesso ou de disponibilidade não é
convertida em diferença.

O desktop e `MyAlbuns.Imaging.exe` incorporam o mesmo manifesto
`longPathAware`. A captura de bindings chamada pelas fronteiras Tauri passa por
`spawn_blocking`, separada da thread assíncrona que atende a interface.

## Instrumento

A execução canônica está em
[`artifacts/0008-windows-path-gate.json`](artifacts/0008-windows-path-gate.json).
Ela foi coletada no commit
`dba3a07098bf73c6babf32241ce6b6f69c9d8d0a`, com
`sourceInputsDirty: false`, no Windows `10.0.26200.0` x64.

O runner cria duas raízes descartáveis no volume local e as expõe por SMB real
através do compartilhamento administrativo loopback `C$`. Uma letra de unidade
livre é mapeada para a primeira raiz, depois remapeada para a segunda durante o
ensaio. O cenário também compila o desktop e o Processador em um target
isolado, confirma previamente que ambos usam o protocolo 9 e extrai seus
manifests.

Os 11 checks passaram:

1. contrato de resolução e identidade;
2. política de `AppPaths`, Cache, staging e formas de caminho;
3. UNC e unidade mapeada reais;
4. protocolo compartilhado;
5. build do Processador;
6. preflight da versão do Processador;
7. build do host desktop;
8. captura fora da thread chamadora;
9. Exportação real pelo Processador com plano UNC congelado;
10. manifesto de caminhos longos do desktop;
11. manifesto de caminhos longos do Processador.

## Resultados

### Binding, remapeamento e nova tentativa

O primeiro plano transformou a raiz operacional da unidade mapeada em UNC e a
reutilizou depois que a letra passou a apontar para outra pasta. A forma
`VerbatimDisk` da mesma unidade também congelou a raiz UNC correta. Depois da
falha, uma nova captura explícita gerou outro digest e passou a observar o
binding atual; não houve repetição automática da ação final.

No ensaio do sidecar, o mapeamento foi removido antes do despacho. O
Processador consumiu o caminho operacional congelado, publicou a saída no UNC
e removeu o staging. Em seguida, a raiz operacional foi temporariamente
retirada do ar: a tentativa falhou em `Prepare`, não iniciou o Processador e
não publicou arquivo. Somente após restauração e nova captura explícita a
Exportação foi publicada.

### Identidade e bloqueio

Os handles do caminho local, da unidade mapeada e do alias UNC produziram
`Same`. Uma raiz indisponível produziu `Indeterminate`.

Uma trava de arquivo foi mantida por um processo e consultada por outros
processos através do alias UNC. A segunda aquisição encontrou conflito, uma
leitura real do conteúdo continuou permitida e outra aquisição só teve sucesso
depois da queda do proprietário. Isso prova o mecanismo final de bloqueio do
arquivo e a equivalência dos aliases para esse ensaio; não implementa ainda o
guardião que focaliza uma sessão já aberta.

### Exportação, Cache e caminhos longos

O staging ficou dentro do próprio Destino UNC e a publicação final foi
confirmada por SHA-256. O Cache permaneceu sob a Known Folder local temporária
`%LOCALAPPDATA%\MyAlbuns2\Cache`, mesmo com raízes externas. Um arquivo local
não ASCII com caminho superior a 260 caracteres foi aberto e publicado pelos
handles esperados. Os dois executáveis anunciaram `longPathAware`.

## Limites da conclusão

- O compartilhamento `C$` exercita SMB real e aliases do mesmo volume, mas não
  representa WAN, DFS, troca de servidor, DNS instável ou latência de rede.
- Rede indisponível foi exercitada de ponta a ponta; acesso negado real não foi
  reproduzido em um servidor com outra credencial.
- O gate prova identidade e trava, mas ainda não implementa o guardião de
  abertura, a focalização de uma única `ProjectSession` nem a política
  fail-closed para `Indeterminate`.
- O mesmo cenário não foi repetido separadamente nas topologias A e B.
- `OperationGate` e `OperationLease` não participam deste ensaio e continuam
  distintos da trava de arquivo.
- `MyAlbuns2` continua sendo um namespace temporário para não misturar dados da
  versão anterior. A árvore final e a migração permanecem abertas.
- O plano transporta suas raízes sem perda; isso não decide a codificação
  persistida de todos os caminhos no formato final do Projeto.

## Conclusão

A interface de caminhos é viável para o corte da fase 1. O trabalho distribuído
usa uma captura imutável, não segue remapeamentos posteriores, preserva
resultados inconclusivos, mantém operações potencialmente remotas fora da
thread da interface e permite falha recuperável com nova tentativa explícita.
Os limites acima permanecem gates próprios e não são inferidos por esta prova.

## Repetição

Em Windows, com o compartilhamento administrativo local disponível:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass `
  -File scripts\Test-WindowsPathGate.ps1 `
  -OutputPath docs\research\artifacts\0008-windows-path-gate.json
```
