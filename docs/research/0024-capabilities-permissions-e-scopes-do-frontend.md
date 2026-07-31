---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-31
updated: 2026-07-31
---

# Capabilities, permissions e scopes do frontend

## Objetivo

Este gate fecha a fronteira entre as Janelas de Projeto e o backend Tauri. A
interface deve chamar somente os comandos próprios necessários ao corte atual,
sem receber APIs genéricas do sistema operacional.

A evidência canônica foi coletada no commit `099c0cc`, com os 212 inputs de
código e build limpos:

- [`artifacts/0016-frontend-security-gate.json`](artifacts/0016-frontend-security-gate.json).

## Capability local e fechada

A capability `default` é explicitamente local, não declara origem remota e se
aplica somente às Janelas `main` e `project-b`. As duas recebem a mesma
capability porque, neste spike, ambas carregam o mesmo `index.html` e usam os
mesmos adapters de plataforma.

`core:default` foi removida. Em seu lugar, a capability concede somente
`project-window-commands`, uma permission própria cuja allow-list contém os 15
comandos realmente invocados pelo frontend. O manifesto da aplicação torna a
ACL fail-closed: registrar um novo handler não o expõe até que a permission seja
alterada explicitamente.

A lista fica em um único manifesto de permission. O teste da fronteira Tauri
extrai os `invoke` dos adapters e exige igualdade exata com essa lista; a build
do Tauri, por sua vez, exige que a capability referencie uma permission válida.
O manifesto compilado foi inspecionado pelo runner e contém somente essa
permission própria.

Os 15 comandos cobrem estado e edição do Projeto, preparação de previews,
Exportação, logging estruturado e os dois probes temporários da comparação de
topologias. `Channel` é parte do transporte de uma chamada permitida e não
exige conceder `core:event` à Janela.

## Sistema de arquivos e shell

O frontend não possui dependência ou import de plugin de filesystem ou shell.
Também não existe `core:*`, `fs:*` ou `shell:*` na capability compilada.

O protocolo de assets deixou de expor recursivamente todo o Cache. O WebView
pode ler somente as representações publicadas que a interface consome:

- `$LOCALDATA/MyAlbuns2/Cache/*/Media/*.jpg`;
- `$LOCALDATA/MyAlbuns2/Cache/*/Media/*.png`.

Índices, metadados, temporários, originais e Exportações ficam fora desse
scope.

O plugin de shell permanece no backend Rust exclusivamente para iniciar o
sidecar empacotado e fixo `myalbuns-imaging`. O runner verificou a dependência,
o registro do plugin no host e a chamada fixa do adapter; isso não concede ao
WebView permissão para escolher ou iniciar processos.

## Evidências

O runner executou seis checks:

| Check | Resultado |
| --- | --- |
| AST do runner PowerShell | passou |
| Contrato Rust da capability | passou |
| Contrato Rust do scope de assets | passou |
| Compilação da ACL pelo Tauri | passou |
| Fronteira Tauri no frontend | 4 testes passaram |
| Build de produção do frontend | passou |

Além dos testes-fonte, o runner comparou a capability e a permission escritas
com `capabilities.json` e `acl-manifests.json` emitidos pelo `tauri-build`. O
artefato registra `sourceInputsDirty: false`; a árvore de trabalho mais ampla
continuava contendo arquivos alheios aos inputs do gate, por isso
`workingTreeDirty` permanece verdadeiro sem invalidar o digest coletado.

## Limites preservados

- Os sete comandos dos probes de topologia continuam temporários ao spike. A
  build final deverá removê-los ou separá-los quando esses modos deixarem de
  existir.
- As duas Janelas ainda compartilham a mesma UI. Se passarem a ter papéis
  diferentes, a permission deverá ser dividida por capability naquele momento.
- `MyAlbuns2` continua sendo o namespace temporário já documentado; a árvore
  final permanece `MyAlbuns`.
- Este gate não valida runtime WebView2, diálogo nativo, instalador nem máquina
  limpa. Esses itens permanecem no próximo critério da fase.

## Conclusão

O critério pode ser encerrado. A interface tem uma ACL local, explícita e
mínima para o corte executável atual; não recebe filesystem ou shell genérico,
e o único acesso direto a arquivo foi estreitado aos previews publicados.

## Repetição

```powershell
powershell -NoProfile -ExecutionPolicy Bypass `
  -File scripts\Test-FrontendSecurityGate.ps1
```
