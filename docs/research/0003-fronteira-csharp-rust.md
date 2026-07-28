---
status: historical-alternative
document: architecture-design
ticket: 01-plataforma-e-arquitetura
date: 2026-07-27
platform: Windows 10/11 x64
---

# Fronteira entre a interface C# e o motor Rust

> Documento histórico e não normativo. A alternativa C#/Rust e suas premissas de processo não definem a arquitetura vigente; consulte o [ADR 0005](../adr/0005-adotar-tauri-react-rust.md).

## Situação atual

Esta arquitetura híbrida não é a direção principal nem a contingência imediata da primeira versão. A decisão vigente é Tauri 2 com React/TypeScript e Rust, registrada no [ADR 0005](../adr/0005-adotar-tauri-react-rust.md). Se essa direção falhar nos critérios declarados do spike, a contingência é uma solução WPF/.NET integralmente em C#.

O restante deste documento é preservado como desenho histórico de uma terceira alternativa: WPF/C# na interface e Rust em um motor separado. Ele somente deverá ser reconsiderado se tanto a direção Tauri quanto a contingência WPF/C# se mostrarem inadequadas e existir evidência de que essa divisão resolve uma limitação específica.

## Desenho da alternativa histórica

Nesta alternativa, usar C# e WPF para a interface e Rust para o motor é tecnicamente possível, desde que as linguagens sejam separadas por processos e por uma interface pequena, tipada e versionada.

A recomendação combina dois níveis:

1. o seam externo entre C# e Rust possui um protocolo mínimo de pedidos e eventos;
2. o adapter C# oferece aos ViewModels uma interface orientada às tarefas reais do editor.

Não carregar o Rust como DLL dentro do processo WPF na primeira versão. O processo separado isola falhas, permite reinício, serve a operações headless e evita transformar detalhes de ABI em parte da interface do produto.

## Responsabilidades

### C# e WPF

- janelas, menus, painéis, modais, atalhos e acessibilidade;
- diálogos nativos de arquivos e pastas;
- Canvas de preview;
- seleção e estado transitório do ponteiro;
- feedback local de Pan, Zoom, movimento e resize durante um gesto;
- tradução e apresentação dos erros;
- adapter do protocolo Rust.

C# não possui invariantes do Projeto e não grava o documento diretamente.

### Motor Rust

- representação canônica de Projeto, Álbum, Lâmina, Página, Frame, Foto, Background, Overlay e Layout;
- comandos, validações, revisões e Undo/Redo;
- dimensões físicas e transformações;
- aplicação e geração de Layouts;
- herança `default`/`custom`;
- persistência, migrações, Identidade do Projeto e Bloqueio de abertura;
- mídia vinculada, metadados, Cache e representações reduzidas;
- snapshot imutável de Exportação;
- planejamento de conflitos e operações em lote;
- coordenação do Processador de Imagens compartilhado entre mídia, Cache e Exportação.

### Processador de Imagens Rust

Trabalho pesado ou sujeito a arquivos defeituosos não roda na fila de comandos do motor.

Cada Projeto possui um único processo `MyAlbuns.Imaging.exe`. Ele executa um trabalho pesado por vez e possui dois modos internos:

- em modo de mídia, lê originais para extrair metadados e gerar representações reduzidas descartáveis do Cache; eventual tiling dependeria de medição;
- em modo de Exportação, recebe um snapshot versionado, resolve novamente os originais e produz JPEG, PNG ou PDF segundo as decisões do ticket 04.

Ao iniciar uma Exportação, o motor pausa ou cancela em um ponto seguro o trabalho de Cache, libera os recursos correspondentes e reserva o Processador de Imagens para a saída final. Depois da Exportação, a fila de Cache é retomada.

Os dois modos compartilham bibliotecas Rust de decodificação e geometria, mas mantêm filas, resultados e garantias lógicas diferentes. Cache continua descartável; Exportação continua final e nunca usa um artefato de Cache como fonte. Um crash, codec defeituoso ou falta de memória no Processador de Imagens não deve encerrar a janela nem corromper o estado canônico.

## Topologia

```text
MyAlbuns.exe
Processo principal leve por usuário
    |
    +-- <Nome do Projeto A>
    |       |
    |       +-- MyAlbuns.Project.exe    Janela do Projeto / WPF
    |       |
    |       +-- album-engine.exe  Projeto A / Rust
    |               |
    |               +-- MyAlbuns.Imaging.exe
    |
    +-- <Nome do Projeto B>
            |
            +-- MyAlbuns.Project.exe    Janela do Projeto / WPF
            |
            +-- album-engine.exe        Projeto B / Rust
                    |
                    +-- MyAlbuns.Imaging.exe
```

### Múltiplos Projetos

A topologia recomendada para isolamento máximo é uma janela/processo WPF e um processo Rust por Projeto aberto.

- Uma janela que monopolize seu Dispatcher não torna as demais janelas irresponsivas.
- Uma exceção não tratada, vazamento ou pressão de memória no Projeto A não derruba o Projeto B.
- Cada motor possui estado, Histórico da sessão, Cache de memória e Bloqueio de abertura próprios.
- Fechar uma janela encerra somente seu motor e seu Processador de Imagens por um Windows Job Object.
- Se o motor falhar, sua janela permanece aberta e oferece reinício e Recuperação de sessão.

O processo MyAlbuns é leve e não contém estado criativo. Ele recebe pedidos de abertura, mantém o registro de janelas e ativa a janela existente quando o mesmo Projeto já está aberto.

O Bloqueio de abertura continua sendo responsabilidade do motor Rust e protege contra outra instância do aplicativo. Projetos diferentes usam locks independentes. Uma Cópia externa recebe nova Identidade e pode permanecer aberta ao mesmo tempo que o original.

### Controle global de recursos

Processos separados não eliminam contenção de CPU, GPU, memória ou disco. O processo MyAlbuns mantém um orçamento global:

- limite global de Projetos que podem executar trabalho pesado simultaneamente;
- prioridade para previews da Lâmina ativa, seguida pelos itens visíveis do Painel de imagens;
- prefetch de Lâminas vizinhas, reconstrução integral e limpeza de Cache em prioridades inferiores;
- cancelamento e agrupamento de pedidos de preview que se tornaram obsoletos;
- orçamento de memória por Projeto;
- descarte de previews e texturas de janelas inativas;
- fila justa para que um lote não impeça trabalho interativo em outro Projeto.

Um Projeto pode continuar sendo editado enquanto outro exporta. Se vários Projetos solicitarem Exportação pesada, os jobs aguardam vagas em vez de saturar o computador.

Dentro do mesmo Projeto, Exportação e geração de Cache nunca executam simultaneamente. Previews já existentes continuam disponíveis; uma preview ainda não gerada aguarda o fim da Exportação.

A interface do motor usa IDs de sessão e não depende da topologia física, permitindo revisar futuramente a quantidade de processos sem alterar os ViewModels.

## Seam externo

O protocolo transporta duas famílias:

```csharp
public interface IAlbumEngineTransport
{
    ValueTask<EngineReply> RequestAsync(
        EngineRequest request,
        CancellationToken cancellationToken = default);

    IAsyncEnumerable<EngineEvent> ObserveAsync(
        ObserveRequest request,
        CancellationToken cancellationToken = default);
}
```

`RequestAsync` não aceita comandos arbitrários. `EngineRequest` é uma união tipada e versionada:

- handshake;
- abrir ou criar Projeto;
- consultar projeção;
- aplicar intenção de edição;
- Undo/Redo;
- Salvar ou `Salvar como`;
- iniciar, decidir ou cancelar job;
- fechar sessão.

`ObserveAsync` entrega somente fatos assíncronos:

- projeção invalidada;
- preview pronto;
- progresso;
- decisão necessária;
- conclusão ou falha de job;
- indisponibilidade do motor.

O contrato deve ser gerado para C# e Rust a partir de uma única fonte. Protobuf length-prefixed sobre named pipe é a candidata inicial, sujeita ao spike.

## Interface para os ViewModels

ViewModels não conhecem envelopes do protocolo. O adapter expõe uma interface orientada ao uso:

```csharp
public interface IProjectEditor : IAsyncDisposable
{
    EditorProjection Current { get; }

    ValueTask<EditorOutcome> EditAsync(
        ProjectEdit edit,
        CancellationToken cancellationToken = default);

    ILocalGesture BeginGesture(GestureStart start);

    ValueTask<EditorOutcome> UndoAsync(
        CancellationToken cancellationToken = default);

    ValueTask<EditorOutcome> RedoAsync(
        CancellationToken cancellationToken = default);

    ValueTask<SaveOutcome> SaveAsync(
        SaveTarget? target = null,
        CancellationToken cancellationToken = default);

    ValueTask<ExportStartOutcome> StartExportAsync(
        ExportRequest request,
        CancellationToken cancellationToken = default);
}
```

`ProjectEdit` representa intenções completas, como `ApplyLayout`, `CommitPhotoPlacement`, `SetBackgroundApplication` ou `ChangeDimensionsSafely`. Não existem chamadas rasas como `SetFrameX` e `SetFrameY`.

## Projeções

C# nunca recebe um Projeto mutável. Rust entrega projeções imutáveis com:

- revisão canônica;
- lados ativos;
- geometria resolvida;
- Pilha visual;
- transformações resolvidas;
- estado de herança;
- permissões de interação;
- avisos;
- referências opacas para previews.

Medidas atravessam o seam em micrômetros inteiros. C# converte para DIPs somente na porta do Canvas.

Originais, bitmaps decodificados e páginas renderizadas não atravessam IPC. Previews são arquivos imutáveis do Cache ou outro artefato somente leitura identificado por versão e lease. A Exportação grava diretamente no destino.

## Gestos

Pan, Zoom, movimento e resize precisam manter 60 Hz sem IPC:

1. Rust fornece a projeção e um plano de interação;
2. C# mostra uma transformação transitória local durante o gesto;
3. ao terminar, C# envia uma única intenção consolidada com a revisão esperada;
4. Rust recalcula e valida;
5. uma aceitação incrementa a revisão e cria uma entrada de Undo;
6. uma rejeição devolve a projeção atual e C# descarta o estado transitório.

C# pode antecipar visualmente o resultado, mas nunca autoriza uma geometria inválida.

## Revisões e falhas

- toda mutação informa `expected_revision`;
- cada comando possui ID idempotente;
- mutações de uma sessão são serializadas;
- revisão antiga produz conflito, nunca merge silencioso;
- Salvar não elimina Undo/Redo;
- job de Exportação captura uma revisão imutável;
- eventos são ordenados e retomáveis por cursor;
- erros possuem códigos estáveis e parâmetros localizáveis;
- stack traces e mensagens internas do Rust não são mostrados diretamente.

O handshake negocia versão e capacidades. Formato do Projeto e protocolo IPC possuem versões independentes.

## Jobs

Exportação e lote usam uma máquina de estados:

```text
Queued
Planning
AwaitingDecision
Running
Cancelling
Succeeded | Failed | Cancelled
```

Conflitos são planejados pelo Rust. C# apresenta o modal e devolve uma decisão tipada. Progresso pode ser agrupado; decisões e resultados terminais nunca podem ser descartados.

## Pipeline de mídia e Cache

Ao importar ou detectar mudança em um Arquivo vinculado:

1. o motor registra ou atualiza a referência canônica;
2. cria pedidos de metadados e preview sem bloquear o comando;
3. o escalonador agrupa pedidos duplicados e escolhe a prioridade;
4. o Processador de Imagens, em modo de mídia, abre o original e gera somente a representação necessária;
5. o artefato é escrito temporariamente e publicado de forma atômica;
6. o motor emite um evento com uma nova referência imutável;
7. WPF substitui o placeholder ou a versão anterior.

Um pedido pode ser cancelado quando o usuário muda de Lâmina ou rola o Painel. Artefatos já publicados permanecem válidos até o fim de suas leases e são limpos posteriormente.

Cache nunca participa do documento canônico, nunca constitui Salvamento e pode ser apagado ou reconstruído integralmente. Um Arquivo ausente pode continuar mostrando a última preview com aviso, mas nenhuma operação final considera essa preview equivalente ao original.

Formato, localização, fingerprint, níveis, orçamento de disco e estratégia exata de invalidação continuam sendo decisões do ticket 03.

## Adapters e testes

No seam externo:

- `NamedPipeAlbumEngineAdapter`: produção;
- `ScriptedAlbumEngineAdapter`: telas e ViewModels;
- `RustEngineHarnessAdapter`: testes de contrato com processo real e diretório temporário.

O adapter simulado não reimplementa o domínio. Invariantes são testadas no Rust e por testes de integração contra o motor real.

No Rust, filesystem, relógio, IDs, codecs, Cache, locks e escritores finais são dependências locais substituíveis. Seus adapters de teste permanecem internos ao motor e não aumentam a interface C#.

Testes necessários:

- domínio e comandos em Rust;
- compatibilidade do protocolo gerado;
- ViewModels com adapter roteirizado;
- motor real com arquivos temporários;
- fixtures canônicas de preview e Exportação;
- queda e reinício do motor;
- queda do Processador de Imagens em qualquer modo sem queda da interface;
- cancelamento, deduplicação e repriorização de geração de previews;
- reconstrução completa depois de apagar o Cache;
- instalador limpo com .NET e executáveis Rust.

## Alternativas comparadas

### Interface de transporte mínima

Dois pontos de entrada produzem máxima profundidade e um protocolo simples. Sozinha, porém, ela seria genérica demais para ViewModels; por isso fica restrita ao adapter.

### Interface extensível por sessões, projeções e jobs

Representa muito bem múltiplos Projetos, migrações e operações em lote. O risco é antecipar plugins e capacidades não necessárias. A proposta adota revisões, jobs e projeções, mas não uma arquitetura pública de plugins.

### Interface orientada ao editor

É a melhor interface para o chamador comum. Esconde IPC, revisão e projeções atrás de tarefas como editar, salvar e exportar. A proposta a utiliza entre ViewModels e o adapter C#.

## Consequências

Vantagens:

- C# oferece uma UI Windows madura;
- Rust concentra as regras e o processamento pesado;
- domínio não é duplicado;
- Exportação e lote podem ser reutilizados sem WPF;
- falhas nativas são isoladas;
- o motor pode ganhar futuramente uma CLI;
- testes cruzam o mesmo seam usado pelo produto.

Custos:

- dois toolchains e duas linguagens;
- protocolo e geração de contratos;
- debugging entre processos;
- empacotamento e logs correlacionados;
- preview WPF e renderização final Rust continuam implementações visuais diferentes;
- commits de gesto podem receber correção ao serem validados.

## Recomendação

Esta arquitetura híbrida não deve ser implementada em paralelo. Ela é mais trabalhosa que WPF/C# puro e, diante da direção Tauri vigente, acrescentaria uma terceira combinação tecnológica sem necessidade comprovada.

Se ela vier a ser reconsiderada após as duas opções prioritárias, um novo ADR e um spike direcionado deverão comprovar:

- Pan/Zoom e resize locais sem IPC por frame;
- commit consolidado e Undo/Redo no Rust;
- projeção de uma Lâmina real;
- preview por referência de Cache;
- Processador de Imagens compartilhado gerando preview sem bloquear comandos;
- Exportação isolada usando originais;
- pausa da fila de Cache durante a Exportação e retomada posterior;
- queda e reinício do Processador de Imagens sem perda do estado canônico;
- contrato C#↔Rust gerado e versionado;
- build e instalador `win-x64`.
