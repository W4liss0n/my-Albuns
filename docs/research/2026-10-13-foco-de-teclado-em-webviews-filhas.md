---
status: current
document: research
date: 2026-10-13
platform: windows-11-x64
---

# Foco de teclado em WebViews filhas

Pesquisa em fontes primárias sobre um problema medido no aplicativo real com
teclas enviadas por `SendInput`: quando o Windows ativa uma janela do
myAlbuns (Alt+Tab de volta, fechamento de um diálogo com dono, fechamento das
Configurações, barra de tarefas), o foco de teclado fica no HWND de topo
(`Tauri Window`, conferido com `GetGUIThreadInfo.hwndFocus`) e não dentro do
WebView2. Atalhos como Ctrl+W e Enter não fazem nada até um clique. As
medições locais que motivaram esta pesquisa ficam fora do Git.

Versões analisadas: Tauri 2.11.5, `tauri-runtime-wry` 2.11.4, wry 0.55.1,
tao 0.35.3 (as do `Cargo.lock`). Caminhos `$REG/...` abaixo apontam para
`.tools/cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`. A base de
código do myAlbuns é `main` em `7a9b9575`, com a correção ainda não
versionada em `src-tauri/src/webview_keyboard_focus.rs`.

Cada afirmação traz sua fonte. **[verificado]** indica leitura direta da fonte
citada; **[inferência]** indica conclusão minha a partir das fontes.

## 1. A causa no myAlbuns: o recurso `unstable` torna toda WebView filha

- O myAlbuns liga o recurso `unstable` do Tauri
  (`src-tauri/Cargo.toml:35`). **[verificado]**
- Com `unstable`, `tauri-runtime-wry` cria a WebView de **toda**
  `WebviewWindow` como `WebviewKind::WindowChild`, e não só as adicionadas com
  `Window::add_child` (`$REG/tauri-runtime-wry-2.11.4/src/lib.rs:4682-4687`).
  `WindowChild` chama `build_as_child` do wry (`lib.rs:5245`). **[verificado]**
- No wry 0.55.1, a única restauração de foco do WebView2 é a subclasse da
  janela-pai `parent_subclass_proc`, que chama
  `MoveFocus(PROGRAMMATIC)` em `WM_SETFOCUS` e `WM_ENTERSIZEMOVE`
  (`$REG/wry-0.55.1/src/webview2/mod.rs:1254-1257`). Ela só é instalada
  quando `!is_child` (`mod.rs:538-541`). A janela contêiner `WRY_WEBVIEW` de
  uma WebView filha repassa `WM_SETFOCUS` ao primeiro filho
  (`mod.rs:184-200`), mas ninguém dá o foco ao contêiner quando a janela de
  topo é ativada. **[verificado]**
- Consequência: no myAlbuns, nenhuma janela tem restauração de foco do wry, e
  não só o editor. Isso explica por que a janela de Configurações, uma
  `WebviewWindow` comum, também abria com o foco no HWND de topo.
  **[verificado no código; a ligação com a medição é inferência]**
- O `master`/`dev` atual do wry (commit `6642074`, 12/10/2026) mantém o mesmo
  bloqueio `if !is_child` (`src/webview2/mod.rs:614` e `:1361`).
  [wry dev](https://github.com/tauri-apps/wry/blob/6642074224a2db9eb197f69d21b300b02b5dcd49/src/webview2/mod.rs).
  **[verificado]**

### Eventos `Focused` no Tauri 2.11.4 sob `unstable`

- `WindowWrapper.has_children` começa `false` em toda janela criada
  (`lib.rs:4717`) e só vira `true` em `add_child` (`lib.rs:3096`).
  **[verificado]**
- Se `has_children` é `false`, o `Focused` vindo do tao é descartado; o Tauri
  só emite `Focused` sintetizado a partir de `GotFocus`/`LostFocus` do
  controlador WebView2 (`lib.rs:522-556`, `lib.rs:5287-5340`). **[verificado]**
- Portanto, no editor recém-aberto e nas Configurações, `Focused(true)` só
  chega **depois** que a WebView já recebeu foco. O ramo
  `WindowEvent::Focused(true)` de
  `src-tauri/src/desktop_webview_policy.rs:64-66` não cobre a ativação que
  deixou o foco no HWND de topo. Depois de um "Salvar como" (`add_child`),
  `has_children` vira `true` e o `Focused` do tao passa a chegar, de forma
  redundante com a subclasse. **[inferência a partir do código]**

### Comportamento do tao relevante

- `Window::set_focus` não faz nada se a janela já está em primeiro plano
  (`$REG/tao-0.35.3/src/platform_impl/windows/window.rs:174-186`); caso
  contrário, chama `SetForegroundWindow` e, se falhar, simula Alt com
  `SendInput` (`window.rs:1500-1527`). **[verificado]**
- `WindowEvent::Focused` só é emitido quando `is_active && is_focused` muda
  (`event_loop.rs:1779-1806`, `window_state.rs:191-207`). **[verificado]**
- Eventos gerados enquanto o tratador do laço de eventos já está em execução
  são guardados e entregues depois (`event_loop/runner.rs:208-228`). Isso vale
  para tudo o que acontece dentro de uma tarefa `run_on_main_thread`.
  **[verificado]**
- Uma janela criada com `focused(false)` é mostrada com `SW_SHOWNOACTIVATE`
  na primeira vez (`window.rs:1147`, `window_state.rs:329-333`).
  **[verificado]**

## 2. WebView2: o que a Microsoft documenta

- `ICoreWebView2Controller::MoveFocus`: com `PROGRAMMATIC`, o foco vai ao
  elemento que o tinha antes ou ao elemento padrão; com `NEXT`/`PREVIOUS`,
  ao primeiro/último elemento. `NEXT`/`PREVIOUS` servem para o Tab do
  hospedeiro; o próprio WebView só muda o foco por interação do usuário.
  `MoveFocusRequested` dispara quando o usuário sai do WebView com Tab.
  `LostFocus` só dispara depois que o foco de fato sai.
  [ICoreWebView2Controller](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2controller)
  **[verificado]**
- A mesma página diz que, como janela filha, o WebView2 não recebe mensagens
  quando a janela de topo é minimizada ou restaurada, e recomenda
  `IsVisible = FALSE` ao minimizar. **[verificado]**
- **Posição oficial sobre Alt+Tab**: em
  [WebView2Feedback #4626](https://github.com/MicrosoftEdge/WebView2Feedback/issues/4626),
  um membro da equipe (`johna-ms`, 5/8/2024) respondeu que o comportamento é
  intencional: no Alt+Tab o sistema põe o foco na janela de topo e o WebView2
  é uma janela filha, então é preciso chamar `MoveFocus` ao receber o foco na
  janela de topo. **[verificado]** Esse é o padrão recomendado pela Microsoft
  para hospedeiros Win32.
- `MoveFocus` falha com `E_INVALIDARG` (0x80070057) quando a janela dona está
  desabilitada por um diálogo modal
  ([#3256](https://github.com/MicrosoftEdge/WebView2Feedback/issues/3256),
  explicação da equipe em 4/3/2023; ainda aberto) e quando o hospedeiro está
  minimizado ([wry #1798](https://github.com/tauri-apps/wry/issues/1798)).
  `SetFocus` documenta que `ERROR_INVALID_PARAMETER` significa janela
  desabilitada
  ([SetFocus](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setfocus)).
  **[verificado]**
- **Controle WPF oficial**: `OnGotKeyboardFocus` repassa o foco ao
  `CoreWebView2`. A documentação diz que o controle nunca quer ficar num
  estado em que a própria janela dele tenha o foco de teclado. Também diz que
  a reativação da janela-pai pode dar foco ao controle sem esse método, e que
  esse caso é tratado no `WndProc`.
  [OnGotKeyboardFocus](https://learn.microsoft.com/en-us/dotnet/api/microsoft.web.webview2.wpf.webview2.ongotkeyboardfocus)
  **[verificado na documentação; o código do `WndProc` não é público]**
- **Controle WinForms oficial**: a pilha publicada em
  [#2311](https://github.com/MicrosoftEdge/WebView2Feedback/issues/2311)
  mostra `WebView2.WndProc` → `Control.WmSetFocus` → `WebView2.OnGotFocus` →
  `CoreWebView2Controller.MoveFocus`. **[verificado]** Quem leva o foco ao
  controle na reativação é o WinForms: `Form.WmActivate` define `Active`, e
  ativar foca o `ActiveControl`, ou o primeiro controle se nenhum estiver
  escolhido
  ([Form.cs:258-274, 6687-6692](https://github.com/dotnet/winforms/blob/44feaa40e975b60175b9422b87e394f2f34188e6/src/System.Windows.Forms/System/Windows/Forms/Form.cs#L258-L274)).
  **[verificado]**
- **Exemplo Win32 oficial** (`WebView2APISample`, commit `a6a092b`):
  `AppWindow.cpp` não trata `WM_SETFOCUS` nem `WM_ACTIVATE`. O foco é movido
  só por menu (`ControlComponent.cpp:215-230`) e pelo Tab via
  `IsDialogMessage` no laço (`App.cpp:215-238`).
  [WebView2Samples](https://github.com/MicrosoftEdge/WebView2Samples/tree/a6a092b7226347c243677fa6df6b2cbef3dc7228/SampleApps/WebView2APISample)
  **[verificado]** A falta disso no exemplo "Getting Started" está registrada
  em [#2956](https://github.com/MicrosoftEdge/WebView2Feedback/issues/2956)
  (aberto e aceito no backlog). Esse relato propõe
  `MoveFocus(PROGRAMMATIC)` em `WM_ACTIVATE` ou `IsDialogMessage` no laço.
  **[verificado]**
- Não achei documentação da Microsoft dizendo que `MoveFocus` é descartado
  enquanto a página carrega. **[não verificado]**

## 3. Wry e Tauri: o que mudou depois das nossas versões

- **Tauri PR #15625** (`fix(windows)!: restore webview focus for unstable
  feature`, incluído em 22/7/2026) corrige
  [tauri#15624](https://github.com/tauri-apps/tauri/issues/15624): sem foco no
  WebView após Alt+Tab com `unstable` no Windows. Ele fecha a proposta no
  wry ([wry PR #1755](https://github.com/tauri-apps/wry/pull/1755)).
  [PR #15625](https://github.com/tauri-apps/tauri/pull/15625). **[verificado]**
  O que a correção faz, lido no diff:
  - toda janela criada com WebView sob `unstable` passa a ter
    `has_children = true`;
  - um `FocusState { WindowFocused, WebviewFocused{label},
    Blured{last_focused_webview_label} }` guarda a última WebView focada.
    O padrão é `Blured { None }`;
  - no `Focused(true)` do tao, se o estado é `Blured { Some(label) }`, chama
    `focus()` (`MoveFocus`) **na hora** nessa WebView. O `Focused(false)` da
    janela é ignorado; a perda de foco vem só do `LostFocus` da WebView.
  - **[verificado no diff]**
- A correção entrou no `tauri-runtime-wry` **2.12.0**
  ([CHANGELOG](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-runtime-wry/CHANGELOG.md),
  seção 2.12.0). A mesma versão corrige `Window::is_focused` sempre `false`
  em multiwebview ([#15950](https://github.com/tauri-apps/tauri/pull/15950))
  e `Focused` falso/verdadeiro ao arrastar a janela
  ([#15626](https://github.com/tauri-apps/tauri/issues/15626)). O Tauri 2.12.3
  depende de `tauri-runtime-wry ~2.12.1`, que pede `wry ^0.57` e
  `tao ^0.37` (crates.io). O myAlbuns fixa `tauri = "~2.11"`. **[verificado]**
- Limite importante: como o estado inicial é `Blured { None }`, a 2.12 **não**
  foca a WebView na primeira ativação de uma janela cuja WebView nunca teve
  foco, que é o caso de `focused(false)`. O foco inicial continua sendo
  responsabilidade do aplicativo. **[verificado no diff]**
- O mantenedor do wry diz que, com `build_as_child` usado diretamente, é
  esperado que o aplicativo ajuste foco e tamanho; com a multiwebview do Tauri,
  é bug do Tauri (comentário de `Legend-Master` em 30/6/2026 em
  [tauri#15624](https://github.com/tauri-apps/tauri/issues/15624)).
  **[verificado]**
- O relato de #15624 também diz que tentativas no aplicativo de repetir
  `MoveFocus` a partir da ativação geraram ciclos de 140–200 Hz, como o nosso
  primeiro ensaio. **[verificado como relato; o mecanismo não é documentado]**
- **`MoveFocus` na criação**: no wry 0.55.1, `init_webview` termina com
  `controller.MoveFocus(PROGRAMMATIC)?` quando `focused` é verdadeiro
  (`mod.rs:543-548`). O `?` descarta a WebView inteira se a chamada falhar.
  [wry #1798](https://github.com/tauri-apps/wry/issues/1798) reproduz isso com
  a janela minimizada, e o [PR #1799](https://github.com/tauri-apps/wry/pull/1799)
  passa a ignorar o erro a partir do wry **0.56.1**. O comentário em
  `src-tauri/src/native_dialog_window.rs:869-871` está correto.
  **[verificado]**
- `tao 0.37.1`: com `with_focused(false)`, chamadas seguintes de
  `set_visible(true)` passam a **focar** a janela
  ([tao CHANGELOG](https://github.com/tauri-apps/tao/blob/dev/CHANGELOG.md),
  0.37.1). Isso afeta a abertura silenciosa de Projetos em segundo plano numa
  atualização. **[verificado no changelog; o impacto é inferência]**
- [wry PR #1823](https://github.com/tauri-apps/wry/pull/1823) (aberto) mexe na
  subclasse da janela-pai quando WebViews filhas são descartadas. Nada
  publicado ainda. **[verificado]**

## 4. Electron e Chromium

- No Chromium, a janela `Widget` guarda e restaura a *View* focada:
  `FocusManager::StoreFocusedView` ao perder a ativação e
  `RestoreFocusedView` ao ganhar
  ([focus_manager.h:194-209](https://github.com/chromium/chromium/blob/2433fced5937c741f1c7ab03da0899f4b98087a6/ui/views/focus/focus_manager.h#L194-L209);
  [desktop_native_widget_aura.cc:1580-1600](https://github.com/chromium/chromium/blob/2433fced5937c741f1c7ab03da0899f4b98087a6/ui/views/widget/desktop_aura/desktop_native_widget_aura.cc#L1580-L1600)).
  **[verificado]**
- No Windows, `HWNDMessageHandler` trata `WM_ACTIVATE` **depois** do
  processamento padrão (`PostProcessActivateMessage`, chamado após o
  resultado do `WndProc`) e então avisa o delegado (`HandleActivationChanged`)
  ([hwnd_message_handler.cc:1274-1276, 1609-1638](https://github.com/chromium/chromium/blob/2433fced5937c741f1c7ab03da0899f4b98087a6/ui/views/win/hwnd_message_handler.cc#L1609-L1638)).
  **[verificado]**
- O código avisa explicitamente do risco de ciclo infinito entre ativação e
  restauração de foco. Atualiza a ativação antes de restaurar o foco e não
  restaura se há uma janela modal filha
  ([desktop_native_widget_aura.cc:475-510](https://github.com/chromium/chromium/blob/2433fced5937c741f1c7ab03da0899f4b98087a6/ui/views/widget/desktop_aura/desktop_native_widget_aura.cc#L475-L510)).
  **[verificado]**
- **Foco inicial** é um conceito separado: `WidgetDelegate::GetInitiallyFocusedView`.
  Se a janela é mostrada inativa, `Widget::SetInitialFocus` só **registra**
  essa View como a que será restaurada (`SetStoredFocusView`) e a ativação
  posterior a foca
  ([widget.cc:2497-2523](https://github.com/chromium/chromium/blob/2433fced5937c741f1c7ab03da0899f4b98087a6/ui/views/widget/widget.cc#L2497-L2523);
  [widget_delegate.h:507-509](https://github.com/chromium/chromium/blob/2433fced5937c741f1c7ab03da0899f4b98087a6/ui/views/widget/widget_delegate.h#L507-L509)).
  **[verificado]**
- O Electron não trata `WM_ACTIVATE` para foco: o `NativeWindowViews` define
  a view de conteúdo como `GetInitiallyFocusedView` e deixa a restauração para
  o `FocusManager` do Chromium
  ([native_window_views.cc:566-580, 1893-1915, 1969-1971](https://github.com/electron/electron/blob/85376852379c127d7d9da0bc78c03551a4881b18/shell/browser/native_window_views.cc#L1969-L1971)).
  O conteúdo web do Electron não é um HWND filho de outro processo como o
  WebView2, então a analogia vale para o modelo, não para o mecanismo.
  **[verificado; a analogia é inferência]** Não achei issue do Electron
  específica sobre `WebContentsView` e Alt+Tab. **[não verificado]**

## 5. CEF

- No `cefclient`, a janela de topo deixa o `DefWindowProc` tratar
  `WM_ACTIVATE` ("Allow DefWindowProc to set keyboard focus"). Em
  `WM_SETFOCUS`, chama **na hora** `browser->GetHost()->SetFocus(true)`, e só
  se `IsWindowEnabled(hwnd)`. Em `WM_ENABLE(TRUE)`, também foca o navegador
  ("after a modal dialog is dismissed")
  ([root_window_win.cc:724-741, 860-867](https://github.com/chromiumembedded/cef/blob/a875f407e36bdf4fe6c31622e150ce73f77629c8/tests/cefclient/browser/root_window_win.cc#L724-L741);
  [browser_window_std_win.cc:115-121](https://github.com/chromiumembedded/cef/blob/a875f407e36bdf4fe6c31622e150ce73f77629c8/tests/cefclient/browser/browser_window_std_win.cc#L115-L121)).
  **[verificado]**
- `CefFocusHandler::OnSetFocus` permite cancelar um pedido de foco do
  navegador, mas não há orientação sobre ciclos
  ([cef_focus_handler.h](https://github.com/chromiumembedded/cef/blob/a875f407e36bdf4fe6c31622e150ce73f77629c8/include/cef_focus_handler.h)).
  **[verificado]**

## 6. Fundamentos Win32

- `DefWindowProc`, ao receber `WM_ACTIVATE` de ativação com a janela não
  minimizada, põe o foco de teclado na **própria** janela
  ([WM_ACTIVATE](https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-activate);
  [Keyboard Input Overview](https://learn.microsoft.com/en-us/windows/win32/inputdev/about-keyboard-input)).
  Foi exatamente o que medimos. **[verificado]**
- `SW_SHOWMAXIMIZED` e `SW_SHOW` ativam a janela; `SW_SHOWNOACTIVATE` não
  ([ShowWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-showwindow)).
  **[verificado]**
- **Padrão de guardar e restaurar foco**: os diálogos devolvem o foco ao
  controle que o tinha. Para janelas comuns, Raymond Chen (Microsoft) mostra
  duas formas: (a) guardar `GetFocus()` em `WM_ACTIVATE(WA_INACTIVE)`, se for
  filho, e chamar `SetFocus` no guardado na ativação, sem fazer nada se
  minimizado
  ([Old New Thing, 2014](https://devblogs.microsoft.com/oldnewthing/20140521-00/?p=943));
  (b) guardar em `WM_ACTIVATE` e restaurar em **`WM_SETFOCUS`** com `SetFocus`
  dentro do próprio tratador
  ([Old New Thing, 2003](https://devblogs.microsoft.com/oldnewthing/20031021-00?p=42083)).
  **[verificado]**
- Chamar `SetFocus` dentro de `WM_SETFOCUS` não é desaconselhado (é a forma
  (b) acima, e é o que wry e CEF fazem). O alerta é para mexer na **ativação**
  dentro de `WM_ACTIVATE`; nesse caso, a recomendação é postar uma mensagem e
  agir depois
  ([Old New Thing, 2005](https://devblogs.microsoft.com/oldnewthing/20050809-13/?p=34653)).
  `SetFocus` em `WM_ACTIVATE` antes do `DefWindowProc` é desfeito por ele,
  que foca a janela em seguida. É o que descreve o wry PR #1755 e o motivo de
  o Chromium tratar a ativação depois do padrão.
  **[verificado nas fontes; a junção é inferência]**
- Ao fechar um modal, o dono precisa ser reabilitado **antes** de destruir o
  diálogo; se não, o Windows não consegue devolver a ativação a ele e escolhe
  outra janela
  ([Old New Thing, 2004](https://devblogs.microsoft.com/oldnewthing/20040227-00/?p=40463)).
  **[verificado]**

## 7. Síntese: o padrão canônico

| Questão | Prática nas fontes | Fonte |
| --- | --- | --- |
| Mensagem | `WM_SETFOCUS` da janela de topo (depois que o `DefWindowProc` de `WM_ACTIVATE` deu o foco a ela); `WM_ENABLE(TRUE)` como complemento | Microsoft #4626, wry não filha, CEF, Chen 2003 |
| Alternativa | `WM_ACTIVATE` só se tratado **depois** do padrão ou adiado | Chromium, wry PR #1755, Chen 2005 |
| Síncrono ou adiado | Síncrono em `WM_SETFOCUS` | wry `mod.rs:1254-1257`, CEF, WinForms |
| Condições | Janela habilitada e não minimizada; sem modal filho | CEF, Chen 2014, Chromium, #3256, wry #1798 |
| Alvo | A última WebView/View focada, não "a única" | Tauri 2.12 `FocusState`, Chromium `StoreFocusedView`, WinForms `ActiveControl` |
| Foco inicial | Conceito separado: janela mostrada inativa registra quem focar na ativação, ou o app foca explicitamente | Chromium `SetInitialFocus`, WinForms `SelectNextControl`, Tauri 2.12 `Blured{None}` |
| Contra ciclos | Agir só na transição para "foco no quadro"; ignorar a perda de foco da janela causada pela própria WebView | Tauri 2.12, Chromium (ativação antes do foco), Chen (`IsChild`) |

**[inferência a partir das fontes listadas]**

### Por que o gancho geral não cobre a abertura do Projeto

Fatos verificados que se combinam aqui:

1. O gancho é instalado **sob demanda** em `on_window_event`
   (`desktop_webview_policy.rs:52-54`). O Tauri registra esse ouvinte por uma
   mensagem enfileirada (`WindowMessage::AddEventListener`,
   `$REG/tauri-runtime-wry-2.11.4/src/lib.rs:1986-1993`). Para a janela do
   editor, o `Focused` do tao é descartado porque `has_children` é `false`
   (seção 1).
2. A apresentação roda dentro de uma tarefa `run_on_main_thread`
   (`src-tauri/src/product_runtime.rs:1222-1248`). Os eventos gerados nela
   (`Resized` da maximização, por exemplo) só são entregues **depois** dela
   (`runner.rs:208-228`).
3. No caminho maximizado, `SetWindowPlacement` com `SW_SHOWMAXIMIZED`
   (`src-tauri/src/project_window_placement.rs:196-199`) já **ativa** a
   janela. O `WM_SETFOCUS` acontece ali, dentro da tarefa, e
   `window.set_focus()` vira operação nula porque a janela já está em primeiro
   plano (`tao window.rs:174-186`).

Hipóteses, em ordem de probabilidade **[inferência]**:

- **H1**: a subclasse ainda não existia quando o `WM_SETFOCUS` da abertura
  chegou. Nenhum evento da janela tinha chegado ao `on_window_event` antes da
  apresentação, e os da própria apresentação ficam guardados até ela
  terminar.
- **H2**: o `WM_SETFOCUS` veio do `SW_SHOWMAXIMIZED` com a janela ainda
  encoberta (`set_cloaked`), e o `MoveFocus` adiado rodou num momento sem
  efeito.
- **H3**: o `SetForegroundWindow` foi negado pela trava de primeiro plano, e
  a ativação real chegou depois por outro caminho.
- **H4**: no instante do adiamento, a janela tinha mais de uma WebView, e
  `let [webview]` não faz nada. É improvável na abertura, mas possível durante
  "Salvar como", que cria uma WebView de pré-teste.

O que registrar para confirmar, num build de diagnóstico:

- no `observe`, o instante da instalação por HWND;
- no `focus_window_proc`, cada `WM_SETFOCUS`/`WM_KILLFOCUS`/`WM_ACTIVATE`/
  `WM_ENABLE`, com `wParam`, `GetFocus()`, `GetForegroundWindow()`,
  `IsWindowEnabled`, `IsIconic` e o atributo DWM `CLOAKED`;
- na tarefa de apresentação, antes e depois de `set_placement`/`show` e de
  `set_focus`, os mesmos três valores;
- no `focus_webview_later`, se o guarda passou, `webviews().len()` e o
  resultado de `Webview::set_focus`;
- os `GotFocus`/`LostFocus` do controlador (já viram `Focused` sintetizado).

Basta ordenar os registros por tempo: se a instalação vier depois do primeiro
`WM_SETFOCUS`, H1 está confirmada.

## 8. Comparação com a primeira implementação

| Ponto | Hoje (`webview_keyboard_focus.rs`) | Padrão das fontes | Avaliação |
| --- | --- | --- | --- |
| Gatilho | `WM_SETFOCUS` do HWND de topo | O mesmo (Microsoft #4626, wry, CEF) | Correto, não é gambiarra |
| Instalação | Sob demanda, no primeiro evento Tauri da janela | Na criação da janela | Frágil: perde o primeiro foco (H1) |
| Execução | Adiada: `tokio::spawn` → `run_on_main_thread` | Síncrona em `WM_SETFOCUS`; adiar só quando a origem é `WM_ACTIVATE` | Funciona, mas o salto por outra thread não tem ordem garantida |
| Guarda anticiclo | Só age se `GetFocus() == hwnd` | Agir só quando o foco está no quadro (Chen `IsChild`, Tauri `FocusState`) | Correta e com base nas fontes |
| Condições | Nenhuma | Habilitada, não minimizada (CEF, Chen, #3256, wry #1798) | Falta |
| Alvo | Só se a janela tem exatamente uma WebView | Última WebView focada | Aceitável hoje, frágil durante "Salvar como" |
| `Focused(true)` em `desktop_webview_policy.rs:64-66` | Reforço | Não existe nas fontes | Quase sem efeito no 2.11.4 (seção 1); remover |
| Foco inicial em `product_runtime.rs:1237-1242` | Caso especial | Conceito próprio (Chromium, WinForms, Tauri 2.12) | Legítimo; já existe igual em `project_webview_authority.rs:332` e `webview_recovery.rs:450` |

## Recomendação para o myAlbuns

1. **Separar dois conceitos**, como fazem Chromium, WinForms e o Tauri 2.12:
   - *foco inicial*: ao apresentar uma janela que deve receber o teclado, focar
     a WebView explicitamente depois de mostrar e ativar a janela. Um único
     auxiliar usado em todos os pontos de apresentação: abertura do Projeto,
     Configurações (`settings_window.rs:148`), diálogos
     (`native_dialog_window.rs:1050, 1064`), Geração, Lote
     (`global_runtime.rs:2194`). Assim, `product_runtime.rs:1237-1242` deixa
     de ser um caso especial.
   - *restauração na ativação*: a subclasse em `WM_SETFOCUS`.
2. **Instalar a subclasse na criação**, não sob demanda: um plugin interno com
   `Builder::on_window_ready` (`$REG/tauri-2.11.5/src/plugin.rs:490-512`).
   O Tauri chama esse gancho via `run_on_main_thread` logo depois de registrar
   a janela (`manager/window.rs:113-123`); no thread principal, roda na hora
   (`tauri-runtime-wry lib.rs:235-250, 1982-1984`). Como as janelas do
   myAlbuns nascem ocultas, o gancho roda antes de qualquer ativação.
   **[inferência]** Manter o teste `GetWindowSubclass` para não instalar duas
   vezes, e remover a instalação em `on_window_event` e o ramo
   `Focused(true)`.
3. **Condições do gatilho**: só agir se a janela está habilitada e não
   minimizada. Tratar também `WM_ENABLE(TRUE)` quando a janela é a ativa e
   `GetFocus() == hwnd`, como o CEF. Isso cobre o fim de um modal em que a
   ativação chegou antes da reabilitação.
4. **Execução**: preferir chamar na hora dentro de `WM_SETFOCUS`, como wry e
   CEF, mantendo a guarda `GetFocus() == hwnd`. Se a medição mostrar ciclo,
   adiar com `PostMessage` de uma mensagem própria para o próprio HWND e
   reconferir a guarda ao tratá-la, como propôs o wry PR #1755. Evitar o
   salto por `tokio`. **[inferência; precisa de medição]**
5. **Alvo**: guardar a última WebView focada a partir dos `Focused`
   sintetizados (ou de `GotFocus`), em vez de exigir uma WebView só.
6. **Médio prazo**: atualizar para Tauri 2.12.x, que traz a restauração do
   tauri#15625, a correção de `is_focused` e o wry 0.56.1+ sem perda da WebView
   na criação. Depois, medir de novo e provavelmente remover a subclasse,
   ficando só com o foco inicial (item 1) e o `WM_ENABLE` (item 3), se ainda
   for preciso. Antes, verificar o efeito do tao 0.37.1 em
   `with_focused(false)` + `set_visible(true)` na abertura silenciosa.
   Desligar `unstable` devolveria o tratamento do wry, mas o editor depende de
   `add_child` para o "Salvar como".

## Resultado da implementação

Medido em 13/10/2026 no app real (compilação de desenvolvimento, teclas e
cliques reais do Windows, cópia local de um Projeto do myAlbuns antigo). As
provas ficam fora do Git.

O que foi implementado em `src-tauri/src/desktop_webview_policy.rs`, junto
das outras regras que valem para toda janela:

- `desktop_webview_policy::plugin()`, com `Builder::on_window_ready`,
  registrado nos dois processos (`product_runtime.rs` e `global_runtime.rs`):
  a subclasse é instalada uma vez, na criação de cada janela.
- Gatilhos: `WM_SETFOCUS` e `WM_ENABLE(TRUE)` da janela de topo.
- Execução adiada por `PostMessage` de uma mensagem registrada para o próprio
  HWND. Não foi medida a versão síncrona: no thread principal, o
  `Webview::set_focus` do Tauri executa na hora e usaria o estado interno do
  Tauri no meio de uma mensagem que pode ter vindo de dentro dele.
- Ao tratar a mensagem: só age com a janela habilitada, não minimizada e com
  `GetFocus() == hwnd`.
- Alvo: a WebView com o rótulo da janela. Toda WebView de conteúdo do
  myAlbuns usa o rótulo da sua janela; a exceção é a WebView oculta de 1×1
  que o "Salvar como" cria na janela do editor.

Removidos: a instalação sob demanda em `on_window_event`, o ramo
`Focused(true)`, a chamada explícita na abertura do Projeto e as chamadas
explícitas nos diálogos. Do lado da interface, Esc faz o que o botão de
fechar faz em toda janela que usa `OwnedWindowShell` (`src/ui/OwnedWindowShell.tsx`),
e a ação em destaque de `ConfirmationDialog` e `MessageDialog` recebe o foco
ao abrir (`src/ui/useDefaultDialogAction.ts`).

**A abertura do Projeto era H1.** Com a subclasse instalada na criação, o
registro mostrou o foco passado à WebView 7 ms depois de a janela ser
apresentada, com uma única WebView na janela. Não foi preciso um auxiliar de
foco inicial: na abertura, o `WM_SETFOCUS` da ativação basta quando a
subclasse já existe. Ao fechar um diálogo chegam `WM_ENABLE(TRUE)` e depois
`WM_SETFOCUS`. O primeiro passa o foco; no segundo, a guarda vê que o
teclado já está na WebView e não faz nada.

| Cenário | Tauri 2.11.4 sem correção | Com a correção |
| --- | --- | --- |
| Abrir o Projeto e usar um atalho sem clicar | não chega | chega |
| Alt+Tab para fora e de volta, Ctrl+W | não chega | chega |
| Configurações abrem com o teclado na página | não | sim |
| Fechar Configurações, Ctrl+W | não chega | chega |
| Fechar diálogo de confirmação, Ctrl+W | não chega | chega |
| Voltar clicando direto num campo e digitar | texto completo | texto completo |
| Trocas de foco com o editor parado por 4 s | 0 | 0 |

A versão anterior, sem guarda e acionada por `Focused(true)`, chegou a cerca
de 400 trocas de foco em 4 s e perdia teclas digitadas.

Continua valendo o item 6 da recomendação: o Tauri 2.12 pode tornar a
subclasse desnecessária, o que precisa ser medido.

## Afirmações não verificadas

- O código do `WndProc` dos controles WinForms e WPF do WebView2: o SDK não
  publica o código. Só há a documentação da API e as pilhas nas issues.
- Por que focar a WebView faz o foco "voltar" ao HWND de topo e gera o ciclo
  medido. Nenhuma fonte da Microsoft descreve isso; há só o relato de
  #15624.
- A afirmação de #15624 de que, no Alt+Tab, o Windows põe o foco direto no
  `Chrome_WidgetWin_1`. Ela contradiz a resposta da Microsoft em #4626 e a
  nossa medição (foco no `Tauri Window`).
- Que `MoveFocus` seja descartado enquanto a página carrega.
- Por que `IsDialogMessage` no laço resolveria a reativação (#2956): é uma
  sugestão da comunidade, sem mecanismo documentado, porque `WM_ACTIVATE` é
  enviada e não passa pelo laço.
- O comportamento do Tauri 2.12 no myAlbuns: não foi testado.
- Issue do Electron específica sobre `WebContentsView` e Alt+Tab: não
  encontrada.

## Fontes

Código local (registro do Cargo, `$REG`):

- `$REG/wry-0.55.1/src/webview2/mod.rs:184-200, 538-548, 1254-1257, 1493-1500`
- `$REG/tao-0.35.3/src/platform_impl/windows/window.rs:174-186, 1147, 1500-1527`
- `$REG/tao-0.35.3/src/platform_impl/windows/event_loop.rs:1779-1806`
- `$REG/tao-0.35.3/src/platform_impl/windows/event_loop/runner.rs:208-228`
- `$REG/tao-0.35.3/src/platform_impl/windows/window_state.rs:191-207, 329-333`
- `$REG/tauri-runtime-wry-2.11.4/src/lib.rs:235-250, 522-556, 1982-1993, 3096, 3967-3971, 4682-4687, 4717, 5245, 5287-5340`
- `$REG/tauri-2.11.5/src/plugin.rs:490-512`, `src/manager/window.rs:83-123`,
  `src/webview/webview_window.rs:535-539, 2259-2262`,
  `src/webview/mod.rs:1535-1538`, `Cargo.toml:130`

Código do myAlbuns:

- `src-tauri/Cargo.toml:35`; `src-tauri/tauri.conf.json` (janela `project`,
  `"focus": false`)
- `src-tauri/src/webview_keyboard_focus.rs`;
  `src-tauri/src/desktop_webview_policy.rs:52-66`
- `src-tauri/src/product_runtime.rs:542, 1222-1248`;
  `src-tauri/src/project_window_placement.rs:189-199`
- `src-tauri/src/project_webview_authority.rs:304-332`;
  `src-tauri/src/native_dialog_window.rs:869-871`

Microsoft:

- [ICoreWebView2Controller](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2controller)
- [WPF WebView2.OnGotKeyboardFocus](https://learn.microsoft.com/en-us/dotnet/api/microsoft.web.webview2.wpf.webview2.ongotkeyboardfocus)
- [WebView2Feedback #4626](https://github.com/MicrosoftEdge/WebView2Feedback/issues/4626),
  [#3256](https://github.com/MicrosoftEdge/WebView2Feedback/issues/3256),
  [#2311](https://github.com/MicrosoftEdge/WebView2Feedback/issues/2311),
  [#2956](https://github.com/MicrosoftEdge/WebView2Feedback/issues/2956)
- [WebView2Samples @ a6a092b](https://github.com/MicrosoftEdge/WebView2Samples/tree/a6a092b7226347c243677fa6df6b2cbef3dc7228/SampleApps/WebView2APISample)
- [dotnet/winforms Form.cs @ 44feaa4](https://github.com/dotnet/winforms/blob/44feaa40e975b60175b9422b87e394f2f34188e6/src/System.Windows.Forms/System/Windows/Forms/Form.cs)
- [WM_ACTIVATE](https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-activate),
  [Keyboard Input Overview](https://learn.microsoft.com/en-us/windows/win32/inputdev/about-keyboard-input),
  [SetFocus](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setfocus),
  [ShowWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-showwindow)
- Old New Thing:
  [2003](https://devblogs.microsoft.com/oldnewthing/20031021-00?p=42083),
  [2004](https://devblogs.microsoft.com/oldnewthing/20040227-00/?p=40463),
  [2005](https://devblogs.microsoft.com/oldnewthing/20050809-13/?p=34653),
  [2014](https://devblogs.microsoft.com/oldnewthing/20140521-00/?p=943)

Tauri e wry:

- [tauri#15624](https://github.com/tauri-apps/tauri/issues/15624),
  [tauri PR #15625](https://github.com/tauri-apps/tauri/pull/15625),
  [tauri PR #15950](https://github.com/tauri-apps/tauri/pull/15950),
  [tauri#15626](https://github.com/tauri-apps/tauri/issues/15626),
  [tauri-runtime-wry CHANGELOG](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-runtime-wry/CHANGELOG.md)
- [wry PR #1755](https://github.com/tauri-apps/wry/pull/1755),
  [wry #1798](https://github.com/tauri-apps/wry/issues/1798),
  [wry PR #1799](https://github.com/tauri-apps/wry/pull/1799),
  [wry PR #1823](https://github.com/tauri-apps/wry/pull/1823),
  [wry dev @ 6642074](https://github.com/tauri-apps/wry/blob/6642074224a2db9eb197f69d21b300b02b5dcd49/src/webview2/mod.rs),
  [tao CHANGELOG](https://github.com/tauri-apps/tao/blob/dev/CHANGELOG.md)

Chromium, Electron e CEF:

- Chromium @ 2433fce:
  [focus_manager.h](https://github.com/chromium/chromium/blob/2433fced5937c741f1c7ab03da0899f4b98087a6/ui/views/focus/focus_manager.h),
  [desktop_native_widget_aura.cc](https://github.com/chromium/chromium/blob/2433fced5937c741f1c7ab03da0899f4b98087a6/ui/views/widget/desktop_aura/desktop_native_widget_aura.cc),
  [hwnd_message_handler.cc](https://github.com/chromium/chromium/blob/2433fced5937c741f1c7ab03da0899f4b98087a6/ui/views/win/hwnd_message_handler.cc),
  [widget.cc](https://github.com/chromium/chromium/blob/2433fced5937c741f1c7ab03da0899f4b98087a6/ui/views/widget/widget.cc),
  [widget_delegate.h](https://github.com/chromium/chromium/blob/2433fced5937c741f1c7ab03da0899f4b98087a6/ui/views/widget/widget_delegate.h)
- [Electron native_window_views.cc @ 8537685](https://github.com/electron/electron/blob/85376852379c127d7d9da0bc78c03551a4881b18/shell/browser/native_window_views.cc)
- CEF @ a875f40:
  [root_window_win.cc](https://github.com/chromiumembedded/cef/blob/a875f407e36bdf4fe6c31622e150ce73f77629c8/tests/cefclient/browser/root_window_win.cc),
  [browser_window_std_win.cc](https://github.com/chromiumembedded/cef/blob/a875f407e36bdf4fe6c31622e150ce73f77629c8/tests/cefclient/browser/browser_window_std_win.cc),
  [cef_focus_handler.h](https://github.com/chromiumembedded/cef/blob/a875f407e36bdf4fe6c31622e150ce73f77629c8/include/cef_focus_handler.h)
