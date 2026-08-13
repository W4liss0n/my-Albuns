---
status: current
document: technical-research
ticket: 48-lifecycle-do-vite-no-handoff-global-project-host
date: 2026-08-13
updated: 2026-08-13
---

# Lifecycle de desenvolvimento multiprocesso

## Pergunta

Quem deve possuir o servidor Vite quando o processo Global termina depois de
entregar um Projeto a um Host independente, mas a WebView desse Host ainda
carrega `http://localhost:1437`?

O defeito de referência apresentou simultaneamente os seguintes observáveis:

- o Host `11392` continuou vivo;
- o Global e seu pai terminaram;
- a porta `1437` ficou sem listener e o HTTP deixou de responder;
- a Janela do Projeto permaneceu aberta, porém sem interface.

Essa combinação exclui falha do Host e identifica perda de propriedade do
frontend de desenvolvimento.

## Contratos consultados

A rodada foi fixada em Tauri CLI `2.11.4`, crate `tauri 2.11.5`, Vite `7.3.6`,
Node.js `24.18.0`, `tauri-driver 2.0.6` e WebView2/EdgeDriver
`151.0.4129.78`.

- A configuração do Tauri define `beforeDevCommand` como o comando executado
  antes de `tauri dev` e `devUrl` como a URL carregada durante desenvolvimento.
  `frontendDist` continua sendo a entrada empacotada da produção:
  [Configuration Files](https://v2.tauri.app/develop/configuration-files/) e
  [Vite](https://v2.tauri.app/start/frontend/vite/).
- No código da CLI `2.11.4`, um encerramento normal do aplicativo chama
  `kill_before_dev_process`; no Windows, essa função mata recursivamente a
  árvore do processo iniciado por `beforeDevCommand`:
  [dev.rs, linhas 322–348](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.11.4/crates/tauri-cli/src/dev.rs#L322-L348).
- Um Job Object gerencia processos como unidade. Processos filhos herdam o Job
  por padrão e `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` termina todos os associados
  quando o último handle fecha:
  [AssignProcessToJobObject](https://learn.microsoft.com/en-us/windows/win32/api/jobapi2/nf-jobapi2-assignprocesstojobobject),
  [Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects) e
  [JOBOBJECT_BASIC_LIMIT_INFORMATION](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_basic_limit_information).
- O WebDriver do Edge recomenda o modo *attach* quando há UI nativa ou mais de
  uma WebView. A aplicação é iniciada fora do driver, o Host recebe
  `--remote-debugging-port` e o driver usa `DebuggerAddress`:
  [Automatizar aplicativos WebView2](https://learn.microsoft.com/en-us/microsoft-edge/webview2/how-to/webdriver).
- No Node.js 24 para Windows, `ChildProcess.kill()` encerra o processo alvo de
  modo forçado e abrupto. O gate usa essa propriedade para matar apenas o
  supervisor e comprovar que o Job recolhe os descendentes:
  [Child process](https://nodejs.org/docs/latest-v24.x/api/child_process.html#subprocesskillsignal).
- No Windows, `Start-Process` abre por padrão uma nova janela; o gate solicita
  `WindowStyle Hidden`, sem reutilizar o console do runner. Um emissor pode
  liberar seu console, anexar-se ao console desse processo e gerar
  `CTRL_C_EVENT`; com grupo `0`, o evento alcança todos os processos que
  compartilham aquele console. O emissor ignora o próprio evento e a prova
  observa o término dos PIDs, em vez de inferi-lo do retorno da API:
  [Start-Process](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.management/start-process?view=powershell-5.1),
  [AttachConsole](https://learn.microsoft.com/en-us/windows/console/attachconsole),
  [FreeConsole](https://learn.microsoft.com/en-us/windows/console/freeconsole),
  [SetConsoleCtrlHandler](https://learn.microsoft.com/en-us/windows/console/setconsolectrlhandler) e
  [GenerateConsoleCtrlEvent](https://learn.microsoft.com/en-us/windows/console/generateconsolectrlevent).

## Decisão

`npm run tauri:dev` não delega mais o Vite ao `beforeDevCommand`. Ele prepara o
sidecar e inicia o executável de desenvolvimento `myalbuns-dev`, compilado
somente com a feature `dev-supervisor`. Esse launcher possui dois workers:

1. Vite, cuja resposta HTTP em `1437` é condição para iniciar o restante;
2. Tauri CLI, que continua executando o fluxo real `tauri dev` e o Global.

Cada worker primeiro se bloqueia numa barreira local autenticada. O supervisor
o associa ao Job Object e só então libera a criação de descendentes. Isso fecha
a janela entre `spawn` e `AssignProcessToJobObject`.

O Host produtivo de debug registra seu PID por um canal local autenticado e
fecha o socket depois do envio. O supervisor abre um handle Windows com direito
de sincronização e espera o término desse processo. O TCP serve apenas para
autenticar e transferir o PID; o handle do processo é a autoridade de vida,
portanto a duração do socket não participa do lease.

O supervisor mantém Vite depois que a Tauri CLI e o Global terminam enquanto
há ao menos um handle de Host ativo. O último Host libera o ambiente. Falha do
Vite, falha da CLI/bootstrap, queda do supervisor e Ctrl+C fecham ou terminam o
Job Object, recolhendo Vite, Node, Global, Host e descendentes. O Global não é
mantido artificialmente vivo. Se o usuário fecha normalmente apenas a Tela
Global, sem iniciar Projeto, o supervisor reserva uma janela curta para um
registro de Host atrasado e então conclui a sessão com sucesso.

## Ready causal

`Ready` agora é a conjunção fechada de dois sinais produtivos e idempotentes:

- `host_ready`: política nativa aplicada e Janela do Projeto exibida;
- `project_ui_ready`: React carregou a projeção e confirmou pelo comando Tauri
  público `project_ui_ready`, depois do commit da interface.

O terminal `Ready` é escrito uma única vez, em qualquer ordem de chegada dos
dois sinais. Somente então o Global registra
`global_exited_after_project_handoff` e encerra. Não há espera temporal usada
como prova de prontidão.

## Gate Windows real

O comando reproduzível é:

```powershell
npm run test:dev-lifecycle
```

Ele atravessa launcher → Vite → Tauri CLI → Global → Host → WebView2/React. O
modo de lançamento do `tauri-driver` não pode transferir uma sessão da WebView
do Global para a WebView de outro processo. Por isso o gate segue a orientação
oficial de *attach*: inicia o launcher público, espera o handoff causal e anexa
um EdgeDriver da mesma versão do runtime à porta de depuração exclusiva do
Host. Essa porta e a variável WebView2 só existem em build de debug.

Na fase normal, o gate exige Global encerrado, Host vivo, Vite respondendo,
`.app-shell` presente e screenshot não branca. Antes de enviar `WM_CLOSE` à
janela nativa, ele captura uma floresta recursiva cujas raízes são supervisor,
Global já encerrado e cada processo atual do produto. A subárvore do Host deve
conter o próprio Host e ao menos um descendente WebView2, e todos esses PIDs
precisam pertencer à floresta. O terminal exige zero PID observado, zero Host e
zero listener em `1437`.

Uma fase própria inicia o supervisor em console isolado e só avança depois de
observar o handoff completo: Global já encerrado, Host independente vivo e sua
subárvore WebView2 materializada. Então envia `CTRL_C_EVENT` pela API pública do
Windows e exige os mesmos terminais vazios. Outra fase encerra somente o
supervisor raiz de modo abrupto e prova o fallback `KILL_ON_JOB_CLOSE`. As fases
finais cobrem falha de bootstrap e queda do próprio Vite. Ctrl+C e queda
abrupta permanecem evidências distintas.

Os dados brutos da rodada canônica ficam em
[0022-dev-lifecycle.json](artifacts/0022-dev-lifecycle.json). O JSON identifica
o commit de entrada e marca qualquer mudança rastreada ou não rastreada antes,
durante ou depois da execução.

## Isolamento da produção

`tauri build` preserva `beforeBuildCommand` e `frontendDist: ../dist`. O binário
`myalbuns-dev` exige a feature não padrão `dev-supervisor`; o registro do Host e
a porta de depuração estão sob `debug_assertions`. O executável release não
inicia, consulta nem depende de Vite.
