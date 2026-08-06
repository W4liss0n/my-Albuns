---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-31
updated: 2026-07-31
---

# Capabilities, permissions e scopes do frontend

## Objetivo

Este gate fecha a fronteira entre as Janelas de Projeto, a Tela de
Boas-vindas e o backend Tauri. Cada superfície deve chamar somente os comandos
próprios necessários ao corte atual, sem receber APIs genéricas do sistema
operacional.

A evidência canônica foi atualizada no commit `abc023d`, com os 223 inputs de
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

A lista das Janelas de Projeto fica em um único manifesto de permission. O
teste da fronteira Tauri extrai os `invoke` dos adapters e exige igualdade exata
com essa lista; a build do Tauri, por sua vez, exige que cada capability
referencie uma permission válida. O runner inspecionou o manifesto compilado e
encontrou exatamente as duas permissions próprias descritas nesta seção.

Os 15 comandos cobrem estado e edição do Projeto, preparação de previews,
Exportação, logging estruturado e os dois probes temporários da comparação de
topologias. `Channel` é parte do transporte de uma chamada permitida e não
exige conceder `core:event` à Janela.

A Tela de Boas-vindas usa outra capability local, `global-shell`, aplicada
somente à Janela `global`. Sua única permission própria é
`global-shell-logging`, cuja allow-list contém apenas `frontend_log`. Para o
gate de distribuição, ela também recebe somente a permission oficial
`dialog:allow-open`; `dialog:default`, salvar e mensagens não são concedidos.
Assim, materializar o processo global não concede à superfície acesso a
estado, edição, previews, Exportação normal ou probes das Janelas de Projeto.

O contrato Rust verifica separadamente as duas capabilities contra suas
permissions. O teste de fronteira do frontend mantém o entrypoint global fora
do editor e permite que somente os dois composition roots escolham adapters de
plataforma.

## Sistema de arquivos e shell

O frontend não possui dependência ou import de plugin genérico de filesystem
ou shell. Além da API central do Tauri, somente o adapter global do seletor
importa `@tauri-apps/plugin-dialog`. Também não existe `core:*`, `fs:*` ou
`shell:*` nas capabilities compiladas.

O protocolo de assets deixou de expor recursivamente todo o Cache. O WebView
pode ler somente as representações publicadas que a interface consome:

- `$LOCALDATA/MyAlbuns2/Cache/*/Media/*.jpg`;
- `$LOCALDATA/MyAlbuns2/Cache/*/Media/*.png`.

Índices, metadados, temporários, originais e Exportações ficam fora desse
scope.

O plugin de shell permanece no backend Rust exclusivamente para iniciar o
sidecar empacotado e fixo `myalbuns-imaging`. O runner verificou a dependência,
o registro do plugin no host e a chamada fixa do adapter; isso não concede ao
WebView permissão para escolher ou iniciar processos. O builder do processo
global não registra esse plugin. Inversamente, o plugin de diálogo é registrado
somente no builder global, não no host das Janelas de Projeto.

## Evidências

O runner executou sete checks:

| Check | Resultado |
| --- | --- |
| AST do runner PowerShell | passou |
| Contrato Rust da capability das Janelas de Projeto | passou |
| Contrato Rust da capability da Tela de Boas-vindas | passou |
| Contrato Rust do scope de assets | passou |
| Compilação da ACL pelo Tauri | passou |
| Fronteira Tauri no frontend | 7 testes passaram |
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
- As duas Janelas de Projeto ainda compartilham a mesma UI e permission. Se
  passarem a ter papéis diferentes, essa permission deverá ser dividida naquele
  momento; a Tela de Boas-vindas já está separada.
- `MyAlbuns2` continua sendo o namespace temporário já documentado; a árvore
  final permanece `MyAlbuns`.
- O plugin oficial inclui o caminho escolhido em seus scopes dinâmicos durante
  o processo. O frontend não recebe o plugin de filesystem, e a validação do
  arquivo continuará pertencendo ao `ProjectStore`; o filtro do seletor não
  seria uma fronteira de confiança.
- Este gate valida a ACL do diálogo, não sua janela real, o runtime WebView2,
  o instalador ou a máquina limpa. Esses itens permanecem no gate de
  distribuição.

## Conclusão

O critério permanece encerrado. As Janelas de Projeto e a Tela de Boas-vindas
têm capabilities locais, explícitas e separadas; nenhuma recebe filesystem ou
shell genérico. As Janelas de Projeto leem somente previews publicados, e a
Tela de Boas-vindas pode apenas abrir o seletor nativo.

## Repetição

```powershell
powershell -NoProfile -ExecutionPolicy Bypass `
  -File scripts\Test-FrontendSecurityGate.ps1
```
