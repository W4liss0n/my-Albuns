---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-31
updated: 2026-07-31
---

# Candidato do processo global de Boas-vindas

## Pergunta

O mesmo `MyAlbuns.exe` consegue materializar uma Tela de Boas-vindas real nas
duas alternativas do spike sem iniciar a carga de trabalho de um Projeto e sem
antecipar a escolha entre hosts independentes e host multiwindow?

Este gate responde somente se o processo global continua sendo um candidato
viável. Ele não aplica orçamento numérico, não ranqueia A e B e não atualiza a
decisão de topologia do ADR 0005.

## Corte arquitetural

O papel global continua sendo selecionado no início de `MyAlbuns.exe`, antes de
construir `TopologySpike`, `ProjectHost`, `ProjectSession`, `CacheEngine` ou
`ImagingProcessor`. O processo:

1. resolve `AppPaths` e seu diretório próprio de dados da WebView;
2. inicializa logging local;
3. reserva o endpoint de singleton e status do spike;
4. inicia esse servidor tipado em uma thread nomeada;
5. constrói somente a Janela Tauri `global`, apontada para `global.html`.

A interface possui um entrypoint separado:

```text
global.html
  └─ src/global/main.tsx
       ├─ GlobalShell
       ├─ Logger → tauriLogger → frontend_log
       └─ ProjectFileDialog → tauriProjectFileDialog → dialog:allow-open
```

`GlobalShell` recebe `Logger` e o port estreito `ProjectFileDialog`. O
composition root escolhe os adapters Tauri; a superfície não importa `App`,
PixiJS, componentes do editor, domínio, ports da Sessão de Projeto nem bridges
dos probes de topologia. O build multipágina mantém `global.html` e
`index.html` como entradas distintas.

Não foi criado `GlobalCoordinator`, command bus, store genérico de Projetos
recentes ou port para controlar hosts. Depois deste gate, `Abrir Projeto`
recebeu somente o port estreito `ProjectFileDialog` para validar a integração
nativa de distribuição; seleção e cancelamento não iniciam Sessão nem decidem
o formato do arquivo. `Novo Projeto`, `Exportação em lote`, `Configurações` e
`Ajuda` permanecem desabilitados. Seus casos de uso pertencem aos tickets de
produto correspondentes.

## Fronteira de segurança

A Janela `global` recebe a capability local `global-shell`. Sua única
permission própria, `global-shell-logging`, libera somente `frontend_log`.
O gate posterior de distribuição acrescentou `dialog:allow-open`, sem liberar
salvar, mensagens, filesystem ou shell. Ela não recebe nenhum dos 15 comandos
das Janelas de Projeto e não recebe `core:*`, `fs:*` ou `shell:*`.

O artefato atualizado
[0016-frontend-security-gate.json](artifacts/0016-frontend-security-gate.json)
comparou as duas capabilities escritas com `capabilities.json` e
`acl-manifests.json` compilados pelo Tauri. Os sete checks passaram com os 220
inputs limpos no commit `40649bc`.

## Build da superfície global

O build de produção produziu `global.html` com três referências diretas:

| Asset | Bytes |
| --- | ---: |
| Entry JavaScript global | 1.396 |
| Chunk compartilhado de React/Tauri | 194.889 |
| CSS global | 2.400 |
| **Total direto** | **198.685** |

`global.html` não referenciou nenhum asset `project-*`. Essa observação não é
tratada isoladamente como prova de todo o grafo: o contrato de fonte também
falha se o entrypoint global passar a importar PixiJS, editor, domínio ou ports
do Projeto.

## Método de execução

O runner `scripts/Test-GlobalShellGate.ps1` usou um target Cargo isolado,
reconstruiu o perfil `release` e executou o mesmo binário duas vezes, primeiro
com a identidade `independent` e depois com `multiwindow`. Em cada rodada ele:

1. criou diretório de log e `runId` exclusivos;
2. exigiu simultaneamente o evento `welcome_screen_ready` e a Janela visível
   intitulada `MyAlbuns`;
3. consultou o status tipado, correlacionando PID, topologia, `runId` e probe;
4. iniciou uma duplicata e exigiu código `73` sem deslocar a proprietária;
5. mediu a árvore inteira, incluindo subprocessos WebView2;
6. exigiu zero processo `myalbuns-imaging`;
7. encerrou e confirmou a saída da árvore capturada antes da rodada seguinte.

A evidência canônica está em
[0017-global-shell-candidate.json](artifacts/0017-global-shell-candidate.json).
Ela pertence ao commit `d200628`, com 220 inputs limpos, digest
`25481f3a3e96773c68ef1060828dbb26c2dfb8433c8091f5882d97d8b3048589`
e executável SHA-256
`ba8ad10424637fdae303cc998b7270f1ed9c84f4d65371b4a469dec1313f615f`.

A máquina executava Windows 11 Pro `10.0.26200`, Intel Core i5-13450HX,
24.260,7 MiB de memória física, NVIDIA GeForce RTX 3050 6GB Laptop GPU e Intel
UHD Graphics.

## Resultados

| Evidência | A — identidade independent | B — identidade multiwindow |
| --- | ---: | ---: |
| Welcome pronta | 1.650 ms | 487 ms |
| Processos na árvore | 7 | 7 |
| Working set agregado | 372,25 MiB | 358,41 MiB |
| Memória privada agregada | 181,59 MiB | 180,71 MiB |
| Threads agregadas | 160 | 158 |
| Handles agregados | 3.230 | 3.224 |
| Duplicata rejeitada | código `73` | código `73` |
| Proprietária preservada | sim | sim |
| Processadores de Imagens | 0 | 0 |

Em ambas as alternativas, a árvore contém o host `myalbuns-desktop` e seis
subprocessos `msedgewebview2`. A memória privada do processo raiz foi 10,41
MiB em A e 10,42 MiB em B; a maior parte do custo agregado pertence ao runtime
da WebView.

A diferença de prontidão não é comparação de topologia. A execução A abriu a
primeira WebView da sequência e B reutilizou o runtime aquecido. O artefato
registra `ranking: null`, `recommendation: null` e
`numericBudgetApplied: false` exatamente para impedir essa leitura.

## Interpretação

O critério de candidato está atendido:

- as duas identidades do spike executaram a mesma Tela de Boas-vindas real;
- a superfície possui entrypoint, capability e permission próprios;
- nenhuma Sessão, Canvas ou integração do editor é alcançável pelo grafo de
  imports do entrypoint global;
- nenhum Processador de Imagens foi iniciado;
- singleton e status continuaram válidos após a introdução da WebView;
- o gate não deixou processo MyAlbuns ou WebView2 pertencente à rodada.

“Leve” significa aqui que o processo global não inicializa a carga de trabalho
de Projeto e que seu entrypoint próprio é pequeno. A árvore real ainda carrega
o custo base do WebView2, aproximadamente 181 MiB privados nesta máquina. A
aceitação de orçamento e a comparação com o custo total das alternativas
continuam reservadas à execução final, cujos critérios precisam ser congelados
antes da medição.

## Limites preservados

- O seletor nativo só confirma seleção ou cancelamento neste corte; ele ainda
  não abre Projeto, valida formato ou atualiza recentes. As demais ações não
  executam casos de uso do produto.
- O gate não exercita a convivência funcional da Tela de Boas-vindas com
  Janelas de Projeto abertas.
- O candidato usa o mesmo binário completo do aplicativo; não foi produzido ou
  aprovado um executável fisicamente mínimo.
- O TCP loopback continua sendo transporte observacional do spike, não IPC
  normativa.
- A rodada usa uma máquina e uma ordem fixa; não serve como benchmark A/B.
- Este artefato `0017` não valida WebView2 Evergreen instalado, diálogo nativo,
  instalador `win-x64` ou máquina limpa; esses itens pertencem ao gate posterior
  de distribuição.
- Não há watchdog, eleição, reinício automático ou coordenador universal.
- A topologia, seus riscos e seu custo de implementação ainda não foram
  recomendados; o ADR 0005 permanece proposto.

## Conclusão

`MyAlbuns.exe` permanece candidato ao processo global nas duas alternativas,
agora com uma Tela de Boas-vindas materializada e isolada do editor. Isso fecha
somente esse critério. A decisão A/B continua explicitamente aberta até os
gates terminais de ambiente, comparação congelada e recomendação do ADR.

## Repetição

```powershell
powershell -NoProfile -ExecutionPolicy Bypass `
  -File scripts\Test-GlobalShellGate.ps1

powershell -NoProfile -ExecutionPolicy Bypass `
  -File scripts\Test-FrontendSecurityGate.ps1
```

O primeiro build `release` em um target vazio pode levar mais de 20 minutos;
execuções incrementais ainda recompilam e vinculam o host antes de medir.
