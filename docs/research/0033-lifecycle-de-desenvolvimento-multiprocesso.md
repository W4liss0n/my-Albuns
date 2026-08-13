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
`151.0.4129.78`. A fronteira Win32 usa os bindings `windows-sys 0.61.2`.

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
  quando o último handle fecha. Em Windows 8 ou posterior, Jobs podem ser
  aninhados; `JOB_OBJECT_LIMIT_BREAKAWAY_OK` junto de
  `CREATE_BREAKAWAY_FROM_JOB` retira o filho apenas do Job imediato até
  encontrar um ancestral que não permite a separação:
  [AssignProcessToJobObject](https://learn.microsoft.com/en-us/windows/win32/api/jobapi2/nf-jobapi2-assignprocesstojobobject),
  [Nested Jobs](https://learn.microsoft.com/en-us/windows/win32/procthread/nested-jobs),
  [Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects) e
  [JOBOBJECT_BASIC_LIMIT_INFORMATION](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_basic_limit_information).
- O PID identifica um processo apenas durante a vida de seu objeto e pode ser
  reutilizado depois que o objeto é liberado. O handle devolvido na criação
  continua ligado ao objeto exato enquanto estiver aberto:
  [Process Handles and Identifiers](https://learn.microsoft.com/en-us/windows/win32/procthread/process-handles-and-identifiers) e
  [PROCESS_INFORMATION](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/ns-processthreadsapi-process_information).
- `GetProcessTimes` devolve o instante de criação do processo como `FILETIME`,
  um valor de 64 bits em unidades de 100 nanossegundos. O handle consultado
  precisa de `PROCESS_QUERY_INFORMATION` ou
  `PROCESS_QUERY_LIMITED_INFORMATION`; o último existe em todas as versões de
  Windows suportadas pelo produto:
  [GetProcessTimes](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getprocesstimes) e
  [FILETIME](https://learn.microsoft.com/en-us/windows/win32/api/minwinbase/ns-minwinbase-filetime).
- Um handle de processo permanece válido depois do término. Uma espera de zero
  milissegundos distingue o objeto ainda ativo (`WAIT_TIMEOUT`) do objeto já
  sinalizado (`WAIT_OBJECT_0`) sem introduzir espera temporal no handshake:
  [WaitForSingleObject](https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-waitforsingleobject).
- `GetWindowThreadProcessId` correlaciona uma janela ao PID proprietário e
  `SendMessageTimeoutW` limita a espera por `WM_CLOSE`; retorno zero representa
  falha ou timeout. O gate combina essa janela correlacionada com a identidade
  de criação já validada do processo:
  [GetWindowThreadProcessId](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getwindowthreadprocessid) e
  [SendMessageTimeoutW](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendmessagetimeoutw).
- Em `tauri 2.11.5`, `with_webview` entrega o handle específico da plataforma
  numa closure executada na main thread, enquanto `on_page_load` oferece o
  terminal público `PageLoadEvent` da WebView. Como ambas as janelas usam
  `create: false`, a política nativa é ligada ao `Finished` da primeira carga,
  quando a WebView já pertence ao runtime:
  [WebviewWindow::with_webview](https://docs.rs/tauri/2.11.5/tauri/webview/struct.WebviewWindow.html#method.with_webview) e
  [WebviewWindowBuilder::on_page_load](https://docs.rs/tauri/2.11.5/tauri/webview/struct.WebviewWindowBuilder.html#method.on_page_load).
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

Depois de criar cada Host, o Global usa o handle exato do `Child` para capturar
uma `HostProcessInstanceId`: o par tipado entre o PID usado como localizador e o
`FILETIME` de criação. Isso ocorre antes de o pai enviar o request de bootstrap,
portanto o Host ainda não alcançou o registro. A autoridade da execução autoriza
como credencial consumível o `launch_nonce` já correlacionado à tentativa,
vinculado a essa identidade de instância. A autoridade não é herdada pelo Host:
seu comando recebe somente o endpoint e a credencial individual.

O Host produtivo de debug registra seu PID por esse canal local autenticado. O
servidor reserva o nonce atomicamente uma única vez e, ainda antes de criar uma
lease ou responder `REGISTERED`, abre um handle Windows com
`SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SET_QUOTA |
PROCESS_TERMINATE`, consulta `GetProcessTimes` e exige a mesma
`HostProcessInstanceId`. No mesmo ponto de linearização, uma espera não
bloqueante exige que esse handle ainda esteja não sinalizado e o supervisor
comprova/estabelece sua associação ao Job externo. Só depois publica
`Connected` e `REGISTERED`. PID diferente, criação diferente, Host já encerrado,
falha da API ou replay são terminais fechados: a credencial permanece queimada,
nenhum evento `Connected` é emitido e nenhum processo alheio é acompanhado. O
PID permanece somente localizador e diagnóstico; a identidade de criação valida
a aquisição e o handle exato validado passa a ser a autoridade de vida.

Cada aquisição validada recebe um `HostLeaseId` opaco. Conexão e desconexão
transportam o mesmo `HostLeaseId`; assim, uma notificação atrasada do handle
antigo não remove outro Host que tenha reutilizado o mesmo número de processo.
O socket fecha depois da confirmação e sua duração não participa da lease.
Credenciais, identidade de criação e IDs de lease não são gravados nos logs.

O supervisor mantém Vite depois que a Tauri CLI e o Global terminam enquanto
há ao menos um handle de Host ativo. O último Host libera o ambiente. Falha do
Vite, falha da CLI/bootstrap, queda do supervisor e Ctrl+C fecham ou terminam o
Job Object, recolhendo Vite, Node, Global, Host e descendentes. O Global não é
mantido artificialmente vivo. Se o usuário fecha normalmente apenas a Tela
Global, sem iniciar Projeto, o supervisor reserva uma janela curta para um
registro de Host atrasado e então conclui a sessão com sucesso.

Global e Host constroem sua WebView oculta durante `setup` e instalam antes da
criação um handshake de `PageLoadEvent::Finished`. Esse callback aplica a
política pelo handle nativo e conclui uma readiness consumível. A janela só é
exibida — e `host_ready` só pode ser publicado — depois desse terminal. Isso
evita enfileirar `with_webview` enquanto o runtime ainda não registrou a WebView,
sem transformar sleep, retry ou visibilidade em prova de prontidão.

Antes de Tauri criar qualquer WebView, cada processo desktop supervisionado
instala também um Job local aninhado com `KILL_ON_JOB_CLOSE`. Assim, os processos
brokerados do WebView2 encerram causalmente com o Global ou Host que os criou,
mesmo quando sobrevivem ao PID principal por alguns segundos. O Job local da
Global permite separação explícita; o comando do Host usa
`CREATE_BREAKAWAY_FROM_JOB`, sai somente desse Job imediato, permanece no Job
externo do supervisor e instala seu próprio Job local antes de registrar a
lease. O handoff portanto não mantém a Global viva nem mata o Host ao fechar a
Global.

A instalação desse Job local é parte fechada do bootstrap. Uma falha anterior à
WebView emite o terminal estruturado `desktop_start_failed` com estágio e código
estáveis, encerra o processo desktop com código diferente de zero e faz o
supervisor recolher Vite e toda a árvore. Ela nunca percorre o terminal legítimo
`dev_global_only_exited`. O gate possui uma sonda exclusiva do build de debug
para provocar essa falha antes de criar a WebView e comprovar o caminho
produtivo completo, sem depender de uma falha ambiental intermitente.

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
conter o próprio Host e ao menos um descendente WebView2. O instrumento captura
PID e instante de criação de cada objeto antes do terminal; o fechamento exige
zero dessas instâncias exatas, sem confundir reutilização posterior de PID com
processo órfão. Também exige zero Host e zero listener em `1437`.

Global e Host deixam de ser números assim que são descobertos. O instrumento
captura no mesmo snapshot o PID e o instante de criação e conserva esse valor
imutável em todas as provas posteriores. Liveness compara a instância completa;
a expansão de uma raiz aceita somente o mesmo objeto e descendentes criados
depois dele; `WM_CLOSE` revalida a mesma instância imediatamente antes de
sinalizar. O cleanup de contingência encerra o supervisor que possui o Job, sem
sinalizar o PID armazenado do Global ou Host. Um PID reutilizado falha fechado e
não pode provar handoff, virar raiz nem receber fechamento. Uma regressão
independente do runner usa um processo Windows real e um instante de criação
divergente para provar os três terminais.

O `WM_CLOSE` usa a janela nativa correlacionada à instância e
`SendMessageTimeoutW` com limite de cinco segundos. Uma janela sem resposta
falha o gate e entrega o controle ao cleanup exato, sem bloquear o instrumento;
uma regressão cria uma janela Windows real sem message pump e prova esse limite.

Antes de iniciar o supervisor, o gate exige zero instâncias preexistentes do
mesmo binário desktop. A precondição torna a descoberta pertencente à rodada:
uma sessão debug alheia falha o instrumento antes do launch, em vez de ser
promovida a Global, Host ou raiz de cleanup.

A mesma autoridade de instância vale para todo sinal emitido pelo instrumento.
O listener Vite, o WebDriver e cada supervisor são capturados com PID e instante
de criação; `CTRL_C_EVENT`, término forçado e fallbacks de cleanup revalidam essa
identidade imediatamente antes da operação. Uma reutilização de PID é tratada
como instância ausente e nunca autoriza sinalizar uma árvore alheia.

Uma fase própria inicia o supervisor em console isolado e só avança depois de
observar o handoff completo: Global já encerrado, Host independente vivo e sua
subárvore WebView2 materializada. Então envia `CTRL_C_EVENT` pela API pública do
Windows e exige os mesmos terminais vazios. Outra fase encerra somente o
supervisor raiz de modo abrupto e prova o fallback `KILL_ON_JOB_CLOSE`. As fases
finais cobrem falha de bootstrap, falha de contenção anterior à WebView e queda
do próprio Vite. Ctrl+C e queda abrupta permanecem evidências distintas.

Os dados brutos da rodada canônica ficam em
[0022-dev-lifecycle.json](artifacts/0022-dev-lifecycle.json). O JSON identifica
o commit de entrada e marca qualquer mudança rastreada ou não rastreada antes,
durante ou depois da execução.

## Isolamento da produção

`tauri build` preserva `beforeBuildCommand` e `frontendDist: ../dist`. O binário
`myalbuns-dev` exige a feature não padrão `dev-supervisor`; o registro do Host e
a porta de depuração estão sob `debug_assertions`. O executável release não
inicia, consulta nem depende de Vite.
