---
status: accepted
document: design
updated: 2026-09-01
---

# Propriedade de estado e módulos do núcleo

## Objetivo

Definir onde vivem o estado criativo, os cálculos de composição e os estados operacionais do MyAlbuns. O desenho evita duas fontes canônicas para o mesmo Projeto, impede que Cache, watcher ou interface marquem o documento como alterado e fornece os mesmos cálculos para editor, Exportação normal e lote.

Este documento detalha a direção aceita no [ADR 0005](../adr/0005-adotar-tauri-react-rust.md). Os nomes abaixo continuam sendo nomes de trabalho. O contrato de propriedade e as separações de responsabilidade importam mais que a quantidade final de crates ou tipos públicos.

## Princípios

- Cada Projeto aberto possui exatamente uma `ProjectSession` como proprietária mutável de seu estado criativo.
- O arquivo persistido, a Sessão do Projeto, o estado transitório da interface, o estado operacional das mídias e o Cache são estados diferentes.
- Cálculos de composição são puros e determinísticos: recebem valores imutáveis e retornam resultados sem I/O ou mutação escondida.
- O `ProjectCore` continua sendo o seam externo pequeno usado pelo host interativo e pelo lote. Suas subdivisões são inicialmente internas e não precisam virar interfaces públicas.
- Processos são uma decisão de implantação. Os módulos permanecem neutros, embora a implantação aceita use um host independente por Projeto.
- Uma operação longa possui seus próprios cancelamento e progresso. Exclusividade global é pequena e explícita, não um coordenador universal de toda ação do aplicativo.
- Persistências com garantias diferentes usam stores concretos; somente primitivas mecânicas comprovadamente iguais são compartilhadas.

## Mapa de responsabilidades

| Módulo de trabalho | Possui | Não possui |
|---|---|---|
| `ProjectDomain` | tipos persistentes, Identidades, Lâminas, Frames, referências de mídia, invariantes e comandos de domínio | I/O, JSON, migração de arquivos, Cache, watcher, IPC ou estado da interface |
| `ProjectSession` | estado criativo atual, revisão, revisão salva, mudanças pendentes, Undo/Redo, aplicação de comandos e snapshots | seleção, hover, zoom do Canvas, estado de rede, artefatos de Cache ou publicação de Exportação |
| `ProjectStore` | detecção de versão, migração, desserialização, validação estrutural, revisão esperada e Salvamento atômico do arquivo de Projeto | Sessão viva, Undo/Redo, Cache ou interpretação visual |
| `ProjectIdentityRegistry` | última Localização autorizada por Identidade, schema e substituição atômica do registro local que sobrevive à Sessão | documento de Projeto, Identidade física eterna, Sessão viva, Projetos recentes, Cache ou Recuperação |
| `CompositionCore` | recortes, preenchimento, transformações, ordem de desenho, travessia central, divisão por Página e plano determinístico de composição | I/O, codecs, PixiJS, documento bruto, estado mutável ou publicação |
| `LayoutRules` | compatibilidade, Mapeamento, escopo inferido, identidade de Layout personalizado, prioridade de candidatos, produção de `LayoutPatch` e a garantia de ao menos um candidato compatível | confirmação do patch, catálogo persistido, Undo/Redo, estado da interface ou o algoritmo do Gerador |
| `MediaResolver` | inspeção autoritativa de arquivos, validação de formato, busca de Religação e produção de observações ou propostas imutáveis | mutação direta do Projeto, estado observado da sessão, Undo/Redo, Cache persistido ou canonicalização textual como identidade |
| `MediaRuntime` | registro da disponibilidade observada por sessão, última observação, fingerprint conhecido e estado do monitor | I/O de inspeção, caminho canônico, decisões do usuário, estado salvo ou mudanças pendentes |
| `MediaMonitor` | eventos de mudança agrupados e pedidos de nova inspeção | verdade autoritativa sobre existência, ausência ou conteúdo |
| `CacheEngine` | jobs, índice descartável, artefatos, invalidação, pausa, reconstrução e manutenção do Cache | estado criativo, Originais como substitutos ou fonte de Exportação |
| `ExportPipeline` | planejamento, execução e Publicação de uma tentativa a partir de snapshot imutável e originais | mutação, Salvamento ou Religação automática do Projeto |
| `OperationGate` | concessões de exclusividade global estritamente necessárias | fila geral de comandos, seleção, progresso, cancelamento ou estado criativo |
| `OperationLease` | aquisição e liberação conjunta da concessão, da pausa do Cache e da reserva do Processador, garantidas em qualquer estado terminal | a política de exclusividade, os jobs de Cache, a renderização, o progresso ou o cancelamento da tentativa |
| `CommandCatalog` | IDs, descrições, contextos e associações padrão estáveis | foco corrente, seleção, reconhecimento de gestos, dispatch ou Histórico da sessão |

## `ProjectCore` e a única sessão mutável

`ProjectCore` é a interface que esconde as subdivisões `ProjectDomain`, `ProjectSession`, `ProjectStore` e `ProjectIdentityRegistry`. Ela possui dois modos de acesso e operações públicas distintas, sem oferecer cada subdivisão aos chamadores:

| Operação | Usada por | Devolve | Instancia `ProjectSession`? |
|---|---|---|---|
| Criar sessão editável | fluxo de criação | sessão viva publicada, autorizada e bloqueada | sim |
| Abrir sessão editável | Janela do Projeto | sessão viva autorizada: aplicar intenção, salvar, `Salvar como`, obter snapshots | sim |
| Carregar revisão persistida | `BatchRunner` | valor imutável suficiente para o `RenderSnapshot` | não |

Criação e abertura editável devolvem um `EditableProject`; `Salvar` e `Salvar como` são transições desse mesmo proprietário, não entradas livres que aceitam estado serializado pelo frontend. A carga somente de leitura compartilha detecção, migração e validação, mas não oferece comando, Salvamento ou Undo/Redo e por isso não pode gravar no arquivo do usuário. A forma detalhada dessas operações pertence ao [Contrato público de persistência do ProjectCore](0015-contrato-publico-de-persistencia-do-project-core.md).

Antes de devolver um `EditableProject`, `ProjectCore` coordena o `ProjectStore`, o registro local, o `ProjectIdentityLease` e a trava física. Somente o terminal em que o arquivo e o registro da Identidade aplicável foram publicados e verificados produz uma autoridade de Identidade consumível por Cache, Recuperação ou WebView2. Um `projectId` apenas desserializado nunca basta para montar esses namespaces.

Essa separação é o que sustenta a regra do [ADR 0005](../adr/0005-adotar-tauri-react-rust.md) de que `MyAlbuns.exe` hospeda o lote sem possuir estado criativo mutável. Um chamador precisa escolher o modo conscientemente: pedir o editável para um trabalho de leitura cria um dono mutável desnecessário.

`ProjectSession` é a única proprietária mutável do estado criativo de um Projeto aberto. Uma alteração segue este fluxo:

1. a interface mantém apenas o feedback transitório do gesto;
2. ao concluir, envia uma intenção consolidada;
3. `ProjectSession` valida e aplica um comando de domínio;
4. a revisão, o estado de Undo/Redo e a indicação de mudanças pendentes são atualizados juntos;
5. consumidores recebem um novo snapshot ou resultado, nunca uma referência mutável ao estado interno.

Seleção, hover, painel aberto, foco, posição de rolagem e navegação do Canvas permanecem no frontend. Estado observado de rede e watcher permanece em `MediaRuntime`. Nenhum desses valores participa de Salvamento, Recuperação ou `RenderSnapshot`.

### Que regras pertencem ao núcleo

Vale o critério do [ADR 0005](../adr/0005-adotar-tauri-react-rust.md): se o resultado sobrevive ao Salvamento ou participa do Undo/Redo, a regra é do núcleo.

Isso alcança regras determinísticas que a SPEC descreve dentro de gestos, mas cujo efeito é documental. `ProjectDomain` as possui e a interface apenas as invoca:

| Regra | Decide |
|---|---|
| Placeholder de menor borda esquerda, desempate pela menor borda superior | qual Frame o duplo clique preenche |
| Frame mais acima na Pilha visual cujo retângulo externo contém o ponto | qual Frame recebe uma Foto solta em área de sobreposição |
| Menor entre o deslocamento desejado e o que mantém o conjunto na superfície ativa; sem deslocamento viável, mesma posição em ordem determinística | onde uma colagem de Frames é posicionada |

A interface envia a intenção com seus dados de entrada — a Foto, a Lâmina alvo, o ponto da soltura — e recebe a decisão pronta. Ela não ordena placeholders, não resolve sobreposição e não calcula deslocamento de colagem.

O ganho não é teórico: duplo clique e arraste precisam concordar sobre qual Frame é atingido, e as duas entradas passam pela mesma regra. Além disso, cada uma delas fica testável sem interface, com valores de entrada e resultado esperado.

Migrações pertencem a `ProjectStore`, pois descrevem versões do formato persistido. Depois de carregar e migrar, `ProjectDomain` valida as invariantes do valor resultante sem conhecer JSON, extensão ou versão antiga.

## Transformações e composição

Dois conceitos não podem compartilhar o mesmo estado:

- `MediaTransform` é a transformação persistente de uma Foto dentro de um Frame: Pan, Zoom adicional, espelhamento, giro em passos de 90 graus e Ângulo da Foto. Ela participa do Projeto, Undo/Redo, Recuperação, prévia e Exportação.
- `ViewportTransform` é a transformação transitória usada para navegar visualmente no Canvas durante o Modo de edição: zoom e deslocamento da visualização. Ela pertence à interface e nunca participa do Projeto, do Histórico, do Salvamento ou da Exportação.

O Zoom de preenchimento é derivado pelo `CompositionCore`; o Zoom do usuário pertence a `MediaTransform`. A composição nunca permite que a imagem deixe regiões vazadas no Frame.

`CompositionCore` recebe uma entrada imutável e retorna um `CompositionPlan`. Esse plano descreve geometria efetiva, recortes, transformações, ordem de desenho e unidades `Por lâmina` ou `Por página`. Ele não abre mídia nem produz pixels.

O editor e a Exportação usam a mesma regra de composição por meio dessa interface. PixiJS adapta o plano à cena interativa; o Processador de Imagens adapta o mesmo contrato à rasterização final com os originais.

As regras de Layout pertencem a `LayoutRules`, inicialmente dentro desse núcleo. Consultar ou pré-visualizar um Layout produz um `LayoutPatch` imutável; somente `ProjectSession` pode confirmá-lo como um único comando de Undo/Redo. Um `LayoutEngine` separado só será justificado quando o futuro Gerador exigir busca, ranqueamento, diversidade, sementes ou orçamento de tempo próprios.

`LayoutRules` possui a garantia de que existe sempre ao menos um candidato compatível para a quantidade atual de Frames, o formato da superfície e o escopo aplicável. A garantia é absoluta: quando nenhum candidato do catálogo ou do Gerador servir, `LayoutRules` produz um arranjo de reserva determinístico, derivado apenas da quantidade de Frames e da superfície ativa.

Essa garantia não é conveniência de interface. As automações da SPEC — excluir ou acrescentar Frames fora do Modo de edição, converter uma extremidade, soltar uma Foto em área vazia — são especificadas como "aplicar o primeiro Layout compatível" e não possuem comportamento alternativo definido. Um Gerador que pudesse devolver vazio deixaria cada uma dessas ações sem saída. Ver [ADR 0008](../adr/0008-garantir-layout-compativel-por-arranjo-de-reserva.md).

## Mídias, watcher e Cache

A referência persistente de uma mídia, indicada como `MediaRef`, pertence a `ProjectDomain` e contém sua Identidade, caminho, categoria e decisões do usuário. Metadados derivados não substituem esses dados.

`MediaResolver` consome o módulo central de caminhos para inspecionar importações, procurar candidatos de Religação, validar formatos e produzir propostas. Ele nunca altera a Sessão diretamente. Na Religação:

1. `MediaResolver` encontra zero, um ou vários candidatos;
2. a interface apresenta ou confirma a escolha aplicável;
3. `ProjectSession` aplica `RelinkMedia` como comando;
4. `MediaResolver` reinspeciona a nova referência e devolve uma observação imutável;
5. `MediaRuntime` registra a observação confirmada;
6. `CacheEngine` invalida e reconstrói somente o necessário.

A tradução entre o resultado de I/O e o estado de domínio pertence a `MediaResolver`, que a aplica ao produzir cada observação. O módulo de caminhos entrega apenas a evidência tipada descrita em [Resolução e política de caminhos](0011-resolucao-e-politica-de-caminhos.md):

| Evidência do módulo de caminhos | Estado de domínio |
|---|---|
| `NotFound`, com a raiz confirmadamente acessível | `Arquivo ausente` |
| `Unavailable` | `Arquivo indisponível` |
| `AccessDenied` | `Arquivo indisponível` |

Nenhum outro resultado confirma ausência. A assimetria é deliberada e segue o [ADR 0001](../adr/0001-vincular-arquivos-externos.md): tratar indisponibilidade como ausência faria uma queda momentânea de rede pedir a Religação de todos os vínculos daquela origem. `Arquivo indisponível` preserva o vínculo e a última representação conhecida, com indicação própria, e oferece nova tentativa em vez de Religação — sem criar Undo/Redo ou alterar o Projeto.

`MediaMonitor` produz indícios de mudança e consolida eventos rápidos. Ele pede ao `MediaResolver` uma nova inspeção autoritativa; somente a observação imutável resultante pode confirmar `Arquivo ausente`, `Arquivo indisponível` ou conteúdo alterado. `MediaRuntime` registra essas transições e o `CacheEngine` reage a elas, mas nenhuma cria Undo/Redo ou mudanças pendentes enquanto o caminho persistido não mudar.

`CacheEngine` é o proprietário lógico do Cache. Ele coordena pedidos, índice, gerações, publicação de artefatos, invalidação, pausa e manutenção. O `MyAlbuns.Imaging.exe` pode ser o único adaptador escritor dos arquivos, mas não se torna por isso proprietário do estado criativo ou das referências persistentes.

## Exportação e operações longas

`ExportPipeline` possui uma interface em duas etapas porque as dependências precisam ser conhecidas antes de capturar caminhos:

1. `plan(snapshot, options)` recebe um `RenderSnapshot` imutável e devolve um `ExportPlan` que possui esse snapshot exato, além das unidades, dependências e raízes necessárias; o snapshot não volta a ser parâmetro em nenhuma etapa posterior;
2. o proprietário resolve essas raízes em seu `OperationPathContext` e o congela em `RootBindingPlan`;
3. `execute(export_plan, root_bindings, cancellation, progress)` executa a tentativa sem redescobrir raízes já capturadas.

Sua implementação possui três fases internas:

1. `ExportPlanner` calcula unidades, dependências, raízes, nomes, conflitos e plano de saída, movendo o snapshot validado para dentro do `ExportPlan`;
2. `ExportExecutor` captura os Originais e consome somente as unidades derivadas do `RenderSnapshot` já composto, renderiza e verifica a preparação;
3. `Publisher` promove arquivos aos nomes finais e executa a limpeza de órfãos permitida.

`ExportPipeline` possui o ciclo de vida da preparação; `ExportExecutor` grava e verifica as saídas nela, e o pipeline garante sua limpeza nos estados terminais tratáveis. `Publisher` segue a transação limitada do [ADR 0006](../adr/0006-publicar-exportacao-com-transacao-limitada.md). Staging no Destino permite substituição atômica por arquivo quando suportada, mas não oferece rollback do conjunto, backup integral ou manifesto persistente.

`BatchRunner` permanece fora de `ExportPipeline`. Ele descobre e pré-valida Projetos, mantém checkpoint e processa os itens serialmente. Primeiro usa `plan` para os itens conhecidos e captura o único `RootBindingPlan` da tentativa do lote; depois chama `execute_group` para cada item com esse mesmo plano — `execute` é apenas a especialização de saída única. Uma raiz inesperada interrompe o planejamento atual em vez de ser resolvida independentemente por um worker.

### Dois mecanismos de exclusividade, deliberadamente separados

O aplicativo possui duas exclusividades que nunca devem ser unificadas:

| Mecanismo | Escopo da posse | Término da posse |
|---|---|---|
| `OperationGate` | uma tentativa de operação do aplicativo | terminal da tentativa ou abandono quando o processo proprietário morre |
| Bloqueio de abertura | uma Sessão editável por Identidade de Projeto | fechamento normal da Sessão; em uma queda, o Windows só o libera depois que o processo proprietário realmente morre |

A separação não é acidental. O terminal de uma tentativa devolve sua concessão sem encerrar a Sessão e sem liberar o arquivo; o Bloqueio de abertura continua retido enquanto essa Sessão permanecer editável. Nenhum dos mecanismos é estado persistente: diante da queda do proprietário, o sistema operacional libera os recursos somente depois que o processo deixa de existir. Uma trava de arquivo também é inadequada para arbitrar operações internas. Unificá-los quebraria os dois ciclos de vida.

O Bloqueio de abertura também combina identidade física e Identidade persistida, que o `OperationGate` desconhece por completo. Ele pertence ao fluxo de abertura de Projeto, não a este módulo.

`OperationGate` concede uma única reserva exclusiva global, sem codificar rótulos
de operações que ainda não possuem consumidores. Hoje a Exportação normal usa
essa concessão. O futuro lote e a manutenção total do Cache devem adquirir a
mesma concessão, enquanto seus proprietários aplicam as políticas adicionais
definidas pela SPEC. O gate não armazena o tipo da operação, progresso,
cancelamento ou estado criativo e não vira um coordenador universal.

Cada tentativa possui seu próprio token de cancelamento e destino de progresso.

Toda Exportação precisa de três reservas antes de produzir saída: a concessão aplicável do `OperationGate`, a pausa do trabalho de Cache que compartilharia o Processador e a reserva do próprio Processador. As três são adquiridas e liberadas por um `OperationLease` único, e não pelo chamador.

`OperationLease` não possui a política de exclusividade nem os jobs de Cache — `OperationGate` e `CacheEngine` continuam donos deles. O que ele possui é a **ordem de aquisição e a garantia de liberação**: as três reservas são devolvidas em sucesso, falha, cancelamento e queda do proprietário, sempre juntas. Como cada Projeto possui host e Processador de Imagens isolados, a pausa cobre o namespace do Projeto exportado.

A Exportação normal adquire uma instância de `OperationLease` por tentativa. O `BatchRunner` adquire uma única instância de `OperationLease` para toda a tentativa do lote e executa todos os itens sob ela, sem liberar ou readquirir entre itens. Assim, os dois caminhos usam o mesmo mecanismo e não podem divergir em quais recursos devolvem depois de uma falha. Uma reserva vazada é o pior modo de falha do sistema: a concessão é global, então uma Exportação que não devolve a sua impede qualquer outra em todos os Projetos abertos até o programa reiniciar. Concentrar a liberação em um lugar é o que torna esse caso verificável por um teste só.

## Persistência concreta e primitivas compartilhadas

As garantias abaixo não devem ser escondidas por um `AppStorage` ou `Store<T>` genérico:

- `ProjectStore` possui schema, migrações, revisão esperada e Salvamento explícito;
- `ProjectIdentityRegistry` possui evidência local durável, atualização atômica e falha fechada própria;
- `SettingsStore` possui preferências globais e atualização imediata;
- `LayoutCatalogStore` possui conteúdo global criado pelo usuário e revisão própria;
- `StateStore` possui estado local reconstruível ou substituível;
- `RecoveryStore` possui checkpoints e ciclos de remoção específicos;
- o índice de Cache pertence ao `CacheEngine` e é inteiramente descartável.

Esses stores podem compartilhar uma implementação interna pequena para criar temporário irmão, descarregar buffers, substituir um único arquivo, versionar envelopes e traduzir erros. Compartilhar essas primitivas não iguala políticas de corrupção, recuperação, concorrência ou ciclo de vida.

## Topologia e IPC

Os módulos acima permanecem neutros quanto à implantação para não espalhar detalhes de processo pelo núcleo. O [ADR 0005](../adr/0005-adotar-tauri-react-rust.md) mapeia essa arquitetura para um host independente por Projeto no MVP.

Quando uma operação lógica atravessar processos, a IPC transporta somente valores imutáveis. Para renderização, o `ExportPlan` já possui o `RenderSnapshot` validado inteiro, e sua conversão para o envelope recebe somente o mesmo plano de bindings de raiz; não existe parâmetro capaz de trocar o snapshot depois do planejamento. O documento bruto, a Sessão e o Cache não atravessam essa fronteira. O Processador consome o `CompositionPlan` já contido no snapshot, não interpreta novamente o Projeto nem invoca `CompositionCore`; o [Contrato do Renderizador final](0019-contrato-do-renderizador-final.md) fixa os campos e invariantes semânticos desse envelope. Progresso e cancelamento usam mensagens ou handles limitados à tentativa. Nenhum processo mantém uma segunda cópia mutável do Projeto como fonte canônica.

O baseline aceito é uma aplicação Tauri com Janelas e hosts separados, uma Sessão e um Processador de Imagens isolado por Projeto e um Processador temporário para o lote.

## Testes

Os testes atravessam as interfaces que representam comportamento observável:

- comandos aplicados pela interface de sessão do `ProjectCore`, incluindo invariantes, Undo/Redo e mudanças pendentes;
- carregamento, migração e Salvamento atômico pela interface do `ProjectCore`;
- autorização de Identidade, movimentação, Cópia externa e `Salvar como` pela superfície pública, sem montar estado local antes do terminal autorizado;
- casos dourados versionados do `CompositionCore`, reutilizados por prévia e Exportação conforme o [corpus do Renderizador final](../../tests/fixtures/final-renderer-cases-v1.json);
- propostas sem mutação do `MediaResolver`;
- invalidação e reconstrução descartável do `CacheEngine`;
- uma mesma suíte do `ExportPipeline` para Exportação normal e lote, com injeção de falhas antes e durante a Publicação;
- aquisição e liberação de concessões em sucesso, falha e cancelamento;
- transporte do envelope com o `RenderSnapshot` inteiro e os bindings, rejeitando documento, Sessão, Cache e campos criativos duplicados fora do snapshot.

Testes não dependem da quantidade final de crates nem atravessam seams internos apenas para observar detalhes de implementação.

## Decisões adiadas

- nomes finais, crates e visibilidade pública das subdivisões internas;
- codificação física, framing e materialização do adapter de IPC, sem alterar os campos e invariantes semânticos do envelope fechado pelo design 0019;
- formato concreto de `RenderSnapshot`, `CompositionPlan` e patches;
- algoritmo do Gerador de Layouts;
- biblioteca concreta de codecs/PDF e otimizações internas que preservem o contrato aceito do renderizador;
- quantidade futura de workers ou paralelismo entre Álbuns;
- qualquer ampliação do `CommandCatalog` para remapeamento;
- representações adicionais de Cache sem necessidade medida.
