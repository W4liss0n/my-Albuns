---
status: current
document: research
date: 2026-09-13
---

# Diálogo preto: pesquisa de falhas do WebView2

## Evidência posterior: origem identificada em dump real

A investigação posterior à implementação da recuperação encontrou uma falha
em `RTSSHooks64.dll`, do RivaTuner, acessando uma biblioteca DXGI já descarregada.
O mesmo padrão foi reproduzido abrindo Configurações sem limpar Cache.
Consulte [o diagnóstico com dump e reprodução](2026-09-13-rivatuner-webview2-configuracoes.md).
O levantamento abaixo preserva o estado anterior, quando ainda faltava essa
captura; suas hipóteses não substituem a evidência posterior.

## Recomendação para o MyAlbuns

A prioridade é tratar a perda do navegador que desenha a interface e obter um
dump da falha. O isolamento do diálogo e a desativação de sua GPU permanecem
mitigações; os testes já mostraram que a janela do Álbum também pode perder
seu navegador. Para o progresso simples, uma implementação Win32 é uma
alternativa documentada que elimina a dependência de um navegador nessa tela.
Isso não corrige, por si só, a falha da janela do Álbum.

Esta é uma recomendação de engenharia, não uma correção já validada. A pesquisa
não encontrou uma atualização específica comprovadamente capaz de resolver
o encerramento observado. As fontes, limites e próximos testes estão abaixo.

## Escopo e versões

Pesquisa somente em documentação, código, releases e relatos nos repositórios
oficiais. Consulta em 13 de setembro de 2026; nenhum Runtime, dependência ou
comportamento do aplicativo foi alterado nesta pesquisa.

O [Cargo.lock](../../Cargo.lock) resolve Tauri 2.11.5, tauri-runtime-wry 2.11.4,
WRY 0.55.1, tao 0.35.3 e webview2-com 0.38.2. A conferência local do executável
do Runtime identificou WebView2 152.0.4191.66.

O sintoma visual precisa ser separado da classe de falha: os registros locais
contêm `BrowserProcessExited` (`kind=0`), `Unexpected` (`reason=0`) e saída
`-1073741819` (`0xC0000005`). Esse código corresponde a uma violação de acesso;
não identifica sozinho GPU, driver ou módulo defeituoso. Um engenheiro da
Microsoft fez essa mesma distinção ao responder a um relato com o mesmo código,
solicitando versão atual e dumps para investigação.
[Resposta da Microsoft em WebView2Feedback #4199](https://github.com/MicrosoftEdge/WebView2Feedback/issues/4199#issuecomment-3339588902).

## O que as evidências locais permitem afirmar

- Os registros de `soft-clean` mostram saída do navegador da janela Global
  às `16:09:35Z` e da janela `project` às `16:10:01Z` e `16:15:07Z`, em
  13/09/2026. Todos apresentam `kind=0`, `reason=0` e `-1073741819`.
- Nos bindings instalados, `kind=0` é `BrowserProcessExited`; GPU tem outro
  valor (`6`). `reason=0` é `Unexpected`, e não `Crashed` (`3`). O encerramento
  está comprovado, mas a classificação não revela sua causa interna.
  [Enum oficial de motivos](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/winrt/microsoft_web_webview2_core/corewebview2processfailedreason?view=webview2-winrt-1.0.4078.44).
- O [handler atual](../../src-tauri/src/desktop_webview_policy.rs) registra a
  falha, mas não inicia reconstrução da WebView. O
  [diálogo](../../src-tauri/src/native_dialog_window.rs) possui um snapshot do
  progresso no Host; o [processamento inicial](../../src-tauri/src/product_runtime.rs)
  conserva sua conclusão em `OnceCell`. São pontos disponíveis para projetar
  recuperação sem executar novamente o processamento já concluído.
- O [Cache dos Álbuns](../../crates/myalbuns-paths/src/app_paths.rs) fica em
  `Cache`; os perfis do navegador ficam em `State/WebView2`. São armazenamentos
  distintos. A limpeza em [cache_service.rs](../../src-tauri/src/cache_service.rs)
  opera nos namespaces do Cache. Não foi encontrada evidência de que esse
  comando esteja apagando o perfil de um navegador vivo.
- Não há `.dmp` nos perfis de `soft-clean` inspecionados. O arquivo
  `edge_shutdown_crash.txt`, contendo apenas `1`, não fornece uma pilha de falha.
- A máquina possui NVIDIA RTX 3050 Laptop e Intel UHD. A existência de duas
  GPUs é contexto para um teste futuro, não prova de conflito entre drivers.

As evidências anteriores estão em
[black-dialog-findings.md](../../.scratch/debug/cache-reconstruction/black-dialog-findings.md)
e nos logs sob `.scratch/debug/cache-reconstruction/soft-clean`. Esses arquivos
são locais e ignorados pelo Git; esta pesquisa registra o resultado, mas não
torna esses artefatos parte de um clone do repositório.

## Soluções documentadas e sua aplicação

### Recuperar a WebView conforme o processo que falhou

A Microsoft distingue as ações necessárias:

| Evento | Tratamento documentado |
| --- | --- |
| `BrowserProcessExited` | Recriar os controles WebView2 afetados. |
| `RenderProcessExited` | Recarregar o conteúdo ou recriar o controle. |
| `GpuProcessExited` | O Runtime normalmente recupera o processo automaticamente. |

`ProcessFailed` e `BrowserProcessExited` podem chegar em qualquer ordem.
Limpeza de perfil, encerramento e recuperação precisam ser coordenados.
[Tratamento oficial de eventos de processo](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/process-related-events).

Aplicação proposta: manter o Host Rust e a Sessão do Projeto vivos; reconstruir
somente a apresentação a partir de um snapshot consistente. Uma tentativa
limitada, identificada por geração, deve ser cancelada se a operação terminar ou
o usuário fechar a janela. Se falhar novamente, apresentar um erro utilizável.
Preservar alterações não salvas e impedir repetição de comandos são critérios
de implementação, ainda não verificados. Isso não significa reiniciar
automaticamente o processo Global, comportamento excluído pelo
[ADR 0005](../adr/0005-adotar-tauri-react-rust.md).

A recriação deve ser agendada depois do callback. WebView2 exige a thread de
interface com processamento de mensagens; abrir um diálogo modal síncrono
dentro de um callback pode causar reentrada não suportada. Não há evidência
suficiente para atribuir a falha atual a isso, mas uma recuperação nova precisa
respeitar esse contrato.
[Modelo de threads](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/threading-model).

### Usar uma janela nativa para o progresso simples

A Microsoft recomenda XAML ou Win32 para telas iniciais e diálogos simples,
evitando custo de inicialização e disputa de recursos do WebView2. Também
recomenda manter a aceleração por GPU, exceto durante diagnóstico.
[Boas práticas oficiais](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/performance#dont-use-webview2-for-initial-ui).

Para o MyAlbuns, uma janela Win32 desenhada com as cores, tipografia, espaçamento
e dimensões aceitos pode mostrar as mesmas etapas e contagens sem criar outro
navegador. Exige verificação de DPI, acessibilidade e bloqueio do proprietário.
O mesmo adaptador atual também apresenta decisões de Recuperação e Cópia externa;
a migração deve preservar essas transições. Não basta trocar uma barra de
progresso ou adotar um diálogo genérico do Windows.

Desligar a GPU do editor inteiro não é uma solução compatível com o requisito
de WebGL2 acelerado do ADR 0005. A flag atual do diálogo deve ser reavaliada
depois do diagnóstico, sem tratá-la como demonstração de defeito no compositor.

### Identificar a origem antes de mudar mais configurações

Consultar `FailureReportFolderPath` permite localizar os minidumps. A Microsoft
avisa que uma saída inesperada pode não produzir dump; também podem existir
dumps antigos sem relação com o evento atual. É necessário correlacionar
horário, processo e versão.
[API de relatórios](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2environment11).

Se o Crashpad não produzir material, uma próxima reprodução isolada pode usar
ProcDump anexado ao PID exato do navegador de teste: monitoramento de exceção
(`-e`) e, conforme o caso, término (`-t`). O dump de término não substitui
necessariamente a captura da exceção original.
[ProcDump oficial](https://learn.microsoft.com/en-us/sysinternals/downloads/procdump).
O objetivo é obter módulo, instrução e pilha relacionados ao acesso inválido.
[Diagnóstico de C0000005](https://learn.microsoft.com/en-us/shows/inside/c0000005).

Só então direcionar a investigação para Runtime, driver, lifetime ou interferência
externa. A Microsoft documenta falhas provocadas por injeção de DLL e bloqueio
de processos por ferramentas de segurança. É uma hipótese a verificar por
eventos e módulos; não há evidência local que justifique desativar proteções.
[Compatibilidade com ferramentas de segurança](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/measures).

### Comparar versões de forma controlada

Evergreen recebe atualizações automaticamente. Para comparar versões de forma
reprodutível, a Microsoft oferece Fixed Version, selecionada pelo diretório dos
binários ao criar o ambiente. Isso permite um experimento separado sem trocar
o Runtime de todos os aplicativos. Manter Fixed Version em produção traz a
responsabilidade de distribuir atualizações.
[Distribuição do Runtime](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution).

Proposta: mesmo executável, mesmas imagens sintéticas e mesma sequência de
abertura, mudando uma variável por vez. Usar perfis novos por versão. Comparar
o Runtime atual com outra versão disponível e suportada; depois avaliar a
atualização compatível da pilha Tauri/WRY. Não combinar todas as mudanças no
primeiro teste, pois isso impediria identificar qual delas altera a falha.

## Relatos comparáveis e correções publicadas

As correções abaixo foram publicadas para os respectivos casos. Sua aplicação
ao MyAlbuns exige reproduzir a condição e comparar processo, pilha e versão.

| Fonte primária | O que foi observado ou corrigido | Correspondência e limite local |
| --- | --- | --- |
| [WRY #1794](https://github.com/tauri-apps/wry/issues/1794), [correção #1795](https://github.com/tauri-apps/wry/pull/1795) | Violação de acesso no processo da aplicação ao destruir uma janela: a liberação do controller permitia reentrada na subclassificação Win32. A correção remove a subclassificação antes de liberar o controller; entrou na [WRY 0.56.1](https://github.com/tauri-apps/wry/releases/tag/wry-v0.56.1), de 13/08/2026. | A WRY 0.55.1 local contém o caminho antigo, portanto há uma correção real para avaliar. Porém, o relato derruba o processo hospedeiro; nossos registros indicam saída do processo browser do WebView2. Não prova a causa do diálogo preto. |
| [WRY #1798](https://github.com/tauri-apps/wry/issues/1798), [correção #1799](https://github.com/tauri-apps/wry/pull/1799) | Criar WebView com foco enquanto a janela está minimizada falhava com `0x80070057`. WRY 0.56.1 passa a ignorar essa falha de foco não fatal. | A versão local está entre as afetadas, mas o erro é distinto de `C0000005`. O construtor local usa `focused(true)` e `visible(false)`; isso não demonstra a condição minimizada do relato. |
| [WebView2Feedback #5706](https://github.com/MicrosoftEdge/WebView2Feedback/issues/5706) | Relato aberto de inicialização intermitente presa esperando a conclusão de `AddScriptToExecuteOnDocumentCreated`, com mensagem Win32 sendo processada. Runtime **152.0.4191.66**, webview2-com **0.38.2**, WRY 0.54.4. Sem solução confirmada. | Correspondem Runtime e bindings. O processo permanece vivo esperando callback; é diferente da saída do browser registrada aqui. Útil se uma futura captura demonstrar espera infinita durante criação. |
| [WebView2Feedback #5704](https://github.com/MicrosoftEdge/WebView2Feedback/issues/5704) | Relato aberto de handles retidos depois de criar/fechar environment/controller repetidamente no Windows 11 e Runtime **152.0.4191.66**. Navegar antes de fechar não foi uma solução consistente. | Mesma versão do Runtime e ciclo de janelas relevante, mas o relato mede handles; não demonstra tela preta ou saída `C0000005`. |
| [Tauri #15652](https://github.com/tauri-apps/tauri/issues/15652) | Janela criada oculta deixa de receber eventos; `invoke` continua funcionando. Autor relata contornos com show/hide inicial ou polling. Permanece aberto, com pedido de reprodução mínima. | Tauri-runtime-wry **2.11.4** coincide. O relato declara ausência de crash. Não justifica aplicar show/hide como cura de um browser encerrado. |
| [Runtime 152.0.4191.53](https://learn.microsoft.com/en-us/microsoft-edge/webview2/release-notes/runtime/152) | As notas oficiais registram correção de uma falha de inicialização de aplicativos WebView2, sem assinatura ou reprodução detalhada. | O Runtime instalado **.66** já é posterior à **.53**. A nota não identifica nosso erro e não sustenta prometer que atualizar para 152 resolverá. |
| [Runtime 140.0.3485.44](https://learn.microsoft.com/en-us/microsoft-edge/webview2/release-notes/runtime/140) | Correção oficial de caixas pretas em diálogos com *visual hosting*. | Já anterior ao Runtime local; a WRY 0.55.1 cria controller associado a HWND. Similaridade visual insuficiente para atribuir a mesma causa. |

## Soluções antigas que não devem ser copiadas sem comparação

O [relato WRY #1525](https://github.com/tauri-apps/wry/issues/1525) associa falha
de inicialização à flag `RemoveRedirectionBitmap`, WRY 0.50.4 e Edge Beta
135.0.3179.18. A [mudança #1572](https://github.com/tauri-apps/wry/pull/1572)
substituiu esse caminho pela API de cor de fundo. O código local da WRY 0.55.1
já utiliza essa API e não contém a flag. Não é uma correção pendente para
reaplicar aqui.

Também existem violações de acesso causadas por lifetime de objetos no
hospedeiro: no [WebView2Feedback #5597](https://github.com/MicrosoftEdge/WebView2Feedback/issues/5597#issuecomment-4536794839),
o autor relata que manter receptores DevTools em campos resolveu sua aplicação
C#/ARM64. Esse resultado não demonstra defeito de GPU nem se transfere
automaticamente ao hospedeiro Rust/x64.

## Implicações para a próxima investigação

A recuperação aprovada a partir deste levantamento está descrita em
[Recuperação da interface após falha do WebView2](../design/0034-recuperacao-da-interface.md).
As observações do código neste levantamento correspondem ao baseline
`271f63d`, anterior a essa implementação.

Nenhuma fonte consultada fornece ainda uma correspondência exata e resolvida
para **BrowserProcessExited / Unexpected / C0000005** neste fluxo e nesta
versão do Runtime. Há correções comprovadas de criação e destruição de janelas
na WRY 0.56.1 que merecem comparação controlada, mas elas não substituem a
captura da falha do browser.

Uma atualização não é apenas trocar a versão isolada da WRY: o pacote instalado
tauri-runtime-wry 2.11.4 declara WRY `0.55.0`, e a WRY
[0.57.0](https://github.com/tauri-apps/wry/releases/tag/wry-v0.57.0) também altera
bindings Windows e webview2-com. Qualquer teste deve preservar um baseline e
verificar a compatibilidade da pilha completa.

Comparar versões do Runtime com dados descartáveis é mais informativo que
limpar o perfil do usuário. Há inclusive uma [consulta ainda aberta sobre
reutilizar perfil .66 com Fixed Runtime .62](https://github.com/MicrosoftEdge/WebView2Feedback/issues/5698);
ela não prova incompatibilidade, mas tampouco fornece uma migração validada.

O levantamento distingue correções incorporadas, contornos relatados por
autores e hipóteses ainda não confirmadas. Desabilitar GPU ou recriar um perfil
não pode ser declarado solução definitiva com base apenas nesses relatos.
