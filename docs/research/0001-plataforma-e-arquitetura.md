---
status: superseded
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-27
---

# Plataforma e arquitetura do editor desktop

> Esta recomendação inicial foi substituída primeiro pela avaliação WPF/C# de `0002-alternativas-rust-csharp.md` e, depois, pela decisão Tauri/React/Rust do [ADR 0005](../adr/0005-adotar-tauri-react-rust.md). O documento permanece como histórico da avaliação inicial de Electron e Tauri.

## Resumo executivo

A recomendação é usar **Electron com TypeScript** para a primeira versão, mantendo três partes rigorosamente separadas:

1. uma interface web para painéis e Canvas interativo;
2. um núcleo de domínio independente de Electron, DOM e sistema de arquivos;
3. um processo de renderização final isolado, que lê os arquivos originais e nunca reutiliza o Canvas ou o Cache como fonte da Exportação.

Electron não foi escolhido para renderizar o produto final. Ele foi escolhido para entregar com menor risco a interface complexa do editor, integração desktop e testes, usando a ferramenta já disponível no ambiente. O renderizador final será um módulo separado e substituível, cuja biblioteca de imagem e política de cor/compressão continuam sendo decisões do ticket 04.

Tauri 2 é a segunda opção. Sua aplicação menor e o backend Rust são atraentes, mas não eliminam a necessidade de um pipeline de imagem próprio e introduzem dois custos imediatos: o toolchain Rust ainda não está instalado e o runtime visual muda entre WebView2 no Windows e WebKit no macOS/Linux. Qt 6 é tecnicamente competitivo, sobretudo para uma equipe experiente em C++, mas tem o maior custo de adoção neste repositório e exige uma decisão consciente de licenciamento.

## Plataforma inicial confirmada

O usuário confirmou como plataforma inicial:

- **primeira versão para Windows 10 e 11, arquitetura x64**;
- distribuição por instalador próprio, fora da Microsoft Store;
- macOS como possibilidade futura, não como requisito da primeira entrega;
- Linux fora do compromisso inicial de produto.

Essa plataforma foi confirmada em 2026-07-27. A escolha entre Electron e suas alternativas ainda precisa ser aceita antes de transformar a recomendação em ADR. Electron fornece binários para Windows 10 ou superior, macOS e Linux, mas cada plataforma adicional cria trabalho próprio de assinatura, empacotamento, testes de GPU, diálogos e integração com arquivos ([suporte de plataformas do Electron](https://github.com/electron/electron#platform-support)).

Se macOS e Linux precisarem fazer parte da primeira entrega, Electron continua sendo a recomendação preliminar pela uniformidade do Chromium embarcado, mas a matriz de testes e o orçamento de distribuição precisam ser ampliados. Se tamanho mínimo do instalador for um requisito dominante e a equipe aceitar Rust, Tauri deve ser reavaliado.

## Necessidades arquiteturais extraídas do produto

A plataforma deve suportar simultaneamente:

- edição fluida de geometria, seleção, Pan, Zoom e máscaras em uma Lâmina;
- várias imagens grandes vinculadas a arquivos externos;
- prévias descartáveis para interação, sem confundi-las com os originais;
- Exportação JPEG, PNG e PDF em dimensões físicas e DPI definidos;
- operações longas e em lote com progresso, cancelamento e isolamento de falhas;
- salvamento explícito, estado sujo, Undo/Redo e recuperação separada do arquivo do Projeto;
- diálogos nativos, caminhos locais, bloqueio de abertura e múltiplos Projetos independentes;
- evolução posterior do renderizador sem reescrever a interface ou o modelo do Projeto.

O maior risco não é desenhar retângulos no Canvas. É permitir que a prévia, a persistência e a Exportação interpretem a mesma composição de maneiras diferentes. Por isso, a representação canônica deve pertencer ao domínio e ambos os renderizadores devem apenas consumi-la.

## Situação do ambiente local

Verificação realizada em 2026-07-27:

| Ferramenta | Situação |
|---|---|
| Node.js | `v24.18.0` disponível |
| npm | `12.0.1` disponível |
| Rust (`rustc`/`cargo`) | não instalado |
| .NET SDK | nenhum SDK instalado; somente runtimes presentes |
| compilador MSVC (`cl`) | não encontrado no terminal |
| CMake | não encontrado |

Esse dado não decide sozinho a arquitetura, mas altera o custo da primeira entrega. Electron pode começar com o toolchain atual. Tauri requer Rust e, no Windows, também os Microsoft C++ Build Tools; esses são pré-requisitos oficiais ([pré-requisitos do Tauri](https://v2.tauri.app/start/prerequisites/)). Qt exigiria instalar e manter um toolchain C++/Qt completo.

## Comparação

| Critério | Electron | Tauri 2 | Qt 6 nativo |
|---|---|---|---|
| Interface complexa | HTML/CSS/TypeScript em Chromium embarcado | HTML/CSS/TypeScript no WebView do sistema | Widgets ou QML nativos |
| Consistência do Canvas entre sistemas | alta, pois a aplicação leva sua versão do Chromium | menor: WebView2 no Windows e WebKit no macOS/Linux | alta dentro da versão do Qt empacotada |
| Integração local | APIs no processo principal e IPC | comandos Rust, plugins e capabilities | APIs C++ diretas |
| Trabalho pesado | `utilityProcess` ou processo filho | backend/processo Rust | threads/processos C++ |
| Renderização final | exige módulo próprio | exige módulo próprio | QPainter/QImage/PDF oferecem uma base nativa forte |
| Undo/Redo | implementado no domínio | implementado no domínio | Qt possui framework Command/Stack pronto |
| Empacotamento inicial neste ambiente | menor atrito | requer instalar Rust e Build Tools | requer instalar Qt, C++ e ferramentas de build |
| Tamanho/runtime | inclui Chromium e Node | reaproveita o WebView do sistema | inclui bibliotecas e plugins Qt necessários |
| Segurança da ponte UI/SO | depende de sandbox, isolamento e API mínima | capabilities, scopes e CSP | superfície nativa, ainda dependente do código da aplicação |
| Licenciamento da plataforma | MIT | MIT ou Apache-2.0 | LGPL/GPL ou comercial, com obrigações de distribuição |
| Risco principal | consumo e cadência de atualização do runtime; IPC privilegiado | variação de WebView e maior custo Rust imediato | custo C++/Qt, distribuição e conformidade de licença |

### Electron

Electron usa a arquitetura multiprocesso do Chromium. O processo principal controla o ciclo de vida e APIs nativas; cada janela possui um renderer, e processos utilitários podem hospedar trabalho intensivo ou componentes sujeitos a falhas ([modelo de processos](https://www.electronjs.org/docs/latest/tutorial/process-model)). Essa divisão combina bem com o produto desde que o processo principal não faça decodificação ou composição pesada.

Pontos favoráveis:

- leva Node.js e Chromium como uma base conhecida e oferece a mesma família de APIs visuais nos sistemas suportados;
- possui diálogo nativo para arquivos, pastas e múltiplas seleções ([API de diálogo](https://www.electronjs.org/docs/latest/api/dialog));
- permite isolar o renderizador final em `utilityProcess`, evitando congelar a janela;
- o ecossistema e o toolchain local já estão disponíveis;
- Electron Forge é a ferramenta especializada indicada pela documentação para empacotamento; assinatura e publicação continuam sendo etapas explícitas ([visão geral de distribuição](https://www.electronjs.org/docs/latest/tutorial/distribution-overview)).

Pontos contrários:

- incluir Chromium e Node aumenta o instalador, a memória basal e a superfície de atualização;
- o aplicativo precisa acompanhar uma cadência rápida: a política oficial cobre somente as três versões estáveis mais recentes e os majors seguem uma cadência de aproximadamente oito semanas ([calendário de releases](https://www.electronjs.org/docs/latest/tutorial/electron-timelines));
- conceder Node ao renderer transformaria falhas de UI em acesso amplo ao computador.

As configurações obrigatórias são `nodeIntegration: false`, `contextIsolation: true`, sandbox do renderer, CSP restritiva, conteúdo local empacotado, navegação externa bloqueada e validação de origem e argumentos em todo IPC. Essas medidas constam da lista oficial de segurança do Electron ([guia de segurança](https://www.electronjs.org/docs/latest/tutorial/security)).

### Tauri 2

Tauri combina um frontend em WebView com backend compilado em Rust. A comunicação ocorre por mensagens e a exposição ao frontend pode ser limitada por capabilities e scopes ([arquitetura do Tauri](https://v2.tauri.app/concept/architecture/), [capabilities](https://v2.tauri.app/security/capabilities/)).

Pontos favoráveis:

- não embarca um navegador completo e, por isso, tende a produzir uma aplicação menor;
- Rust é uma boa base para I/O, filas de exportação e processos de imagem;
- o sistema de capabilities permite conceder somente operações específicas a cada janela;
- o updater verifica obrigatoriamente assinaturas dos artefatos, e suporta servidor ou JSON estático ([updater do Tauri](https://v2.tauri.app/plugin/updater/)).

Pontos contrários:

- Tauri não fornece, por si só, o renderizador de álbum: ainda seria necessário escolher e validar bibliotecas Rust para JPEG, PNG, TIFF, perfis de cor e PDF;
- no Windows usa WebView2/Chromium, enquanto no macOS e Linux usa WebKit. Portanto, disponibilidade de recursos web, comportamento do Canvas e defeitos de GPU precisam ser testados por plataforma ([versões de WebView](https://v2.tauri.app/reference/webview-versions/));
- a equipe passa a operar duas linguagens e uma fronteira TypeScript/Rust desde o primeiro incremento;
- Rust e os Build Tools necessários não estão presentes no ambiente atual.

Para um produto exclusivamente Windows, Tauri se torna mais competitivo porque o WebView2 é recente e baseado em Chromium. Mesmo nesse cenário, o ganho de tamanho não compensa ainda o aumento de risco de entrega, a menos que tamanho do instalador ou domínio prévio de Rust sejam requisitos reais.

### Qt 6

Qt é a alternativa nativa competitiva. Seu Graphics View gerencia itens 2D, seleção, eventos, transformações, Zoom e rotação com coordenadas de dupla precisão ([Graphics View Framework](https://doc.qt.io/qt-6/graphicsview.html)). `QImageReader` oferece leitura dimensionada, recorte e limites de alocação que podem reduzir uso de memória quando o formato permite ([QImageReader](https://doc.qt.io/qt-6/qimagereader.html)). O framework de Undo implementa Command, Stack, agrupamento, compressão e estado limpo ([Qt Undo Framework](https://doc.qt.io/qt-6/qundo.html)). `QPdfWriter` produz documentos multipágina por comandos de desenho e oferece resolução e modelos de cor ([QPdfWriter](https://doc.qt.io/qt-6/qpdfwriter.html)).

Isso torna Qt forte quando a equipe já domina C++/Qt e quer unificar edição e renderização no mesmo motor. A rejeição, neste momento, é econômica e operacional:

- não há toolchain C++/Qt no ambiente;
- construir toda a UI em Widgets/QML reduz o reaproveitamento do ecossistema web e aumenta o custo inicial;
- distribuir no Windows exige incluir corretamente bibliotecas, runtimes e plugins, normalmente com `windeployqt` ([deploy no Windows](https://doc.qt.io/qt-6/windows-deployment.html));
- uma aplicação proprietária precisa cumprir conscientemente LGPLv3 ou adquirir licença comercial. A própria Qt recomenda análise jurídica, explica a preferência por vínculo dinâmico e exige permitir substituição/relink da biblioteca nas condições aplicáveis ([obrigações LGPL](https://www.qt.io/development/open-source-lgpl-obligations)).

Qt deve ser reconsiderado se surgir uma equipe experiente em Qt, se requisitos de impressão profissional excederem as bibliotecas avaliadas para o worker do Electron ou se a interface precisar ser totalmente nativa.

## Arquitetura recomendada

### 1. Renderer de interface

Responsabilidades:

- janela, painéis, modais, atalhos e acessibilidade;
- Canvas interativo alimentado somente por miniaturas/prévias;
- seleção e gestos de baixa latência;
- apresentação do estado de progresso e erros.

Stack recomendada: TypeScript e React para a interface. O motor específico do Canvas permanece atrás de uma porta de renderização e deve ser escolhido pelo protótipo do ticket 05. Essa escolha não pode contaminar o modelo do Projeto.

O renderer não recebe APIs genéricas de Node, não grava Projeto diretamente e não executa renderização final.

### 2. Núcleo de domínio

Pacote TypeScript puro, sem dependência de Electron, React, DOM ou sistema de arquivos. Deve conter:

- Projeto, Álbum, Lâmina, Página, Frame, Foto, Background, Overlay e Layout;
- invariantes e validações;
- comandos de edição e Undo/Redo;
- cálculo de geometria, enquadramento e Pilha visual;
- serialização canônica independente da interface;
- criação de um snapshot imutável e versionado para Exportação.

Todas as mutações criativas passam por comandos do domínio. Movimentos contínuos podem aparecer a cada quadro, mas devem ser consolidados como uma única transação de Undo ao finalizar o gesto. Salvar apenas marca a revisão corrente como limpa; não apaga a pilha de Undo/Redo.

### 3. Adaptador desktop

Processo principal do Electron e `preload` mínimo. Expõe contratos tipados e específicos para:

- escolher arquivos, pastas e destino;
- ler metadados e administrar referências;
- salvar atomicamente, bloquear abertura e recuperar sessão;
- controlar janelas;
- iniciar, cancelar e observar jobs;
- empacotamento e futura atualização.

Não deve existir uma operação IPC do tipo “ler qualquer caminho” ou “executar qualquer comando”. A UI solicita intenções de domínio; o adaptador valida argumentos, identidade da janela e escopo.

### 4. Serviço de mídia e Cache

O serviço associa IDs internos a caminhos originais e a representações derivadas. O Canvas recebe somente a representação adequada ao Zoom atual. Originais grandes não devem ser enviados como buffers pelo IPC nem permanecer desnecessariamente decodificados no renderer.

O Cache é reconstruível e fica fora do documento canônico. Ausência do original continua sendo erro mesmo que a prévia exista.

### 5. Worker de Exportação

Processo separado, sem DOM e sem dependência do Canvas. Recebe:

- snapshot versionado e imutável da seleção;
- IDs e caminhos resolvidos dos originais;
- dimensões, DPI, formato e destino;
- identificador de job para progresso e cancelamento.

O worker:

- abre cada original somente quando necessário;
- aplica exatamente a ordem de transformações definida pelo domínio;
- limita concorrência e memória;
- produz saídas temporárias e só as publica conforme a política do ticket 04;
- devolve resultados estruturados, sem alterar o Projeto;
- pode falhar ou reiniciar sem derrubar a janela.

Operações em lote reutilizam o mesmo carregador de documento e a mesma fila. Não existe um segundo caminho de renderização para lote.

## Representação canônica

Recomendação para impedir divergência:

- comprimentos físicos e retângulos do documento em **micrômetros inteiros**, com origem no canto superior esquerdo;
- Unidade do Projeto (`mm`, `cm` ou `in`) apenas como preferência de entrada e exibição;
- Giro de 90° em quartos de volta, Ângulo fino separado entre `-45°` e `+45°`, espelhos como booleanos e ordem de transformação explícita;
- Pan/ponto focal e Zoom adicional como valores adimensionais normalizados, separados da escala mínima de preenchimento;
- conversão para CSS pixels somente na porta do Canvas;
- conversão para pixels finais somente na porta de Exportação.

Para a saída, a relação é `pixels = comprimento_em_micrômetros × DPI / 25.400`. O ticket 04 ainda precisa escolher a regra única de arredondamento e verificar limites máximos. O valor arredondado nunca deve voltar ao domínio.

Essa estrutura permite que a prévia use uma escala pequena e o worker use os originais, ambos consumindo a mesma geometria sem compartilhar bitmap.

## Estrutura sugerida do repositório

Estrutura conceitual, ainda não criada:

```text
apps/
  desktop/          Electron main, preload e renderer
  render-worker/    entrada do processo de Exportação
packages/
  domain/           modelo, comandos, invariantes e medidas
  project-document/ schema, migrações e snapshots
  platform-contracts/
  media-contracts/
  render-contracts/
  test-fixtures/
```

Comandos esperados quando o scaffold for autorizado:

- `npm run dev`
- `npm run test`
- `npm run test:integration`
- `npm run test:e2e`
- `npm run package`

Versões devem ser fixadas no lockfile. Electron deve ser atualizado dentro da janela oficial de suporte, com upgrades de um major por vez e execução completa dos testes de Canvas, IPC, persistência e Exportação antes de publicar.

## Estratégia de testes

| Nível | Foco |
|---|---|
| Unitário | invariantes, comandos, Undo/Redo, medidas, transformações e nomes |
| Contrato | mensagens renderer/preload/main e snapshot enviado ao worker |
| Integração | salvamento atômico, arquivos ausentes, Cache, jobs e falhas de processo |
| Renderização | fixtures canônicas, dimensões, pixels de referência, transparência e leitura do original |
| Ponta a ponta | janela real no Windows, criação, diálogo substituível em teste, edição, salvar/reabrir e Exportar |
| Empacotado | instalação limpa, caminhos com Unicode/espaços, assinatura e execução sem toolchain de desenvolvimento |

O Canvas deve ser testado por estado e interação, não por seus objetos internos. O renderizador final deve ter testes próprios contra snapshots do domínio; capturar o Canvas da tela não constitui teste de Exportação.

## Empacotamento e atualização

Para a primeira versão:

- Electron Forge para gerar o instalador Windows;
- assinatura de código antes de distribuição externa;
- publicação manual inicialmente;
- updater automático somente depois de existir infraestrutura de releases assinadas e testes de migração.

Electron possui `autoUpdater` integrado para Windows e macOS, mas não para Linux; formatos e comportamento dependem do empacotamento ([API `autoUpdater`](https://www.electronjs.org/docs/latest/api/auto-updater/)). Uma atualização nunca pode salvar conteúdo criativo nem substituir uma sessão com mudanças pendentes. A aplicação deve oferecer instalação em momento seguro ou no próximo início.

## Riscos e mitigação

| Risco | Mitigação obrigatória |
|---|---|
| Canvas lento com muitas prévias | miniaturas por nível, descarte de texturas fora da Lâmina ativa e orçamento de memória |
| Janela congelada durante Exportação | processo separado, fila limitada e mensagens de progresso |
| Diferença entre prévia e saída | um único domínio canônico e testes de renderização com fixtures |
| IPC privilegiado | preload mínimo, sandbox, CSP, validação de origem/argumentos e nenhuma API genérica |
| Falha de módulo nativo | isolá-lo no worker e validar instalação empacotada em máquina limpa |
| Cadência rápida do Electron | lockfile, rotina de atualização e suporte apenas a versões mantidas |
| Custo futuro de migração | contratos de plataforma, mídia e renderização sem Electron no domínio |
| Escopo multiplataforma tardio | confirmar plataforma agora e testar GPU/arquivos por sistema antes de prometer suporte |

## Recomendação final

Adotar **Electron + TypeScript**, com React na interface, núcleo de domínio puro e worker de Exportação separado.

A plataforma inicial Windows 10/11 x64 está confirmada. A escolha deve ser convertida em ADR depois da aceitação humana da stack recomendada. O ADR deve registrar Tauri 2 como alternativa secundária caso tamanho de distribuição ou adoção de Rust se tornem prioridades, e Qt 6 como alternativa especializada caso a equipe e os requisitos de impressão justifiquem o custo nativo.

Antes do primeiro scaffold, os próximos tickets devem preservar estes bloqueios:

1. ticket 02 define documento, identidade, salvamento e recuperação sem acoplar o domínio ao Electron;
2. ticket 03 decide codecs, metadados e Cache;
3. ticket 04 valida o worker final com originais, formatos, cor, memória e publicação;
4. ticket 05 escolhe o motor de Canvas por protótipo e mede interação, sem usá-lo como exportador final.
