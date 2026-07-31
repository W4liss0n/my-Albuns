---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-31
updated: 2026-07-31
---

# ProjectCore, Sessões e revisões persistidas

## Pergunta

Este gate verifica as duas garantias centrais do `ProjectCore`:

1. a Janela abre um único proprietário mutável do Projeto;
2. o lote carrega somente uma revisão persistida e imutável, sem receber
   comandos, Salvamento, Undo/Redo ou uma `ProjectSession`.

A prova precisa atravessar o `BatchRunner` real com dois Projetos e demonstrar
que o processo headless não constrói `ProjectHost`, não possui
`EditableProject` e não precisa de acesso de escrita ou exclusão aos arquivos
de entrada durante a execução.

## Contrato implementado

`myalbuns-core` publica um seam pequeno com duas entradas:

| Entrada | Resultado | Capacidades |
| --- | --- | --- |
| `ProjectCore::open_editable_session` | `EditableProject` | estado, intenção, Undo/Redo, snapshot e revisão para persistência |
| `ProjectCore::load_persisted_revision` | `LoadedProjectRevision` | somente revisão e `RenderSnapshot` |

`ProjectSession` permanece privada à crate. `EditableProject` é uma fachada
opaca e não clonável; ela conserva o registro da Identidade enquanto vive e o
remove por `Drop`. Um `ProjectCore` compartilhado pelo host rejeita uma segunda
abertura da mesma Identidade, permite outro Projeto e volta a admitir a
Identidade depois do fechamento. `ProjectHost` conserva esse mesmo
`ProjectCore` e também rejeita rótulos de Janela ou Identidades de Projeto
duplicados em sua composição.

`LoadedProjectRevision` contém o valor desserializado e validado, mas expõe
somente `revision()` e `render_snapshot()`. Ela não possui acesso a
`ProjectSession`, `apply`, `persisted_revision`, `confirm_saved_revision`,
`undo` ou `redo`.

`BatchItem` deixou de aceitar `Vec<ExportPlan>` arbitrário. A única construção
é `BatchItem::from_persisted_revision`, que recebe uma
`LoadedProjectRevision`, cria o snapshot internamente e produz os planos de
saída. Assim, o chamador do lote não consegue substituir a revisão salva pelo
snapshot de uma Sessão editável usando a API do `BatchRunner`.

## Provas de propriedade

Os testes do núcleo cobrem três casos observáveis:

- a primeira abertura editável é aceita, uma segunda abertura da mesma
  Identidade no mesmo `ProjectCore` recebe
  `EditableSessionAlreadyOpen`, outro Projeto continua independente e o
  fechamento libera a Identidade;
- uma Sessão aberta na revisão 0 recebe uma alteração ainda não salva e passa
  à revisão 1, enquanto `load_persisted_revision` sobre a fonte salva continua
  devolvendo a revisão 0;
- alterar uma cópia do `RenderSnapshot` não altera a revisão carregada.

O teste do host acrescenta uma defesa na borda: mesmo handles provenientes de
registros diferentes não podem compor duas Sessões com a mesma Identidade no
mesmo `ProjectHost`.

## Instrumento

A evidência bruta está em
[`artifacts/0013-project-core-session-revision.json`](artifacts/0013-project-core-session-revision.json).
O runner `Test-ProjectCoreGate.ps1` executou:

1. validação sintática do runner e parsers JSON de schema fechado;
2. testes focados do seam público, do proprietário único, da entrada do lote e
   do contrato do probe;
3. build `release` real do host Tauri;
4. criação de dois Projetos persistidos;
5. execução headless iniciada antes da construção de qualquer `ProjectHost`;
6. bloqueio, pelo harness do Windows, de abertura para escrita e exclusão dos
   dois inputs durante todo o lote;
7. correlação entre revisões carregadas, itens, pedidos de renderização e
   saídas publicadas;
8. comparação de tamanho e SHA-256 dos inputs antes e depois.

A coleta usou o commit
`5430e08a213151f6b153d8496e82ac18e0f916af`, Windows x64 e PowerShell 5.1.
Os 209 inputs da build estavam limpos e produziram o digest
`cb6223f48d010abe18be904eaec0229aa5f8f0d16cb62d78450e70e2811beade`.
A árvore geral estava marcada como suja somente por arquivos fora desses
inputs. O executável `release` possuía 14.619.136 bytes e SHA-256
`a25d29a9fc5705064f13fe545808353e5d2a8613bade54dd0ae190041e713ffb`.
As nove verificações passaram.

## Resultados

O processo publicou primeiro o estado `ready` com:

- modo `headless_before_project_host`;
- papel `global`;
- `projectHostConstructed: false`;
- `editableProjectOwned: false`.

Depois da liberação pelo runner, o mesmo processo carregou e concluiu os dois
itens:

| Projeto | Revisão | Input antes/depois | Saída publicada |
| --- | ---: | --- | --- |
| `project-spike-001` | 0 | 3.323 bytes — `7dcaf041eb4a585539c97e9ab645a9cfd5c3c968603509d9c33915404c9c64c5` | 34 bytes — `98a24f8b73fec0a3e2e779bc0473d456a8f6958690621fd1408e742e2e7ba864` |
| `project-spike-002` | 0 | 3.320 bytes — `d9ccdb79272538ae2f6d9bbc5b4a14a0fa18cfb0c013d959ebfd11f0c81e019d` | 34 bytes — `e4f720d8994b61891dcf327ff8472f6b68da9827d3a0410d6898674abab9c4fc` |

Nos dois inputs, a tentativa de abrir para escrita e a tentativa de excluir
falharam enquanto o lote estava ativo. Tamanho e hash permaneceram idênticos.
O terminal registrou duas revisões carregadas, dois itens concluídos, duas
saídas publicadas e um único evento `BatchEvent::Completed`. O tipo de entrada
declarado foi `loaded_project_revision`, e os indicadores de `ProjectHost` e
`EditableProject` permaneceram falsos.

As saídas pequenas são cargas determinísticas do transporte in-process do
probe. Elas atravessam o `BatchRunner`, o `ExportPipeline`, a preparação, a
verificação e a Publicação reais, mas este gate não repete a rasterização real
do Processador já coberta pelos gates anteriores.

## Revisão estrutural

A revisão formal por padrões e por especificação não encontrou violação de
padrão documentado. Um teste foi fortalecido para abrir a Sessão, criar uma
alteração não salva e só então carregar a mesma fonte persistida; a coleta
oficial já inclui essa ordem.

Dois pontos avaliados foram mantidos fora deste gate:

- o registro de `ProjectCore` garante unicidade dentro do proprietário
  compartilhado pelo host; entre hosts e processos, `ProjectFileLock` continua
  sendo a proteção final demonstrada em
  [`0021`](0021-operation-gate-e-bloqueio-de-abertura.md), enquanto identidade
  física, aliases e foco/reuso pertencem ao ticket 13;
- reabrir cada caminho e revalidar hash/revisão imediatamente antes de criar o
  snapshot pertence à pré-validação e execução produtiva do lote no ticket 31.

Os runners de evidência permanecem autocontidos. Extrair agora um framework
genérico entre gates com schemas e ciclos diferentes aumentaria o acoplamento
sem remover uma regra de produto; essa extração só deve ser reconsiderada se
uma nova repetição real do mesmo ciclo aparecer.

## Limites da conclusão

- O bloqueio de escrita observado é uma garantia concreta desta execução no
  Windows, não uma sandbox universal para código futuro.
- O gate não implementa o guardião completo de abertura, identidade física,
  foco ou reuso de Janela; esses pontos continuam no ticket 13.
- O gate não implementa descoberta, Religação, pré-validação final,
  checkpoint, retomada ou revalidação imediatamente anterior a cada item;
  esses pontos continuam no ticket 31.
- O probe não abre WebView nem testa a interface do lote.
- O transporte determinístico não mede codecs, qualidade de imagem ou
  desempenho do Processador.

## Conclusão

Os critérios 16 e 17 do ticket 01 estão encerrados. `ProjectCore` possui duas
entradas públicas com capacidades diferentes; `ProjectSession` permanece
interna; o host conserva um proprietário editável por Identidade; e o
`BatchRunner` só aceita uma revisão persistida imutável.

O lote headless processou dois Projetos sem construir `ProjectHost` ou possuir
`EditableProject`. Durante a execução, os arquivos de entrada recusaram escrita
e exclusão e permaneceram byte a byte idênticos, enquanto duas saídas foram
publicadas e correlacionadas às revisões salvas.

## Repetição

```powershell
powershell -NoProfile -ExecutionPolicy Bypass `
  -File scripts\Test-ProjectCoreGate.ps1 `
  -OutputPath docs\research\artifacts\0013-project-core-session-revision.json
```
