---
status: accepted
document: design
date: 2026-09-13
platform: windows
---

# Recuperação da interface após falha do WebView2

Quando o navegador ou o renderizador principal do WebView2 falha, o programa
recria a apresentação na mesma janela nativa. O Host mantém a Sessão, a revisão,
as alterações não salvas e o histórico de Desfazer. A recuperação não reabre o
arquivo do Projeto nem reinicia o processamento de imagens.

Esta decisão implementa a recuperação aprovada após a
[pesquisa sobre o diálogo preto](../research/2026-09-13-webview2-dialogo-preto.md).
A causa interna de `BrowserProcessExited / Unexpected / C0000005` permanece
em investigação. Recuperar o controle trata a consequência visível da falha;
não demonstra uma correção do defeito interno do Runtime.

## Comportamento

- O diálogo de abertura recupera o estado atual de `Preparando imagens`, com
  contagem determinada, e permanece aberto até a preparação completa.
- A janela conserva seu proprietário, tamanho e posição. A recuperação usa a
  cor de fundo do programa, sem adicionar linha temporária de estado.
- O editor reconstitui sua apresentação a partir do Host existente. Avisos da
  preparação inicial não são repetidos como se fosse uma nova abertura.
- Tela de Boas-vindas e Configurações usam a mesma recuperação do controle.
- A política permite até duas tentativas automáticas em 60 segundos por janela.
  Falha da reconstrução ou esgotamento desse limite apresenta uma mensagem
  nativa com `Tentar novamente` e `Cancelar`. Cancelar interrompe a tentativa;
  não fecha o Host nem grava o Projeto. A tentativa manual renova o limite.

As falhas de GPU e de processos auxiliares continuam sob a recuperação própria
do WebView2. O editor mantém a exigência de WebGL2. A decisão não troca a pilha
Tauri/WRY nem amplia a desativação de GPU usada pelo progresso de abertura.

## Coordenação

O callback COM apenas registra a falha e agenda a reconstrução fora de sua
execução. A espera por carregamento também ocorre fora da thread da interface.
Na queda do navegador, a reconstrução aguarda `BrowserProcessExited` do PID
correto, que confirma a liberação de seus processos filhos. O evento pode
chegar antes ou depois de `ProcessFailed`.
Identificadores monotônicos distinguem controles antigos e novos: endereços de
objetos COM podem ser reutilizados depois de fechar um controle.

A recuperação do editor reserva a mesma transição usada pelo `Salvar como`.
Se a troca de autoridade já estiver em andamento, aguarda sua conclusão e
reconfere se o controle que falhou ainda é o atual. O Monitor de Arquivos
permanece ativo durante a ausência temporária do WebView.

O controle novo recebe a URL atual, o diretório de perfil e os argumentos do
ambiente anterior. Um token de prontidão já consumido é removido da URL;
um token ainda pendente é preservado. Decisões de abertura conservam seu
identificador de tentativa e navegam pelo controle atual da janela.
Diálogos reutilizados do Projeto recebem a apresentação mais recente do Host
na URL de recuperação, evitando reapresentar uma configuração de Exportação
antiga quando a operação já está no progresso ou no resultado.

`webview_recovery_ready` indica carregamento da página e instalação da política
nativa. A conclusão da preparação das imagens e a liberação do Projeto continuam
dependendo dos sinais existentes do fluxo de abertura.

## Diagnóstico e validação

Os eventos `webview_diagnostics_ready` e `webview_process_failed` registram a
superfície, PID do navegador, versão do Runtime e `FailureReportFolderPath`.
A falha acrescenta tipo, motivo e código de saída. A existência desse caminho
não garante que o Runtime tenha criado um dump.

O teste `scripts/Run-OpeningProgressPaintGate.mjs`, com
`--fail-progress-browser`, encerra somente o navegador do diálogo pertencente
à instância de teste. A observação usa pixels nativos e exige progresso pintado,
menos de 500 ms de tela preta contínua e todos os artefatos de Cache prontos
antes da liberação do editor.

O teste `scripts/Run-WebviewRecoveryGate.mjs` altera o DPI sem salvar e provoca
duas quedas separadas do navegador do editor por `Browser.crash`. Depois de cada
queda, espera a apresentação das imagens e compara a projeção completa, o HWND
e as dimensões da janela. As quedas são separadas para não confundir o limite
automático com falhas espontâneas adicionais do Runtime; o limite e a rejeição
de notificações obsoletas têm testes próprios em Rust.

Ambos exigem um Projeto de teste com originais descartáveis e gravam em uma
raiz local isolada. Exemplo, a partir da raiz do repositório:

```powershell
node scripts/Run-WebviewRecoveryGate.mjs <executavel> <projeto-de-teste> <nova-pasta-de-evidencias> --procdump <procdump64.exe>
```

O parâmetro opcional usa o ProcDump da Microsoft, valida sua assinatura e a
identidade exata do navegador antes de anexar o diagnóstico. Não instala um
depurador global nem envia arquivos. Um dump de `Browser.crash` demonstra o
funcionamento da coleta; não identifica a causa de uma queda espontânea.
Dumps de encerramento normal também não constituem evidência dessa causa.

O progresso Win32 sem WebView permanece uma alternativa posterior. O fluxo
atual conserva os componentes visuais já aprovados.
