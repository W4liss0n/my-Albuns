---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-31
updated: 2026-07-31
---

# Caminhos, identidade e planos distribuídos

## Objetivo

Este gate fecha três garantias que ainda estavam separadas nas evidências
anteriores da fase 1:

1. cada tentativa captura um `RootBindingPlan` e transmite exatamente esse
   plano ao host e ao Processador de Imagens nas topologias A e B;
2. as dependências de plataforma permanecem atrás da interface do produto,
   preservando as Known Folders, os caminhos longos e o namespace final;
3. aliases físicos do mesmo Projeto reutilizam uma única sessão, enquanto
   identidade inconclusiva falha de forma fechada e a trava nativa continua
   sendo a proteção final.

As evidências canônicas foram coletadas no commit
`15bd09c39730e4c350df067e1c895d0f7eb49175`, com os inputs de código e build
limpos:

- [`artifacts/0014-windows-path-identity-gate.json`](artifacts/0014-windows-path-identity-gate.json);
- [`artifacts/0015-root-binding-topology.json`](artifacts/0015-root-binding-topology.json).

## Plano único por tentativa

`OperationPathContext` continua sendo o único proprietário da captura mutável.
Ao iniciar a tentativa, a fronteira de comando congela o contexto como um
`RootBindingPlan`. O protocolo do Processador transporta o plano completo; o
host o usa na preparação e envia a mesma representação ao processo auxiliar.
Nenhum dos dois participantes volta a resolver uma raiz já capturada.

O gate de topologias executou 27 checks. Todos passaram. Quatro operações
foram correlacionadas:

| Topologia | Papel | Captura nova | Mesmo plano no host e Processador | Mesmo PID do Processador até o terminal |
| --- | --- | --- | --- | --- |
| A — hosts independentes | proprietário | sim | sim | sim |
| A — hosts independentes | sucessor | sim | sim | sim |
| B — host multiwindow | proprietário | sim | sim | sim |
| B — host multiwindow | sucessor | sim | sim | sim |

Cada correlação exige exatamente um evento de captura pelo proprietário, um
spawn pelo host, um início e uma conclusão pelo Processador, todos com o mesmo
identificador de operação, digest do plano e PIDs compatíveis. O digest é
somente uma evidência diagnóstica do conteúdo: duas tentativas podem produzir o
mesmo digest quando as raízes não mudaram. A existência de uma captura nova é
provada pelo evento pertencente à nova tentativa. No ensaio de remapeamento, em
que a raiz operacional mudou, a nova tentativa produziu também outro digest.

## Fronteira das bibliotecas

As bibliotecas avaliadas não se tornam contratos do produto:

| Biblioteca | Decisão | Responsabilidade permitida |
| --- | --- | --- |
| `directories` | usada atrás de `AppPaths` | descobrir `data_dir`/`%APPDATA%` e `data_local_dir`/`%LOCALAPPDATA%` |
| `windows-sys` | usada no adaptador nativo privado | handles, identidade física, unidade mapeada, caminhos longos e `ReOpenFile` |
| `same-file` | não selecionada como dependência direta ou contrato | poderia auxiliar comparações simples, mas não define o resultado ternário nem o binding de unidade mapeada |
| `dunce` | não selecionada como dependência direta ou contrato | simplificação textual não representa identidade física ou raiz operacional |

O produto continua dono dos nomes das pastas e de sua política. `MyAlbuns` é o
sufixo final normativo sob `%APPDATA%` e `%LOCALAPPDATA%`. `MyAlbuns2` permanece
explicitamente temporário até a conclusão integral do programa, apenas para
não misturar os dados desta reconstrução com os de uma versão antiga. A
exceção temporária não altera a árvore final.

O desktop e o Processador carregam manifesto `longPathAware`. A leitura de um
objeto resolvido usa o mesmo handle físico por `ReOpenFile`, inclusive se o
pathname for substituído depois da resolução. Capturas capazes de alcançar a
rede continuam fora da thread da interface.

## Guardião de abertura

`ProjectOpeningGuard` compõe um único `ProjectCore`, o registro de sessões, a
comparação física ternária e `ProjectFileLock`:

- unidade mapeada e alias UNC do mesmo arquivo resultam em foco da sessão já
  aberta, sem criar outra sessão mutável;
- outro objeto físico com a mesma Identidade persistida permanece como Cópia
  externa pendente, sem assumir automaticamente uma nova Identidade;
- resultado `Indeterminate` falha de forma fechada e não altera o registro;
- a trava nativa permanece viva junto com a sessão e é revalidada antes da
  leitura final do documento.

O gate Windows executou 13 checks reais em Windows x64. Ele cobriu SMB por
compartilhamento administrativo loopback, alias mapeado/UNC, conflito entre
processos, remapeamento, falha recuperável, nova captura explícita, staging no
Destino UNC, round-trip de caminhos nativos e manifests de caminhos longos.

Durante a coleta, uma retenção transitória do redirector SMB expôs uma corrida
no instrumento, antes do código de produção. O runner passou a serializar o
uso da unidade mapeada, validar a desconexão e repetir renames somente para os
erros transitórios 5 e 32, com prazo finito. `AccessDenied` continua distinto
de `Unavailable` no contrato do produto.

## Limites preservados

- Este gate não implementa a interface de escolha/foco de Janela, detecção de
  órfãos, recuperação de sessão ou todo o ticket 13.
- `ProjectFileLock` protege o objeto físico atualmente aberto. Quando o futuro
  `ProjectStore` substituir o arquivo por Salvamento atômico, os tickets 02 e
  13 deverão definir a transferência ou readquisição da trava sem intervalo
  desprotegido. Esta prova não antecipa essa política.
- A Identidade de uma Cópia externa continua pendente; persistir uma nova
  Identidade e montar seu namespace próprio pertence ao ticket 02.
- O SMB loopback prova a semântica do Windows nesta máquina, não cobre WAN,
  DFS, outras credenciais, DNS instável ou todos os provedores de rede.
- O gate não escolhe entre as topologias A e B; ele apenas demonstra que o
  contrato de caminhos e identidade funciona nas duas.

## Conclusão

Os três critérios podem ser encerrados. A aplicação possui uma captura de
caminhos por tentativa, propagada sem resolução paralela; uma fronteira de
plataforma substituível e compatível com a árvore final; e um guardião que
reúne aliases físicos, falha fechada e trava nativa sem confundir essas
responsabilidades com `OperationGate`.

## Repetição

```powershell
powershell -NoProfile -ExecutionPolicy Bypass `
  -File scripts\Test-WindowsPathGate.ps1 `
  -OutputPath docs\research\artifacts\0014-windows-path-identity-gate.json

powershell -NoProfile -ExecutionPolicy Bypass `
  -File scripts\Test-OperationGate.ps1 `
  -Suite normal `
  -OutputPath docs\research\artifacts\0015-root-binding-topology.json
```
