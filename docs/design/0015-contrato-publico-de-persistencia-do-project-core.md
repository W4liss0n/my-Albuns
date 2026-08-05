---
status: accepted
document: design
date: 2026-08-03
---

# Contrato público de persistência do ProjectCore

## Objetivo

Definir a menor fronteira pública para criar, abrir, carregar e salvar Projetos sem expor formato, migração, bytes persistidos ou confirmação de Salvamento à interface. Apesar de a decisão ter começado como “contrato público do `ProjectStore`”, o seam externo continua sendo `ProjectCore`: `ProjectStore` é uma subdivisão interna, concreta e testável por meio do comportamento observável do núcleo.

Este contrato complementa [Propriedade de estado e módulos do núcleo](0012-propriedade-de-estado-e-modulos-do-nucleo.md), [Contrato do Arquivo de Projeto v1](0013-contrato-do-arquivo-de-projeto-v1.md), [Criação de Projeto](0003-criacao-de-projeto.md) e a sequência aprovada em [Provar o Salvamento atômico preservando a trava do Projeto](../research/fase-2-fluxo-persistente/issues/04-provar-o-salvamento-atomico-com-trava.md). Os nomes de tipos abaixo fixam responsabilidades e resultados; a implementação pode ajustar apenas detalhes idiomáticos que não alterem essas garantias.

## Modos de acesso e operações

Existem dois modos de acesso e três operações públicas:

| Operação do `ProjectCore` | Modo | Sucesso | Cria `ProjectSession`? |
|---|---|---|---|
| `create_editable(CreateProjectRequest)` | editável | `EditableProject` criado, publicado, verificado e bloqueado | sim |
| `open_editable(OpenProjectRequest)` | editável | `EditableProject` carregado, validado e bloqueado | sim |
| `load_persisted_revision(LoadProjectRequest)` | somente leitura | `LoadedProjectRevision` imutável | não |

Criação e abertura permanecem operações diferentes porque recebem dados, produzem efeitos e admitem falhas diferentes. Não existe uma enumeração genérica que esconda essa diferença. O modo somente leitura não oferece comandos, Salvamento ou Undo/Redo e nunca escreve na origem.

`ProjectStore` não é uma quarta entrada nem uma API usada pelo frontend. Ele possui DTOs, leitura, escrita determinística, migrações, `PersistedBaseline` e a prova privada de publicação; `ProjectCore` coordena essas capacidades com `ProjectSession`.

## Posse da sessão editável

`EditableProject` mantém inseparáveis durante toda a edição:

- a única `ProjectSession` mutável;
- o pathname nativo do Arquivo de Projeto;
- o `ProjectIdentityLease` cross-process, retido por toda a Sessão;
- o `PersistedBaseline` privado, que encapsula o handle e a trava do arquivo atual, sua identidade física e os bytes exatos validados;
- o estado técnico de eventual migração em memória.

Ele oferece comandos de domínio, Undo/Redo, projeções, snapshots e Salvamento. Não oferece os antigos atalhos públicos `persisted_revision` ou `confirm_saved_revision`: nenhum chamador pode obter o documento serializado para publicá-lo por conta própria nem declarar uma revisão salva.

O Host serializa comandos sobre uma instância de `EditableProject`. Fechar consome essa instância e libera seus recursos; abandonar o processo também permite ao Windows liberar tanto o lease de Identidade quanto a trava física. O lease impede que duas cópias físicas com o mesmo `projectId` mantenham Sessões editáveis simultâneas e participa da classificação segura de Cópia externa. Ele é uma posse cross-process com ciclo de vida do sistema operacional, não um `HashSet` textual dentro de `ProjectCore` nem um catálogo persistente de Sessões. Coordenação de Janelas, foco e Projetos recentes permanece fora do núcleo.

O `HashSet` em memória registrado no [gate da Fase 1](../research/0022-project-core-sessoes-e-revisoes-persistidas.md) permanece evidência histórica do proprietário único naquele corte, não parte deste contrato produtivo. Ao integrar pathname e arquivo reais, sua função é substituída pelo único `EditableProject` possuído pelo Host, pelo `ProjectIdentityLease` cross-process e pela trava física encapsulada no baseline; conservar também o registro textual duplicaria a decisão sem proteger aliases ou outros processos.

## Criação

`CreateProjectRequest` contém os valores iniciais já convertidos para as unidades autoritativas do domínio, o pathname nativo escolhido e uma autorização de gravação congelada pelo diálogo do Windows:

| Autorização | Significado | Regra no destino |
|---|---|---|
| `CreateOnly` | o diálogo concluiu com o destino livre | se qualquer objeto surgir antes da Publicação, retorna `DestinationConflict` e não o substitui |
| `ReplaceConfirmed` | o Windows avisou sobre o conflito e o usuário confirmou a substituição | pode substituir o arquivo regular naquele destino; não autoriza substituir um Projeto protegido por outro Host |

`ProjectCore` não volta a inferir consentimento consultando o destino. A autorização acompanha a tentativa desde o bootstrap até a Publicação. `ReplaceConfirmed` é a prova da decisão do usuário; `CreateOnly` nunca se transforma silenciosamente em substituição.

Depois da confirmação do diálogo, o núcleo valida o pedido, gera e reserva uma nova Identidade com `ProjectIdentityLease`, constrói o DTO atual com `revision: 0`, publica o candidato completo pela primitiva atômica aplicável, readquire e verifica o arquivo final, instala o baseline exato com sua trava física e somente então devolve `EditableProject`. O sucesso começa com `revision = savedRevision = 0`, Histórico vazio e nenhuma mudança criativa pendente. Qualquer terminal sem Sessão libera o lease adquirido.

Cancelar o assistente ou o diálogo ocorre antes dessa operação. Falha tratável não cria Sessão nem entrada em Projetos recentes e limpa artefatos privados quando sua posse é comprovada. Se a Publicação puder ter ocorrido, mas o estado final não puder ser provado, `CreateStateIndeterminate` não devolve Sessão e informa que o destino pode conter um Projeto completo que deve ser reinspecionado; nunca declara sucesso por aproximação.

Um destino protegido por uma Sessão existente retorna `ProjectInUse`, inclusive sob `ReplaceConfirmed`. A autorização para substituir conteúdo não é autorização para quebrar o Bloqueio de abertura.

## Abertura editável e carga somente leitura

`open_editable` executa a identificação, leitura estrita, migração em memória, mapeamento para o domínio e validação definidas no contrato v1. Antes de devolver a Sessão, também resolve a Identidade conforme a política aceita, adquire o `ProjectIdentityLease` e a trava física, conserva os bytes exatos validados e instala o `PersistedBaseline`. O lease e a evidência física distinguem alvo já protegido de Cópia externa antes de permitir edição; uma tentativa concorrente sobre o alvo protegido retorna `ProjectInUse`, e evidência inconclusiva falha de forma fechada sem criar Sessão.

`load_persisted_revision` reutiliza leitura, migração e validação, mas devolve somente um valor imutável suficiente para produzir snapshots. Ela não cria `ProjectSession`, não retém um Bloqueio de abertura editável e não promove schema ou corrige a origem. Uma Cópia externa que depender de resolução interativa retorna `ExternalCopyRequiresInteractiveResolution`.

O `ProjectCore` não altera Projetos recentes. O coordenador só promove uma entrada depois que o Host devolver o terminal `Ready` para uma criação ou abertura bem-sucedida. Reutilizar ou focalizar uma Janela já existente também é responsabilidade do coordenador; nesta fase, o resultado estruturado disponível para um alvo bloqueado é `ProjectInUse`.

## Handshake de Salvamento

O único Salvamento público da sessão é conceitualmente `save(expectedRevision)`. A revisão esperada protege o pedido entre a UI e a Sessão; não prova concorrência do arquivo.

1. `ProjectCore` compara `expectedRevision` com a revisão corrente antes de I/O. Divergência retorna `StaleRevision` sem tocar no arquivo.
2. O Host impede novos comandos, e `ProjectCore` congela uma candidata imutável daquela revisão.
3. `ProjectStore` mapeia a candidata para o DTO atual e produz bytes determinísticos.
4. Sob a barreira estável, o Store exige que identidade física e bytes do destino ainda correspondam ao `PersistedBaseline`, publica a candidata e verifica o novo handle e seus bytes exatos.
5. Somente essa prova substitui o baseline e produz um `SaveReceipt` privado, vinculado à Identidade, à revisão e à candidata publicada.
6. `ProjectCore` consome o recibo e confirma privadamente a Revisão salva na `ProjectSession`.
7. A UI recebe apenas o resultado estruturado; nunca recebe ou fabrica `SaveReceipt`.

Como os comandos ficam serializados durante o I/O, a revisão não pode mudar entre o congelamento e a confirmação. Uma divergência nessa etapa é violação interna do protocolo, invalida a Sessão de forma fechada e nunca é apresentada como conflito normal do usuário.

### Estado criativo e atualização técnica

Dois estados transitórios permanecem separados:

- `creativeDirty` indica que a revisão corrente difere da Revisão salva e exige decisão ao fechar;
- `storageUpgradePending` indica que um schema suportado foi migrado somente em memória, sem alteração criativa.

Salvar sem nenhum dos dois retorna `AlreadyCurrent { revision }` sem I/O. Salvar apenas com `storageUpgradePending` publica o DTO atual, preserva a revisão e retorna `Saved { revision }`. Quando ambos existem, uma única publicação persiste o conteúdo e promove o schema. Fechar sem alteração criativa não apresenta confirmação apenas por uma migração pendente e deixa o documento antigo intacto.

## Resultados do Salvamento

| Resultado | Efeito no arquivo | Efeito em `EditableProject` |
|---|---|---|
| `Saved { revision }` | candidata publicada e verificada | confirma `savedRevision`, atualiza o baseline e mantém a Sessão válida |
| `AlreadyCurrent { revision }` | nenhum I/O | mantém a Sessão válida |
| `StaleRevision { expected, current }` | nenhum I/O | mantém a Sessão válida para o chamador atualizar sua projeção |
| `PersistedBaselineConflict` | não substitui o destino divergente | mantém a Sessão válida e preserva separadamente `creativeDirty` e `storageUpgradePending`; não mescla, recarrega ou sobrescreve automaticamente |
| falha conclusiva de caminho ou I/O | preserva o estado final comprovado | mantém a Sessão válida e preserva os dois flags anteriores; uma nova tentativa pode ser oferecida quando fizer sentido |
| `SaveStateIndeterminate` | o núcleo não afirma qual versão ocupa o destino | não confirma a revisão e invalida a Sessão; o Host encerra a edição e exige reabertura |

Revisão igual com bytes diferentes continua sendo `PersistedBaselineConflict`. `revision` não substitui a comparação física e byte a byte.

## Cancelamento e fechamento

Criação, abertura e Salvamento são operações curtas e não canceláveis depois que `ProjectCore` aceita o pedido. Cancelar o assistente, o diálogo nativo ou a confirmação de fechamento ocorre antes da chamada. Portanto, `Cancelled` não é variante de erro de `ProjectStore`, e o núcleo não recebe um token de cancelamento que possa interromper a região de Publicação.

Ao fechar uma Sessão com mudanças criativas, a coordenação da Janela oferece:

| Escolha | Comportamento |
|---|---|
| `Salvar e fechar` | bloqueia novas edições, chama `save` para a revisão corrente e só consome `EditableProject` após `Saved` ou `AlreadyCurrent` |
| `Descartar e fechar` | consome `EditableProject` sem chamar `ProjectStore` |
| `Cancelar` | mantém a Sessão sem iniciar I/O |

Conflito ou falha conclusiva mantém a Janela aberta com os estados `creativeDirty` e `storageUpgradePending` anteriores intactos. `SaveStateIndeterminate` é terminal de segurança e encerra a Sessão mesmo quando a tentativa começou por `Salvar e fechar`. Uma Sessão sem mudanças criativas fecha diretamente.

## Erros estruturados

Cada operação possui seu próprio tipo de erro: `CreateProjectError`, `OpenProjectError`, `LoadProjectError` e `SaveProjectError`. Eles podem reutilizar valores menores, mas não formam um `CoreError` monolítico com variantes impossíveis para a operação.

As categorias compartilhadas preservam, no mínimo:

- `PathFailure`: `NotFound`, `Unavailable`, `AccessDenied`, `InvalidPath`, `UnexpectedObjectType`, `Conflict` e `IoFailure`;
- `DocumentFailure`: `InvalidDocumentType`, `UnsupportedFutureSchema`, `UnsupportedLegacySchema`, `InvalidProjectDocument` e `InvalidProjectState`;
- ciclo de vida e identidade: `ProjectInUse`, evidência de identidade inconclusiva e `ExternalCopyRequiresInteractiveResolution` onde forem aplicáveis.

Criação acrescenta `DestinationConflict` e `CreateStateIndeterminate`. Salvamento acrescenta `StaleRevision`, `PersistedBaselineConflict` e `SaveStateIndeterminate`. Cancelamento anterior à chamada não aparece em nenhum desses erros.

No limite Tauri, sucesso e erro atravessam como código estável e contexto estruturado. A interface escolhe a mensagem e as ações adequadas; strings do sistema operacional e cadeias de causas ficam disponíveis para Logs e diagnóstico, não como o contrato de controle do frontend.

## Verificação obrigatória

A implementação precisa demonstrar pela superfície de `ProjectCore`:

- criação `CreateOnly`, substituição `ReplaceConfirmed`, corrida no destino e `ProjectInUse`;
- ausência de Sessão antes da publicação, verificação e trava da criação;
- abertura estrita, migração em memória e carga somente leitura sem escrita;
- pedido obsoleto recusado antes de I/O;
- sucesso de Salvamento confirmando exatamente a revisão candidata;
- conflito quando identidade, objeto ou bytes do baseline mudarem, inclusive com revisão numérica igual;
- `AlreadyCurrent` sem escrita e promoção de schema sem nova revisão criativa;
- falhas antes, durante e depois da Publicação, distinguindo estado final comprovado de estado inconclusivo;
- fechamento por salvar, descartar e cancelar;
- impossibilidade de um chamador público serializar o documento ou confirmar `savedRevision` diretamente;
- transporte Tauri dos resultados como dados estruturados, sem decisão baseada em texto.

Esses casos atravessam `ProjectCore`, não seams internos inventados apenas para testes. O próximo protótipo multiprocesso deve reutilizar este contrato junto ao bootstrap e ao `ExportPipeline`.

## Limites desta decisão

Este contrato não introduz `Salvar como`, um registro global mutável de sessões, uma API genérica `Store<T>`, cancelamento intermediário, mesclagem de alterações externas ou recuperação automática após resultado inconclusivo. O `ProjectIdentityLease` é apenas a posse cross-process mínima exigida pela exclusividade já aceita, não um coordenador. O contrato também não conclui a experiência de Cópia externa, foco por alias ou múltiplos Projetos; preserva somente as classificações e garantias necessárias para não abrir uma segunda edição insegura.
