---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-31
updated: 2026-08-12
---

# Caminhos Windows, identidade física e UNC

## Pergunta

Este gate verifica se a fundação de caminhos do aplicativo consegue operar no
Windows sem tratar a forma textual de um caminho como identidade:

- aceitar caminhos locais, UNC, unidades mapeadas e formas Verbatim suportadas;
- preservar a escolha lógica do usuário, mas congelar uma raiz operacional para
  toda a tentativa;
- impedir que um remapeamento redirecione trabalho já iniciado;
- diferenciar ausência, indisponibilidade, acesso negado e evidência de
  identidade inconclusiva;
- comparar objetos existentes por handles;
- transportar o mesmo plano ao host e ao Processador sem conversão textual com
  perda;
- manter Cache local e staging de Exportação no Destino;
- retirar da thread da interface qualquer captura que possa alcançar a rede.

## Contrato exercitado

`OperationPathContext` pertence ao planejador. Ele captura cada raiz lógica uma
única vez e produz um `RootBindingPlan` imutável. Uma unidade mapeada é
consultada por `WNetGetUniversalNameW`; o plano conserva a forma lógica
escolhida pelo usuário e grava separadamente a raiz UNC operacional. Os
participantes recebem somente o plano congelado. Raiz não capturada produz
`UnboundRoot`, e outra tentativa precisa criar explicitamente outro contexto.

Os caminhos do plano usam representação nativa opaca. No Windows, a
serialização do protocolo 17 transporta unidades UTF-16, inclusive uma
sequência com surrogate não pareado exercitada pelo teste de round-trip.

Objetos existentes são abertos por handle. A resolução confirma o tipo esperado
e a identidade física usa `FILE_ID_INFO`, combinando o identificador do volume
e o identificador do arquivo. A comparação retorna somente `Same`, `Different`
ou `Indeterminate`; falha de acesso ou disponibilidade não vira diferença.

O desktop e `MyAlbuns.Imaging.exe` incorporam o mesmo manifesto
`longPathAware`. A captura de bindings chamada pelas fronteiras Tauri passa por
`spawn_blocking`, separada da thread assíncrona que atende a interface.

## Instrumento

A execução canônica mais recente está em
[`artifacts/0008-windows-path-gate.json`](artifacts/0008-windows-path-gate.json).
O JSON é a única fonte do `gitCommit`, dos PIDs, dos hashes e dos tempos daquela
rodada. A coleta descrita na primeira versão desta pesquisa foi substituída por
essa rodada posterior; elas não são apresentadas como a mesma execução.

O campo `sourceInputsDirty` usa a mesma regra do gate de recuperação: inspeciona
todo arquivo rastreado e todo arquivo novo não ignorado pelo Git, excluindo
somente a própria evidência gerada. Isso inclui entradas da raiz, configurações
de build e `resources/windows/myalbuns.manifest`; outputs declarados no
`.gitignore` não são tratados como fontes. O commit e o estado são capturados
antes e depois da jornada; mudança de HEAD ou sujeira em qualquer extremidade
torna a proveniência suja.

O runner cria duas raízes descartáveis no volume local e as expõe por SMB real
através do compartilhamento administrativo loopback `C$`. Uma letra livre é
mapeada para a primeira raiz e depois remapeada para a segunda. O cenário
compila desktop e Processador em targets isolados, confirma previamente que
ambos usam o protocolo 17 e extrai seus manifests.

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
`VerbatimDisk` da mesma unidade congelou a raiz UNC correta. Depois da falha,
uma nova captura explícita gerou outro digest e passou a observar o binding
atual; não houve repetição automática da ação final.

No ensaio do sidecar, o mapeamento foi removido antes do despacho. O Processador
consumiu o caminho operacional congelado, publicou a saída no UNC e removeu o
staging. Em seguida, a raiz operacional foi temporariamente retirada do ar: a
tentativa falhou em `Prepare`, não iniciou o Processador e não publicou arquivo.
Somente após restauração e nova captura explícita a Exportação foi publicada.

### Identidade e bloqueio

Os handles do caminho local, da unidade mapeada e do alias UNC produziram
`Same`. Uma raiz indisponível produziu `Indeterminate`.

Uma trava de arquivo foi mantida por um processo e consultada por outros
processos através do alias UNC. A segunda aquisição encontrou conflito, uma
leitura real continuou permitida e outra aquisição só teve sucesso depois da
queda do proprietário. Isso prova a equivalência física dos aliases no ensaio;
o guardião de Sessão do Projeto permanece uma responsabilidade separada.

### Exportação, Cache e caminhos longos

O staging ficou dentro do próprio Destino UNC e a publicação final foi
confirmada por SHA-256. O Cache permaneceu sob a Known Folder local temporária
`%LOCALAPPDATA%\MyAlbuns2\Cache`, mesmo com raízes externas. Um arquivo local
não ASCII com caminho superior a 260 caracteres foi aberto e publicado pelos
handles esperados. Os dois executáveis anunciaram `longPathAware`.

Os valores exatos de planos, output, PID e duração estão apenas no JSON
canônico, evitando misturar resultados de execuções diferentes.

## Limites da conclusão

- o compartilhamento `C$` exercita SMB real e aliases do mesmo volume, mas não
  representa WAN, DFS, troca de servidor, DNS instável ou latência de rede;
- rede indisponível foi exercitada de ponta a ponta; acesso negado real não foi
  reproduzido em servidor com outra credencial;
- o gate prova identidade e trava, mas não implementa promoção de Identidade,
  Cópia externa ou Religação;
- `OperationGate` e `OperationLease` permanecem distintos da trava de arquivo e
  são exercitados por outros gates;
- `MyAlbuns2` continua sendo um namespace temporário para não misturar dados da
  versão anterior;
- o plano transporta raízes sem perda; isso não autoriza persistir observações
  físicas ou bindings no Projeto.

## Conclusão

A interface de caminhos é viável para o corte atual. O trabalho distribuído usa
uma captura imutável, não segue remapeamentos posteriores, preserva resultados
inconclusivos, mantém operações potencialmente remotas fora da thread da
interface e permite falha recuperável com nova tentativa explícita.

## Repetição

Em Windows, com o compartilhamento administrativo local disponível:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass `
  -File scripts\Test-WindowsPathGate.ps1 `
  -OutputPath docs\research\artifacts\0008-windows-path-gate.json
```

O runner substitui o artefato somente depois que os 11 checks e as relações de
evidência passam.
