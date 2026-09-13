---
status: current
document: research
date: 2026-09-13
---

# Queda das Configurações: interferência do RivaTuner no WebView2

## Resultado

A queda das Configurações de **13/09/2026 às 14:29:18**, horário de Brasília,
ocorreu dentro de `RTSSHooks64.dll`, do **RivaTuner Statistics Server (RTSS)**,
carregado no processo browser do WebView2. A instrução que falhou tentava ler
um endereço pertencente a uma instância de `dxgi.dll` já descarregada.

Uma reprodução isolada também derrubou as Configurações **sem abrir Álbum e
sem executar limpeza de Cache**. Seu dump mostra novamente código do RTSS
acessando a mesma posição relativa da biblioteca DXGI descarregada.

A evidência identifica a origem imediata desta falha e sustenta uma
incompatibilidade do hook do RTSS com o ciclo de carregamento do WebView2.
Após autorização do usuário, a detecção do RTSS para `msedgewebview2.exe`
foi desabilitada. Dez aberturas subsequentes de Configurações, a limpeza e
reconstrução de Cache e os testes de exportação passaram sem nova queda.
O Runtime, os drivers, os projetos e o Cache real não foram alterados.

Após os testes, em 13/09/2026, o usuário informou que o programa
“aparentemente está estável”. Esse retorno complementa a validação automatizada
e encerra esta etapa da correção no ambiente local. A exclusão permanece uma
configuração do RivaTuner nesta máquina; ela não é distribuída pelo executável
do MyAlbuns. A recuperação automática implementada em `d288237` permanece
disponível para outras quedas do navegador.

## Ambiente

| Componente | Versão ou identificação |
| --- | --- |
| MyAlbuns | `d28823737f8ff3abd5d4336c4f4b6261508c4199` |
| Executável testado | `.tools/normal-export-app-d288237/myalbuns-desktop.exe` |
| WebView2 x64 | `152.0.4191.66` |
| RTSS.exe instalado | `7.3.5.28314` |
| RTSSHooks64.dll | PE timestamp `68D7A4E4`, tamanho da imagem `00267000` |
| Depurador | Microsoft CDB `10.0.29617.1000`, pacote WinDbg `1.2606.22001.0` |

A DLL não fornece versão de produto em seus recursos; a versão acima é a
do executável RTSS.exe. Os binários do depurador foram extraídos do pacote
oficial para uma pasta local de ferramentas, com assinaturas Microsoft válidas.
Não houve instalação global de depurador. Os dumps permaneceram locais;
somente os símbolos públicos foram obtidos do servidor da Microsoft.

## Falha real

O log registra `webview_process_failed` para `settings` e `global`, ambos
compartilhando o browser PID **19084**. São duas notificações da mesma queda,
não duas quedas independentes. Código `0xC0000005`, `failure_kind=0`
(`BrowserProcessExited`) e `failure_reason=0` (`Unexpected`).

O dump original é `12edf009-8325-4997-81d2-a417de1d3750.dmp`, encontrado no
Crashpad do perfil Global. A cópia está em
`.scratch/debug/webview-settings-20260913-142918/settings-browser-crash.dmp`.
Os comandos `.ecxr`, `k`, `lmvm`, `u` e `lm u` confirmaram:

```text
Exception:       0xC0000005, leitura
Thread:          38052
RIP:             0x1801490af = RTSSHooks64.dll + 0x1490af
Instrução:       mov ecx, dword ptr [rdx]
RDX:             0x7ff9fc079530
Módulo retirado:  dxgi.dll, 0x7ff9fc060000–0x7ff9fc19b000
Posição relativa: dxgi.dll + 0x19530
```

A pilha passa por `RTSSCBTProc`, `user32!DispatchHookA` e
`CreateWindowExW`. Abaixo aparecem a criação de uma janela de COM e o
registro de notificações de custo de rede do Chromium:
`msedge!net::NetworkCostManagerEventSinkWin::RegisterForNotifications`.
Isso mostra um hook do RTSS executando no contexto de uma janela interna do
navegador. A pilha não mostra uma rotina de remoção de arquivos do MyAlbuns.

O nome `ValidateRuntimes+0xc073f` mostrado junto à instrução é a aproximação
pelo símbolo exportado mais próximo. Sem símbolos privados do RTSS, esse nome
não deve ser tratado como identificação exata da função interna. O módulo,
o endereço e a instrução foram confirmados independentemente.

Às **14:29:17**, o arquivo `State/clear-cache-on-startup.v1` foi gravado.
Seu conteúdo é o marcador de limpeza agendada. O serviço de Cache agenda a
limpeza quando existe um namespace ativo; o armazenamento de imagens em
`Cache` é distinto do perfil em `State/WebView2`.

A recuperação recriou as interfaces com browser PID **29128**, e registrou
`webview_recovery_ready` para Settings às **14:29:19.598693**. O intervalo
de aproximadamente um segundo explica o piscar observado. Esse evento
representa a recuperação da página e política da WebView, não uma medição
de cada elemento já pintado na tela.

## Comparação e reprodução

Cinco dumps anteriores de violação de acesso foram comparados: dois do
perfil Global real e três de testes isolados. Todos têm a instrução da
exceção em `RTSSHooks64.dll`; quatro no offset `0x1490af` e um em `0x3b1d0`.
Isso não atribui automaticamente ao RTSS incidentes antigos sem dump.

O ensaio mínimo executado foi:

```powershell
node .tools/Test-SettingsBrowserIdle.mjs
```

O script inicia três instâncias sequenciais com raízes de dados separadas,
abre apenas Configurações e observa eventos de falha por até 15 segundos.
Não chama `clear_all_cache` e não abre projeto. A primeira rodada falhou
após **2.865 ms**; as duas seguintes não falharam nos respectivos intervalos.
O comando encerrou com código **1** e:

```text
RED: Settings browser failed without invoking cache cleanup.
```

O dump da rodada que falhou,
`c1256d12-1f9c-4e92-9f1a-b74d8e0505a1.dmp`, aponta para
`RTSSHooks64.dll+0x3b1d0` e para `dxgi.dll+0x19530` já descarregada.
A falha permanece intermitente; uma rodada sem falha não prova correção.
Um primeiro ensaio do script também encontrou uma corrida no próprio teste,
ao chamar a ponte Tauri antes de ela estar pronta. A espera foi corrigida
antes dessas três rodadas; esse erro de JavaScript não foi contado como
reprodução do problema do usuário.

Logs, dumps e saídas do depurador estão na pasta local de evidências acima,
excluída do versionamento. Os processos encerrados pelo teste foram somente
as instâncias que ele próprio iniciou, verificadas por identidade.

## Intervenção aplicada e validação

Às **14:51:39** de 13/09/2026, foi criado um perfil de exclusão no RTSS para
`msedgewebview2.exe`, com **Application detection level: None**.
A ajuda distribuída com esta instalação do RTSS confirma
que essa opção desabilita detecção, estatísticas e serviços de overlay para
o aplicativo associado ao perfil. O perfil não existia antes da alteração.

Essa exclusão alcança outros aplicativos que usam o mesmo nome de executável
WebView2; ela não é uma preferência interna do MyAlbuns. O ajuste foi aplicado
como alteração explícita e autorizada do ambiente, preservando o perfil global
do RTSS. Não foi necessário encerrar o RTSS nem modificar o executável MyAlbuns.

O SDK fornecido pelo próprio RTSS documenta `LoadProfile`,
`SetProfileProperty("AppDetectionLevel", ...)`, `SaveProfile` e
`UpdateProfiles`. Um helper local usou essa API com a elevação exigida pelo
Windows para gravar em Program Files. A leitura posterior em outro processo
confirmou `AppDetectionLevel=0`; antes o valor efetivo era `1`.
O arquivo criado foi `Profiles/msedgewebview2.exe.cfg` e contém
`EnableHooking=0`, `HookDXGI=0`, `HookDirect3D12=0` e os demais hooks de APIs
gráficas desabilitados pela própria implementação de `AppDetectionLevel`.

O SHA-256 do perfil Global permaneceu
`916258B93C58AD1789E4A47ABE52F02856B6CEF99D1C796D421060683DFA6877`.
A configuração aplicada e o resultado da leitura estão na subpasta local
`rtss-profile-change`. Para reverter, remover somente o perfil criado usando
a interface do RTSS e reiniciar as instâncias de teste.

| Verificação após a exclusão | Resultado |
| --- | --- |
| `node .tools/Test-SettingsBrowserIdle.mjs 10` | Dez processos novos, cada um observado por pelo menos 15 segundos; nenhuma queda de Settings. |
| Preparação de Cache de 60 imagens | 60 entradas preparadas antes de expor o Álbum; nenhum intervalo preto detectado pelo observador nativo. |
| Limpeza pelas Configurações e reabertura | Cache passou de 60 para zero e voltou a 60; progresso determinado, sem mudança de tamanho e sem intervalo preto detectado. |
| Exportação nativa em PNG/PDF | Exportação inicial, ignorar conflitos parciais/totais, cancelar, substituir e ignorar PDF existente passaram. |
| Logs dos três cenários | Nenhum `webview_process_failed` e nenhum registro com nível `ERROR`. |

A DLL `RTSSHooks64.dll` **continuou mapeada** nos browsers das dez rodadas.
Por isso, a validação não afirma ausência de injeção da DLL: o que foi
confirmado é a detecção desabilitada no perfil, os hooks gráficos desabilitados
na configuração persistida e a ausência da falha nos ensaios executados.
As novas instâncias foram iniciadas depois da atualização dos perfis.

As evidências locais estão em `idle-reproduction-1789321943360`,
`exclusion-cache` e `exclusion-export`, dentro da pasta do diagnóstico.
O ensaio de Cache foi executado duas vezes: primeiro para preparar a base,
depois com `--clear-cache --determinate-progress --native-only`. A exportação
usou `.tools/Test-ExportConflicts.mjs` e uma fixture descartável.
Todos os processos de teste foram encerrados ao final.

O resultado sustenta a eficácia da exclusão neste ambiente. Como a falha
original era intermitente, os ensaios não garantem ausência de qualquer outra
falha futura do WebView2. O MyAlbuns testado manteve o mesmo SHA-256:
`2D4E00D12D276CF2EA6E89620DEAD8366C0C2C66118348C67D88E0AC4348CDDD`.

Atualizar o RTSS é uma alternativa a avaliar, mas não foi confirmada uma nota
de versão que corrija precisamente esta assinatura. Não há fundamento nesta
captura para mudar o layout, desativar a GPU do editor ou trocar a pilha
Tauri/WRY como correção deste incidente. A recuperação automática continua
útil, pois reduz o impacto quando o navegador cai.

## Fontes e limites

- [Microsoft: análise de dumps de processos](https://learn.microsoft.com/en-us/windows-hardware/drivers/debugger/analyzing-a-user-mode-dump-file).
- [Microsoft: estrutura da exceção no minidump](https://learn.microsoft.com/en-us/windows/win32/api/minidumpapiset/ns-minidumpapiset-minidump_exception_stream).
- [Microsoft: identificação dos módulos](https://learn.microsoft.com/en-us/windows/win32/api/minidumpapiset/ns-minidumpapiset-minidump_module).
- [Microsoft: eventos de processos do WebView2](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/process-related-events).
- [Microsoft: interferência de DLLs e outras ferramentas no WebView2](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/measures).
- Ajuda primária instalada do RTSS: `Help/BUTTON_NONE` e `Help/BUTTON_ADD`,
  sob `C:/Program Files (x86)/RivaTuner Statistics Server`.
- SDK da mesma instalação: `SDK/Samples/SharedMemory/RTSSSharedMemorySample/RTSSProfileInterface.h`
  e `RTSSProfileInterface.cpp` documentam a propriedade de detecção e o ciclo
  de leitura, gravação e atualização de perfis utilizado.

O Context7 estava sem cota; a consulta utilizou a documentação oficial e os
arquivos de ajuda da versão instalada. A conclusão principal vem dos dumps
locais e da reprodução, não de relatos semelhantes encontrados na internet.
