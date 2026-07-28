---
status: historical
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-27
updated: 2026-07-28
platform: Windows 10/11 x64
---

# Alternativas com Rust e C# para o editor de álbuns

> Documento histórico e não normativo. Esta pesquisa fundamentou a hipótese Tauri/React/Rust, mas suas topologias fixas, calibração e promessas de rollback foram substituídas pela revisão do [ADR 0005](../adr/0005-adotar-tauri-react-rust.md), pelo [ADR 0006](../adr/0006-publicar-exportacao-com-transacao-limitada.md) e pelo ticket 01.

## Resumo executivo

Aceitar investir em Rust ou C# muda materialmente a decisão do ticket 01. Electron deixa de ser a única opção de baixo risco e aparecem duas alternativas fortes:

- **WPF sobre .NET 10, com C#** é a alternativa de menor complexidade tecnológica para um produto inicialmente exclusivo de Windows que valorize integração nativa e uma stack única.
- **Tauri 2 com React/TypeScript e núcleo Rust** coloca o modelo canônico, mídia, Cache e Exportação em Rust sem assumir também o risco de construir toda a interface em um toolkit Rust.

Electron continua sendo o caminho mais rápido para uma equipe produtiva no ecossistema web. Ele também pode receber posteriormente um worker Rust sem que o restante do produto precise ser reescrito.

Uma interface completamente em Rust não é a primeira recomendação para este produto. Slint merece um protótipo se houver uma decisão estratégica de usar Rust de ponta a ponta, mas Iced e egui declaram limitações de maturidade ou estabilidade que aumentam o risco de uma interface rica. Nenhuma dessas opções resolve por si só codecs, perfis de cor, TIFF ou PDF.

## Direção vigente

Tauri 2 com React/TypeScript e Rust é a arquitetura principal da primeira versão. A escolha é estratégica: Rust possuirá o estado canônico, as regras e o pipeline de imagens, enquanto React/TypeScript fornecerá a interface. A decisão aceita explicitamente o custo de duas linguagens e dois ecossistemas.

O spike é Tauri-first e valida essa direção, não uma comparação entre implementações completas. WPF/.NET com C# permanece somente como contingência se o spike falhar em um critério declarado e um novo ADR substituir a decisão vigente.

Leitura comparativa:

| Prioridade dominante | Escolha recomendada |
|---|---|
| Entregar a primeira versão mais rapidamente | Electron + React/TypeScript, mantendo o worker substituível |
| Aplicação genuinamente Windows, com UI e integração nativas | **WPF + .NET 10/C#** |
| Domínio e renderização com forte controle de memória e concorrência | **Tauri 2 + React/TypeScript + Rust** |
| C# na UI, mas Rust somente no processamento pesado | WPF + domínio C# + `render-worker.exe` Rust separado |
| Rust em absolutamente toda a aplicação | prototipar Slint antes de assumir a stack; não adotar Iced/egui como padrão agora |

Para as prioridades confirmadas — Windows 10/11 x64 primeiro, interface complexa, isolamento entre Projetos e Rust como parte estratégica do produto — **Tauri 2 é a direção mais coerente**. WPF/C# continua sendo uma alternativa tecnicamente válida, mas não será mantida em paralelo.

## O que não muda com a linguagem

As quatro arquiteturas precisam preservar as mesmas fronteiras:

1. a interface edita uma representação canônica do Projeto;
2. Undo/Redo pertence ao domínio, e não ao Canvas;
3. a visualização usa representações reduzidas e Cache descartável;
4. a Exportação recebe um snapshot imutável e abre os arquivos originais;
5. operações finais rodam fora do processo da janela;
6. JPEG, PNG e PDF são validados por fixtures de referência, dimensões, DPI, transparência, cor e consumo de memória.

Rust reduz classes importantes de erro de memória e concorrência em código seguro. O sistema de ownership transforma vários erros concorrentes em erros de compilação ([Rust Book: concorrência](https://doc.rust-lang.org/book/ch16-00-concurrency.html)). Isso não prova a correção da diagramação nem elimina falhas de bibliotecas nativas: chamadas FFI são `unsafe` porque o compilador não consegue verificar o código externo ([Rustonomicon: Safe e Unsafe Rust](https://doc.rust-lang.org/stable/nomicon/safe-unsafe-meaning.html)).

C# também é adequado para o modelo, Undo/Redo e jobs assíncronos. A diferença prática estará mais na disciplina das fronteiras, no orçamento de memória e nos codecs escolhidos do que na linguagem isoladamente. Não há base para afirmar que WPF será sempre mais rápido que Chromium, nem que Rust será sempre mais rápido que .NET, sem medir os casos reais do editor.

## Representação canônica comum

Independentemente da stack:

- comprimentos físicos permanecem em micrômetros inteiros;
- Giro de 90° é representado em quartos de volta e o Ângulo fino é armazenado separadamente, limitado a `-45°` e `+45°`;
- espelhamentos são booleanos;
- Pan/ponto focal e Zoom adicional são valores normalizados;
- a escala mínima de preenchimento não é confundida com o Zoom do usuário;
- a conversão para pixels de tela acontece apenas na porta da visualização;
- a conversão para pixels finais acontece apenas no worker;
- cada comando criativo produz uma revisão de domínio e pode gerar uma operação inversa.

Durante um arraste, a interface pode mostrar estado transitório a cada quadro. Somente o resultado consolidado no fim do gesto entra no histórico de Undo. Isso evita enviar dezenas de comandos por segundo por IPC em Electron ou Tauri sem sacrificar a autoridade do domínio.

## A — Electron + React/TypeScript, com worker nativo opcional

### Adequação

Electron continua sendo a alternativa com menor atrito para construir muitos painéis, modais, previews, atalhos e uma superfície interativa. Ele embarca Chromium e Node.js e mantém uma base web única entre sistemas ([introdução oficial](https://www.electronjs.org/docs/latest/)).

Uma organização adequada seria:

```text
React + Canvas interativo
        |
        | comandos tipados
        v
Domínio TypeScript puro
        |
        | snapshot versionado
        v
utilityProcess ou render-worker.exe nativo
```

O Canvas pode usar DOM, Canvas 2D, WebGL ou uma biblioteca especializada escolhida no protótipo do ticket 05. O motor de tela continua sendo apenas uma visualização; ele não produz a Exportação.

### Acesso local, isolamento e segurança

Diálogos de abrir e salvar são APIs nativas do processo principal ([`dialog`](https://www.electronjs.org/docs/latest/api/dialog)). O renderer não deve receber Node.js nem uma API genérica de sistema de arquivos. O padrão obrigatório é renderer em sandbox, `contextIsolation`, `contextBridge`, CSP e IPC pequeno e validado; o guia oficial explica que habilitar Node no renderer também desabilita seu sandbox ([segurança](https://www.electronjs.org/docs/latest/tutorial/security), [isolamento de contexto](https://www.electronjs.org/docs/latest/tutorial/context-isolation)).

`utilityProcess` cria um processo filho com Node e `MessagePort`, indicado para tarefas intensivas ou sujeitas a falhas ([modelo de processos](https://www.electronjs.org/docs/latest/tutorial/process-model), [`utilityProcess`](https://www.electronjs.org/docs/latest/api/utility-process)). Um executável Rust também pode ser iniciado como sidecar. A segunda opção isola o pipeline nativo da ABI de módulos Node, ao custo de um protocolo e de mais artefatos no instalador.

Originais e bitmaps grandes não devem atravessar IPC. A UI recebe IDs, dimensões, metadados e previews; o worker resolve o caminho e lê o original.

### Imagens e PDF

Electron não fornece um renderizador profissional de álbum. Codecs, orientação EXIF, TIFF, ICC, arredondamento e PDF continuam sendo responsabilidade do worker. Um worker Rust ou C++ pode ser acrescentado somente quando o ticket 04 demonstrar necessidade; começar com um contrato de processo já torna essa troca possível.

### Undo/Redo, testes e manutenção

O domínio TypeScript pode implementar Command/Transaction, estado limpo e snapshot de Exportação sem depender de React ou Electron. Testes de domínio são rápidos e a interface se beneficia do ecossistema de testes web; a aplicação empacotada ainda precisa de testes reais no Windows.

O principal custo de longo prazo é acompanhar Chromium, Node, Electron e dependências npm. A própria documentação recomenda não bloquear os processos principal e renderer e medir CPU e memória continuamente ([desempenho](https://www.electronjs.org/docs/latest/tutorial/performance)). O instalador e a memória basal tendem a ser maiores porque o navegador é embarcado, mas em troca a versão visual é controlada pelo aplicativo.

### Quando escolher

Escolher Electron quando:

- prazo e produtividade web dominarem;
- a equipe já dominar React/TypeScript;
- consistência de uma versão fixa de Chromium for mais importante que o tamanho do pacote;
- Rust for desejado apenas no renderizador ou em otimizações comprovadas.

Não é recomendável mover também o domínio para Rust por ligação nativa desde o primeiro incremento. Isso adicionaria a fronteira de duas linguagens sem aproveitar a integração direta que Tauri já oferece.

## B — Tauri 2 + frontend web + núcleo e renderizador Rust

### Adequação

Tauri usa HTML em uma WebView do sistema e um backend Rust, conectados por mensagens ([arquitetura oficial](https://v2.tauri.app/concept/architecture/)). No Windows utiliza WebView2 baseado em Chromium; a versão é atualizada no dispositivo, e o instalador pode garantir sua presença ou uma versão mínima ([versões de WebView](https://v2.tauri.app/reference/webview-versions/), [instalador Windows](https://v2.tauri.app/distribute/windows-installer/)).

É o melhor compromisso quando se deseja bastante Rust sem abrir mão de React para a interface. A topologia escolhida é:

```text
MyAlbuns.exe
├── Boas-vindas, coordenação e operações globais
└── MyAlbuns.Imaging.exe [0..N]  temporários durante o lote exclusivo

MyAlbuns.Project.exe              um processo Tauri independente por Projeto
├── React/TypeScript              interface e interação transitória
├── PixiJS sobre WebGL2           composição da prévia interativa
└── Rust                          domínio, Undo/Redo e persistência
    └── MyAlbuns.Imaging.exe      decodificação, Cache e Exportação
```

`MyAlbuns.exe` não possui estado criativo. Cada `MyAlbuns.Project.exe` é uma aplicação Tauri independente, e não uma janela adicional de um único processo; durante a edição interativa, ela possui exatamente um Projeto e o ciclo de vida de seu `MyAlbuns.Imaging.exe`.

Durante o Modo de lote exclusivo, `MyAlbuns.exe` inicia e supervisiona Processadores temporários sem criar Janelas de Projeto. Cada um recebe somente o estado persistido de um Álbum; o paralelismo exato continua sujeito à calibração.

O núcleo Rust possui as invariantes, medidas, comandos, Undo/Redo, esquema do Projeto e criação do snapshot. A UI mantém somente o estado efêmero de interação. No fim de um gesto, envia um comando consolidado ao núcleo.

### Acesso local, isolamento e segurança

O sistema de capabilities, permissions e scopes limita quais janelas podem chamar comandos e quais caminhos ou argumentos são permitidos ([capabilities](https://v2.tauri.app/security/capabilities/), [permissions](https://v2.tauri.app/security/permissions/)). A autoridade de runtime verifica origem e capability antes de entregar a chamada ao backend ([runtime authority](https://v2.tauri.app/security/runtime-authority/)).

Essa proteção contém uma UI comprometida, mas não protege contra Rust malicioso, scopes amplos ou validação incorreta no comando — limitações declaradas pela própria documentação. O core Rust continua sendo código confiável.

O backend Tauri e a janela não devem executar a Exportação dentro do mesmo processo. Tauri empacota executáveis sidecar e permite restringir sua execução ([external binaries](https://v2.tauri.app/develop/sidecar/)). Na edição, o backend do Projeto inicia seu Processador privado; no lote, o backend de `MyAlbuns.exe` inicia os Processadores temporários. Não é necessário conceder ao frontend uma permissão genérica de shell.

### Desempenho e Canvas

No Windows, Electron e Tauri usam motores Chromium para a interface. A diferença é que Electron embarca uma versão e Tauri usa WebView2 do sistema. Portanto, Tauri não deve ser escolhido com a promessa abstrata de “Canvas nativo mais rápido”. Ele deve ser escolhido pelo backend Rust, pelo pacote potencialmente menor e pelo modelo de capabilities.

Na primeira versão, o Processador de Imagens decodifica os originais e produz uma representação visual reduzida e imutável por mídia; PixiJS sobre WebGL2 a compõe na prévia interativa acelerada por hardware. Tiles somente serão adotados se as medições do spike demonstrarem necessidade. WebGL2 é requisito do editor, e a criação do contexto sozinha não comprova aceleração. O diagnóstico precisa confirmar um backend de hardware; contexto ausente, rasterizador de software ou identificação inconclusiva preservam Boas-vindas e Configurações, mas impedem a abertura do editor. Não haverá fallback de edição por Canvas 2D ou software.

O pipeline de mídia Rust pode:

- limitar concorrência e memória explicitamente;
- produzir representações reduzidas e, se futuramente medido como necessário, tiles;
- manter o original fora da WebView;
- reiniciar um job que falhou sem encerrar a janela;
- compartilhar o mesmo núcleo com uma futura versão para outro sistema.

O crate oficial `image` oferece codecs comuns, incluindo JPEG, PNG e TIFF ([repositório `image-rs/image`](https://github.com/image-rs/image)). Isso comprova viabilidade básica, não qualidade de impressão. Perfis ICC, CMYK, metadados, compressão JPEG e escrita de PDF ainda precisam de spike e testes. Adotar bibliotecas C/C++ por FFI reintroduz uma fronteira `unsafe`.

WebGPU no frontend e `wgpu` no pipeline Rust são decisões diferentes e ambas ficam adiadas. A base da primeira versão é PixiJS/WebGL2 para a prévia e um caminho Rust inicialmente orientado a CPU para processamento final, até medições justificarem outra solução.

### Testes, empacotamento e curva

Tauri oferece runtime mock para testes unitários e de integração e WebDriver para testes ponta a ponta ([testes](https://v2.tauri.app/develop/tests/), [WebDriver](https://v2.tauri.app/develop/tests/webdriver/)). No Windows, produz `.msi` com WiX ou `setup.exe` com NSIS e permite configurar como WebView2 é entregue ([instalador Windows](https://v2.tauri.app/distribute/windows-installer/)). A primeira distribuição usará WebView2 Evergreen e verificará sua disponibilidade.

Os custos são:

- duas linguagens e dois ecossistemas desde o início;
- toolchain Rust e Microsoft C++ Build Tools;
- serialização e contratos explícitos entre frontend e backend;
- WebView2 Evergreen mudando independentemente do aplicativo;
- necessidade de depurar tanto a UI web quanto o core Rust.

### Quando escolher

Escolher Tauri quando:

- aprender e manter Rust for uma decisão estratégica;
- domínio, mídia, lote e Exportação forem mais importantes que minimizar o tempo até o primeiro protótipo;
- um futuro motor reutilizável fora do desktop web tiver valor;
- aceitar React/TypeScript na interface não contrariar a intenção de usar Rust.

Esta é a opção recomendada se “trabalhar mais em Rust” significar que Rust deverá possuir o coração do produto.

## C — .NET nativo

### Recomendação dentro do ecossistema .NET

Para este produto, a ordem é:

1. **WPF sobre .NET 10**;
2. Avalonia somente se multiplataforma se tornar requisito real;
3. WinUI 3 quando o visual Fluent mais recente for mais importante que a maturidade do Canvas e a simplicidade de implantação.

O .NET 10 é LTS e está em suporte ativo até 14 de novembro de 2028 ([política oficial](https://dotnet.microsoft.com/en-us/platform/support/policy)). Uma publicação `win-x64` pode incluir o runtime e usar single-file/ReadyToRun conforme o perfil de distribuição ([`dotnet publish`](https://learn.microsoft.com/en-us/dotnet/core/tools/dotnet-publish), [single-file](https://learn.microsoft.com/en-us/dotnet/core/deploying/single-file/overview)).

### WPF

WPF possui renderização vetorial independente de resolução, coordenadas e transformações em dupla precisão, aceleração por hardware, clipping, composição alpha e hit testing ([visão geral](https://learn.microsoft.com/en-us/dotnet/desktop/wpf/overview/)). Sua camada visual é retained-mode e `DrawingVisual` é uma primitiva leve para imagens, formas e texto sem o custo de layout e eventos de um controle completo ([renderização gráfica](https://learn.microsoft.com/en-us/dotnet/desktop/wpf/graphics-multimedia/wpf-graphics-rendering-overview)).

Isso combina com o editor, desde que a implementação não crie milhares de `UIElement`s. A estratégia recomendada é:

- controles XAML para menus, painéis, propriedades e modais;
- uma superfície customizada para a Lâmina;
- `DrawingVisual`s ou outro backend medido para a cena;
- índice espacial próprio para hit testing;
- tiles e previews em resoluções adequadas ao Zoom;
- adorners ou uma camada pequena para seleção e alças.

O WPF já oferece hit testing na camada visual e pode devolver todos os objetos sob um ponto ou geometria ([hit testing](https://learn.microsoft.com/en-us/dotnet/desktop/wpf/graphics-multimedia/hit-testing-in-the-visual-layer)). Ainda assim, a aplicação deve manter o modelo de seleção no domínio, não na árvore visual.

O Windows Imaging Component fornece codecs nativos para JPEG, PNG e TIFF, metadados EXIF/IPTC/XMP, color contexts e uma variedade de formatos RGB, CMYK e alta profundidade ([WIC](https://learn.microsoft.com/en-us/windows/win32/wic/-wic-lh), [codecs nativos](https://learn.microsoft.com/en-us/windows/win32/wic/-wic-about-windows-imaging-codec), [pixel formats e color context](https://learn.microsoft.com/en-us/windows/win32/wic/-wic-codec-native-pixel-formats)). WPF expõe descoberta de codecs, frames, metadados e perfis por `BitmapDecoder` ([`BitmapDecoder`](https://learn.microsoft.com/en-us/dotnet/api/system.windows.media.imaging.bitmapdecoder)).

Isso é uma vantagem real no Windows, mas não encerra o ticket 04. A Exportação precisa definir conversão de cor, qualidade JPEG, política de metadados e comportamento diante de codecs adicionais. O caminho de documento nativo do WPF é XPS, não uma API de PDF do produto ([visão geral de impressão](https://learn.microsoft.com/en-us/dotnet/desktop/wpf/documents/printing-overview)); portanto, PDF requer uma biblioteca ou escritor explícito e os mesmos testes de fidelidade.

### Arquitetura WPF recomendada

```text
MyAlbuns.exe
        |
        +-- <Nome do Projeto>
                |
                +-- MyAlbuns.Project.exe
                |       Janela do Projeto: WPF/XAML, domínio, Undo/Redo e Salvamento
                |
                +-- MyAlbuns.Imaging.exe
                        Processador de Imagens: Cache, previews e Exportação
```

Nomenclatura canônica da arquitetura:

- **MyAlbuns** é o processo principal e único por usuário; coordena abertura, processos, recursos e operações globais sem possuir o estado criativo dos Projetos.
- **Janela do Projeto** é o processo associado ao Nome real de um Projeto e reúne sua interface WPF, domínio C#, Histórico da sessão e persistência.
- **Processador de Imagens** é o processo isolado de Cache, previews, metadados e Exportação. “Worker” pode aparecer em detalhes internos de implementação, mas não é o nome usado para apresentar a arquitetura.

A superfície principal visível do `MyAlbuns.exe` é a **Tela de Boas-vindas**, não um editor de Projeto. Ela concentra entrada no aplicativo, com `Novo Projeto`, `Abrir Projeto`, `Projetos recentes`, `Exportação em lote`, `Configurações` e `Ajuda`. Abrir diretamente um arquivo pelo sistema operacional pode criar sua Janela do Projeto sem exibir antes a Tela de Boas-vindas.

Mesmo quando a Tela de Boas-vindas não estiver visível, o `MyAlbuns.exe` continua ativo como coordenador e pode hospedar diálogos globais, como progresso de lote e calibração. A Geração de Projetos em lote permanece na Janela do Projeto porque usa o estado visível daquele Projeto como modelo.

O processo principal também coordena as mutações do catálogo global de Layouts personalizados. Ele serializa criação e exclusão, persiste a nova versão e a distribui imediatamente às Janelas de Projeto conectadas. Cada Janela atualiza um Painel de Layouts visível ou usa a nova versão na próxima abertura, sem transformar essa sincronização em mudança do Projeto ou entrada de Undo/Redo.

Ao abrir o primeiro Projeto, a Tela de Boas-vindas é ocultada, sem encerrar o `MyAlbuns.exe`. Uma Janela do Projeto pode exibi-la novamente por uma ação `Tela de Boas-vindas`. Quando a última Janela do Projeto fecha, a Tela reaparece; fechá-la sem Projetos abertos encerra normalmente o processo principal. Encerrar todas as Janelas e o processo principal enquanto existem Projetos abertos continua exigindo o comando explícito `Sair do MyAlbuns`.

Existe somente um `MyAlbuns.exe` por sessão do Windows. Uma execução posterior encaminha sua ativação à instância existente e termina, em vez de criar outra Tela de Boas-vindas ou outro coordenador. Uma ativação pode solicitar apenas a Tela de Boas-vindas ou carregar um ou vários caminhos recebidos do Explorador.

Para cada caminho, a instância existente resolve a Identidade: se a sessão correspondente já estiver aberta, focaliza sua Janela; caso contrário, inicia uma nova `MyAlbuns.Project.exe`. Projetos com Identidades diferentes continuam em processos e janelas separados, inclusive quando vários arquivos são abertos na mesma ativação do sistema operacional.

Dentro da Janela do Projeto, a dependência principal permanece:

```text
WPF/XAML + superfície de edição customizada
        |
        | Command/Transaction em memória
        v
Domínio C# puro, sem dependência de WPF
        |
        | snapshot versionado
        v
MyAlbuns.Imaging.exe
```

Cada Projeto aberto possui uma Janela do Projeto (`MyAlbuns.Project.exe`) e um Processador de Imagens (`MyAlbuns.Imaging.exe`). O estado canônico, as regras, o Undo/Redo, o Salvamento e a Recuperação permanecem na Janela do Projeto, organizados em bibliotecas C# puras sem dependência de WPF. Não existe um terceiro processo de motor C#.

Uma falha acidental do `MyAlbuns.exe` não encerra as Janelas de Projeto nem descarta seu estado criativo não salvo. Para isso, elas são processos independentes e não pertencem a um Job Object configurado para encerrar toda a árvore quando o processo principal termina. Enquanto o processo principal estiver indisponível, cada Janela continua permitindo edição, Undo/Redo, Salvamento e fechamento local; abertura de novos Projetos, Exportação normal, Exportação em lote e outras operações globais permanecem indisponíveis. Os Processadores de Imagens detectam a perda da coordenação e pausam em um ponto seguro.

As Janelas detectam a desconexão do IPC, elegem somente uma delas para reiniciar o `MyAlbuns.exe` e depois se registram novamente, informando sua Identidade de Projeto e operações ainda existentes. O novo processo principal reconstrói a lista de Projetos, as permissões de recursos e os bloqueios globais antes de liberar novamente Cache e Exportação. Se a recuperação automática falhar, o estado local continua acessível para Salvar ou fechar, acompanhado de uma mensagem clara. Pipes nomeados atendem ao IPC local duplex e uma primitiva nomeada do sistema operacional impede reinicializações concorrentes.

O encerramento intencional por `Sair do MyAlbuns` segue outro protocolo: o processo principal solicita o fechamento das Janelas de Projeto, e cada sessão com alterações pendentes oferece as escolhas de Salvamento já definidas. Somente esse fluxo coordenado encerra toda a aplicação; a queda isolada do processo principal nunca é interpretada como pedido para fechar os Projetos.

Se uma `MyAlbuns.Project.exe` encerrar inesperadamente, o `MyAlbuns.exe` isola a falha: encerra o Processador de Imagens associado, revoga suas permissões, libera qualquer bloqueio global de sua propriedade e mantém as demais Janelas funcionando. O Projeto não é reaberto automaticamente, evitando um ciclo quando a mesma sessão reproduz a falha.

O aviso oferece `Reabrir e recuperar`, `Abrir última versão salva` e `Agora não`. A primeira opção cria uma nova Janela do Projeto a partir do estado temporário, ainda marcado como não salvo. A segunda exige confirmação antes de descartar a recuperação e abrir o arquivo persistido. `Agora não` fecha o aviso sem remover a recuperação, que reaparece na próxima abertura daquele Projeto.

A Recuperação de sessão é atualizada depois de cada comando criativo concluído, nunca durante o estado transitório de um gesto. Uma pequena postergação consolida comandos muito próximos e evita escritas redundantes; ao final de um arraste ou redimensionamento, somente o resultado consolidado entra no checkpoint. A substituição do temporário é atômica e não modifica o arquivo do Projeto.

Se a Janela cair no meio de um gesto, a Recuperação retorna ao último comando concluído, descartando apenas a interação ainda incompleta. Depois de `Salvar`, sem novas mudanças pendentes, ou de um fechamento normal confirmado, o temporário é removido. Portanto, o mecanismo reduz perda de trabalho sem se transformar em Salvamento automático.

O checkpoint inclui o estado consolidado, as pilhas disponíveis de Undo e Redo e o marco da última versão salva. Recuperar uma falha continua a mesma sessão lógica e permite desfazer ações anteriores à queda; somente um fechamento normal encerra o Histórico. Estado e Histórico possuem validação separada: se apenas o Histórico estiver ausente ou corrompido, o conteúdo visual ainda é restaurado como não salvo, Undo/Redo começam vazios e o usuário recebe um aviso.

O Histórico de cada Projeto possui orçamento automático de memória e não cresce indefinidamente. Comandos armazenam deltas estruturais e referências, nunca pixels, Cache ou cópias dos arquivos originais. Quando o orçamento é alcançado, as entradas de Undo mais antigas são descartadas primeiro; estado atual, pilha de Redo ainda válida e capacidade de continuar editando permanecem independentes dessa limpeza.

O marco de Salvamento e a indicação de mudanças pendentes também não dependem da presença da ação correspondente no Histórico. A Recuperação persiste somente as entradas ainda disponíveis. O orçamento inicial será definido por medições do spike e não aparece como configuração manual na primeira versão.

O Processador de Imagens é outro processo C#/.NET e recebe jobs e snapshots imutáveis e versionados. Ele possui modos exclusivos de Cache e Exportação: no mesmo Projeto, iniciar uma Exportação pausa o Cache em um ponto seguro e a fila é retomada posteriormente. .NET fornece processos e pipes nomeados para IPC duplex ([pipes em .NET](https://learn.microsoft.com/en-us/dotnet/standard/io/how-to-use-named-pipes-for-network-interprocess-communication)). No Windows, um Job Object pode impor limites e encerrar a árvore do job ([Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects)).

Se o Processador de Imagens encerrar inesperadamente durante o Cache, a Janela do Projeto o reinicia automaticamente. O item incompleto é descartado e os pedidos ainda relevantes são reconstruídos a partir do estado canônico e dos arquivos originais; nenhum resultado parcial do Cache altera o Projeto. A Janela permanece editável durante a recuperação. As reinicializações são limitadas para evitar um ciclo infinito: falhas isoladas são recuperadas sem modal, enquanto falhas repetidas suspendem o Cache e exibem um aviso claro, sem impedir edição ou Salvamento.

Cada Processador de Imagens pode executar código C# em paralelo real em múltiplos núcleos, sem uma trava global equivalente ao GIL do CPython. O paralelismo interno, porém, é sempre limitado por filas com capacidade, permissões de CPU e orçamento de memória; não se cria uma thread por Foto ou por saída. O .NET oferece paralelismo multicore, `Channel<T>` limitado e primitivas de coordenação para essa finalidade ([programação paralela](https://learn.microsoft.com/en-us/dotnet/standard/parallel-programming/), [filas limitadas](https://learn.microsoft.com/en-us/dotnet/core/extensions/queue-service)).

O processo MyAlbuns distribui permissões entre todos os Processadores de Imagens dos Projetos abertos. Nenhum deles considera todos os núcleos ou toda a memória como exclusivamente seus. Exportações têm prioridade; Cache utiliza a capacidade restante, pode ser preemptado e um Processador ocioso não mantém trabalho de CPU. Os limites iniciais de threads, jobs e memória serão definidos pelo spike, não fixados por suposição.

Para a Exportação normal, o coordenador concede uma única exclusividade global e não mantém fila de espera. Enquanto um Projeto exporta, as ações de Exportação normal das demais janelas ficam indisponíveis, mas suas Janelas de Projeto continuam utilizáveis. Somente a janela proprietária mostra o modal de progresso e fica bloqueada por ele; a exclusividade é liberada em qualquer estado terminal do job.

Uma Exportação normal prepara toda a tentativa em uma área temporária isolada e só publica o novo conjunto no destino depois que todas as saídas forem renderizadas e verificadas. Se o Processador de Imagens cair, a operação falha sem repetição automática: o Processador é reiniciado para trabalhos posteriores, o bloqueio global é liberado, a tentativa temporária é removida e o modal oferece `Tentar novamente` ou `Fechar`. A Exportação anterior, inclusive candidatos órfãos, permanece intacta.

Após sucesso integral da preparação, a publicação substitui somente o conjunto autorizado e executa a limpeza de órfãos aplicável como uma transação com recuperação diante de falha. Nenhum artefato auxiliar persistente é deixado na pasta final. Essa garantia consome temporariamente espaço adicional equivalente à nova saída, aceito em troca de nunca expor um Álbum parcialmente atualizado.

Durante a Exportação em lote, o coordenador assume um Modo de lote exclusivo mais forte: desabilita todas as janelas de Projeto, pausa Cache e outros trabalhos de fundo e mantém interativa somente a janela global de progresso e cancelamento. Estados abertos, inclusive alterações não salvas, permanecem intactos; cada item do lote continua sendo lido exclusivamente de seu arquivo persistido. Conclusão, falha ou cancelamento libera as janelas e permite retomar o Cache.

Cada Álbum do lote usa a mesma preparação temporária isolada da Exportação normal. Se o Processador de Imagens encerrar inesperadamente, o coordenador descarta aquela tentativa, reinicia o Processador e repete automaticamente o Álbum uma única vez. Erros determinísticos — como validação, mídia ausente ou espaço insuficiente — não são repetidos. Uma segunda queda marca somente aquele Álbum como falha e não interrompe os demais. A tentativa adicional aparece no progresso e no resumo final, e a saída anterior do item permanece intacta até uma conclusão bem-sucedida.

O `MyAlbuns.exe` persiste atomicamente um registro temporário de coordenação do lote em `%LOCALAPPDATA%\MyAlbuns\Recovery\Batches\{batch-id}.json`. Ele contém opções, itens descobertos, estados, tentativas consumidas e identificadores de preparação, mas nenhum conteúdo criativo de Projeto e nenhum manifesto no destino. Processadores que perdem o coordenador interrompem o trabalho em ponto seguro e nunca publicam uma preparação incompleta.

Após reiniciar, o `MyAlbuns.exe` reconcilia o registro com as publicações concluídas e apresenta `Lote interrompido`, sem continuar silenciosamente. `Retomar` reativa o Modo de lote exclusivo e processa apenas itens incompletos, preservando as tentativas já consumidas. `Encerrar` remove preparações incompletas, mantém os Álbuns já publicados e libera as Janelas de Projeto. O registro temporário é removido somente depois do sucesso integral, do cancelamento concluído ou da escolha `Encerrar`.

O resultado da calibração de paralelismo será um Perfil de desempenho por usuário em `%LOCALAPPDATA%\MyAlbuns\State\performance-profile.json`. Ele permanece fora da pasta protegida de instalação e de qualquer Projeto, sobrevive a atualizações comuns do aplicativo e pode ser recriado sem afetar conteúdo criativo. A quantidade de Álbuns simultâneos é automática e não aparece como configuração no diálogo inicial.

Quando não existe um Perfil válido, a primeira Exportação em lote entra no Modo de lote exclusivo, apresenta uma fase de calibração no próprio modal, grava o resultado atomicamente e só então inicia as saídas. Lotes posteriores reutilizam o perfil. Cancelar durante essa fase encerra o lote antes de qualquer saída.

O Perfil é invalidado por mudança material da CPU ou memória física, alteração explícita da versão do mecanismo de calibração ou arquivo ausente, corrompido ou incompatível. Atualizações comuns do aplicativo ou Windows, troca de Projeto e mudança de destino não exigem novo teste. Pressão transitória de memória ou disco faz o coordenador reduzir a concorrência somente naquela execução.

Uma página `Configurações > Desempenho` mostra somente a data da calibração, um resumo do hardware, o limite automático e a ação `Recalibrar`. Não expõe processos, threads ou orçamento de memória como ajustes. A Recalibração manual exige confirmação, executa imediatamente no Modo de lote exclusivo e preserva o Perfil anterior se o novo teste falhar ou for cancelado.

Separar por processo protege a responsividade e contém crashes, mas não cria por si só um sandbox de segurança. Aplicações WPF standalone executam com permissões irrestritas do usuário ([segurança do WPF](https://learn.microsoft.com/en-us/dotnet/desktop/wpf/security-wpf)). Caminhos, snapshots e destinos precisam ser validados.

Se Rust for desejado apenas no pipeline final, o melhor híbrido é um `render-worker.exe` Rust com protocolo versionado. Isso mantém FFI fora do processo da UI e permite que uma falha nativa seja reiniciada. Não é recomendável dividir cada operação interativa entre WPF/C# e Rust; o custo cognitivo e de IPC seria alto para pouco ganho.

### WinUI 3

WinUI 3 é o framework nativo moderno recomendado pela Microsoft para novas aplicações Windows, roda em Windows 10 1809 ou posterior e faz parte do Windows App SDK ([início com WinUI](https://learn.microsoft.com/en-us/windows/apps/get-started/winui-get-started-overview)). Para desenho 2D, Win2D oferece `CanvasControl` immediate-mode, imagens, máscaras, efeitos e render target ([tutorial Win2D](https://learn.microsoft.com/en-us/windows/apps/develop/win2d/quick-start), [`CanvasControl`](https://microsoft.github.io/Win2D/WinUI3/html/T_Microsoft_Graphics_Canvas_UI_Xaml_CanvasControl.htm)).

É tecnicamente capaz, mas traz mais trabalho para este editor:

- a cena, invalidação e hit testing seriam mais próprios da aplicação;
- Win2D exige alvo x64/x86 específico, não `Any CPU`;
- testes que usam XAML precisam de um processo e thread de UI ([testes WinUI](https://learn.microsoft.com/en-us/windows/apps/winui/winui3/testing/));
- o Windows App SDK é um runtime separado que precisa ser instalado ou incluído, com opções MSIX, unpackaged e self-contained ([implantação](https://learn.microsoft.com/en-us/windows/apps/windows-app-sdk/deploy-unpackaged-apps)).

WinUI deve vencer WPF somente se o design Fluent atual e APIs mais novas do Windows forem requisitos prioritários ou se um protótipo provar vantagem concreta no Canvas.

### Avalonia

Avalonia 12 é retained-mode, usa uma cena própria e fornece custom rendering, hit testing e plataforma headless com captura de pixels para regressão visual ([arquitetura](https://docs.avaloniaui.net/docs/fundamentals/architecture), [hit testing](https://docs.avaloniaui.net/docs/graphics-animation/hit-testing), [testes headless](https://docs.avaloniaui.net/docs/testing/setting-up-the-headless-platform)). É uma opção tecnicamente forte caso macOS/Linux deixem de ser apenas possibilidades futuras.

O problema imediato é suporte: na matriz atual, Windows 10 22H2 x64 é Tier 2, e builds anteriores de Windows 10 são Tier 3; Tier 1 começa no Windows 11 24H2 ([plataformas suportadas](https://docs.avaloniaui.net/docs/supported-platforms)). Para um produto que promete Windows 10/11, isso aumenta o risco ou pode exigir suporte comercial. Avalonia também não acrescenta uma vantagem suficiente enquanto somente Windows estiver no escopo.

### Comparação interna do .NET

Escala de 1 a 5, em que 5 é melhor para este produto:

| Critério | WPF | WinUI 3 | Avalonia 12 |
|---|---:|---:|---:|
| Superfície de edição 2D madura | 5 | 4 | 4 |
| Integração e aparência Windows | 5 | 5 | 3 |
| Compatibilidade com Windows 10 | 5 | 5 | 2 |
| Simplicidade de implantação | 4 | 3 | 4 |
| Testes visuais/headless | 3 | 3 | 5 |
| Caminho futuro multiplataforma | 1 | 1 | 5 |
| Risco total na primeira versão | **baixo** | médio | médio |

## D — interface completamente em Rust

### Slint

Slint é a única opção totalmente Rust que merece um protótipo formal neste momento. Possui UI declarativa compilada, API 1.x estável, renderizadores Skia/FemtoVG/software e declara suporte testado a Windows 10 x64 e Windows 11 ([repositório oficial](https://github.com/slint-ui/slint), [plataformas desktop](https://docs.slint.dev/latest/docs/slint/guide/platforms/desktop/)).

Entretanto, o editor exigiria validar cedo:

- integração de uma grande superfície customizada;
- Pan/Zoom, clipping, seleção e hit testing;
- docking, atalhos, IME e acessibilidade;
- visualização de muitas imagens por tiles;
- testes de interação e regressão;
- integração com worker e diálogos do Windows.

O acesso Rust a WGPU é marcado como instável, enquanto a API pública do renderer Skia é C++ ([backends e renderers](https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backends_and_renderers/), [features Rust](https://docs.slint.dev/latest/docs/rust/slint/docs/cargo_features/)). Isso não impede o produto, mas torna a superfície do editor um risco maior que em React, WPF ou Avalonia. A licença royalty-free para aplicações proprietárias possui condições próprias e deve ser revisada antes da adoção ([licenciamento no repositório](https://github.com/slint-ui/slint#license)).

### Iced e egui

Iced possui renderizadores `wgpu` e `tiny-skia`, widgets customizáveis e um modelo inspirado em Elm, mas seu próprio repositório o classifica como software experimental ([repositório oficial](https://github.com/iced-rs/iced)).

egui é atraente para ferramentas técnicas e custom painting, porém o projeto declara desenvolvimento ativo, interfaces em fluxo, mudanças incompatíveis e ausência de objetivo de aparência nativa ([repositório oficial](https://github.com/emilk/egui)). Isso é aceitável para um painel interno ou protótipo, mas não reduz o risco de um produto desktop centrado em UX.

### Conclusão sobre UI Rust

Rust é mais valioso aqui no **núcleo e no pipeline**, onde ownership, concorrência e controle de memória atacam problemas reais. Usá-lo também em todos os controles da UI troca ecossistemas maduros por mais trabalho em widgets, automação, acessibilidade e convenções do Windows sem melhorar automaticamente a fidelidade da Exportação.

## Matriz de decisão principal

As notas são julgamento arquitetural para os requisitos atuais, não benchmark. Escala de 1 a 5; o total máximo ponderado é 500.

| Critério | Peso | Electron + TS | Tauri + Rust | WPF + C# | Slint + Rust |
|---|---:|---:|---:|---:|---:|
| UI rica e Canvas interativo | 20 | 5 | 5 | 5 | 3 |
| Velocidade inicial e tooling | 15 | 5 | 3 | 4 | 2 |
| Integração Windows | 10 | 4 | 4 | 5 | 4 |
| Domínio/render de alta confiabilidade | 15 | 4 | 5 | 4 | 5 |
| Isolamento de jobs longos | 10 | 5 | 4 | 5 | 4 |
| Ecossistema de imagem/PDF | 10 | 3 | 4 | 4 | 4 |
| Testabilidade | 10 | 5 | 4 | 4 | 3 |
| Fronteira de segurança da UI | 5 | 3 | 5 | 3 | 4 |
| Empacotamento e manutenção | 5 | 3 | 4 | 4 | 4 |
| **Total ponderado** | **100** | **435 (87%)** | **425 (85%)** | **435 (87%)** | **355 (71%)** |

O empate numérico entre Electron e WPF expressa duas otimizações diferentes:

- Electron maximiza velocidade e tooling da UI;
- WPF maximiza alinhamento com Windows e reduz a quantidade de runtimes e linguagens da arquitetura.

Tauri fica muito próximo e vence quando “núcleo Rust” recebe peso maior. Slint só sobe na matriz se “100% Rust” for uma restrição do produto, e não uma preferência.

## Arquiteturas que devem ser evitadas

### Domínio duplicado em TypeScript e Rust

Não manter duas implementações das mesmas regras. A interface pode ter projeções e estado transitório, mas apenas uma linguagem deve possuir as invariantes e gerar o snapshot.

### Rust por FFI dentro da janela sem necessidade

Uma DLL Rust carregada por Electron ou WPF compartilha o destino do processo. Erros de FFI ou biblioteca nativa podem derrubar a UI. Um processo separado é uma fronteira mais simples de testar, atualizar e reiniciar.

### Exportar capturando o Canvas

Canvas, WPF Visual, Win2D ou Slint devem produzir apenas a prévia. A Exportação reabre originais e executa o pipeline canônico.

### Fazer IPC por movimento do ponteiro

Pan, resize e drag precisam de feedback local imediato. O domínio recebe uma transação consolidada no fim do gesto e devolve o estado canônico.

## Sequência de validação da decisão

Antes do desenvolvimento amplo, o ticket 01 deve produzir um spike vertical em Tauri 2, React/TypeScript e Rust:

1. uma Lâmina com duas fotos grandes, dois Frames, máscara, Pan, Zoom e Overlay;
2. interação de arraste contínua sem travar a UI;
3. estado transitório no frontend, commit consolidado no Rust e Undo/Redo canônico;
4. diálogo nativo para escolher as fotos;
5. contrato explícito entre React/TypeScript e o backend Rust, com capabilities e scopes mínimos;
6. `MyAlbuns.Imaging.exe` Rust separado que gera Cache e exporta JPEG usando os originais;
7. um Processador interativo pertencente à Janela do Projeto e um Processador temporário de lote pertencente a `MyAlbuns.exe`, sem abrir uma Janela de Projeto;
8. pausa do Cache durante a Exportação e retomada posterior;
9. encerramento forçado do Processador de Imagens durante o Cache, seguido de reinício automático e reconstrução dos pedidos relevantes sem derrubar ou corromper o Projeto;
10. encerramento forçado do Processador de Imagens durante a Exportação normal, preservando a saída anterior, removendo a tentativa temporária e liberando o bloqueio sem repetição automática;
11. múltiplos Projetos em aplicações `MyAlbuns.Project.exe` independentes;
12. encerramento forçado de uma Janela do Projeto sem afetar as demais, seguido das três escolhas explícitas de recuperação;
13. encerramento forçado e reinício do `MyAlbuns.exe`, preservando as Janelas, o estado não salvo e a capacidade de Salvar;
14. registro novamente de todas as Janelas após a recuperação, reconstruindo permissões e bloqueios globais sem iniciar duas instâncias do processo principal;
15. perda e restauração do contexto WebGL2 durante a sessão;
16. limites de textura, tiling e pressão de memória gráfica com Fotos grandes;
17. diagnóstico com contexto ausente, rasterizador de software forçado e backend inconclusivo, mantendo Boas-vindas e Configurações acessíveis;
18. Exportação de uma Lâmina de `60 × 24 cm` em 300 DPI;
19. teste ponta a ponta, WebView2 Evergreen e instalador `win-x64` em uma máquina limpa.

Medir:

- tempo até primeira interação;
- latência de Pan/Zoom;
- vazão da geração de Cache;
- responsividade durante Cache e Exportação;
- memória com originais grandes e múltiplos Projetos;
- tempo e pico de memória da Exportação;
- comportamento após queda e reinício do Processador de Imagens;
- comportamento após queda, eleição, reinício e reconexão do processo principal;
- complexidade para automatizar o fluxo;
- tamanho e comportamento do instalador;
- complexidade total da solução.

Os itens funcionais são gates binários. Cada execução registra o hardware usado junto às métricas. As primeiras medições são exploratórias; em seguida, o relatório congela os perfis mínimo/recomendado, as metas quantitativas e a margem de correção antes de uma execução final de aceitação. A arquitetura não inventa antecipadamente números de CPU, RAM ou armazenamento e o resultado final não pode reajustar os limites retroativamente.

Se um gate funcional for impossível ou a execução final falhar nas metas congeladas, a evidência deve ser registrada. A pessoa responsável pelo produto decide entre corrigir dentro da margem registrada ou substituir o ADR antes de iniciar a contingência WPF/C#. Não serão construídas duas versões completas em paralelo.

## Recomendação final

Adotar **Tauri 2 com React/TypeScript e Rust** como direção principal. O Processador gera os artefatos de visualização e PixiJS/WebGL2 compõe a prévia; o Rust de cada `MyAlbuns.Project.exe` possui o estado canônico; `MyAlbuns.Imaging.exe` executa Cache e Exportação isoladamente, sempre reabrindo os originais no caminho final.

Electron, WPF e as interfaces totalmente Rust permanecem como alternativas avaliadas, não como implementações paralelas. WPF/.NET com C# é a contingência imediata. A decisão e seus critérios estão no [ADR 0005](../adr/0005-adotar-tauri-react-rust.md).
