---
status: accepted
document: design
date: 2026-08-03
updated: 2026-08-10
---

# Contrato público de persistência do ProjectCore

## Objetivo

Definir a menor fronteira pública para criar, abrir, carregar, salvar e executar `Salvar como` sem expor formato, migração, bytes persistidos ou confirmação de Salvamento à interface. A mesma fronteira só autoriza Cache, Recuperação e WebView2 depois de resolver a Identidade contra a instância física e a evidência local. Apesar de a decisão ter começado como “contrato público do `ProjectStore`”, a fronteira externa continua sendo `ProjectCore`: `ProjectStore` e `ProjectIdentityRegistry` são subdivisões internas, concretas e testáveis por meio do comportamento observável do núcleo.

Este contrato complementa [Armazenamento local e Cache](0010-armazenamento-local-e-cache.md), [Resolução e política de caminhos](0011-resolucao-e-politica-de-caminhos.md), [Propriedade de estado e módulos do núcleo](0012-propriedade-de-estado-e-modulos-do-nucleo.md), [Contrato do Arquivo de Projeto v1](0013-contrato-do-arquivo-de-projeto-v1.md), [Criação de Projeto](0003-criacao-de-projeto.md) e a sequência aprovada em [Provar o Salvamento atômico preservando a trava do Projeto](../research/fase-2-fluxo-persistente/issues/04-provar-o-salvamento-atomico-com-trava.md). Os nomes de tipos abaixo fixam responsabilidades e resultados; a implementação pode ajustar apenas detalhes idiomáticos que não alterem essas garantias.

## Modos de acesso e operações

Existem dois modos de acesso. Criação, abertura e carga entram por `ProjectCore`; Salvamento e `Salvar como` são transições do `EditableProject` já possuído:

| Operação pública | Modo | Sucesso |
|---|---|---|
| `create_editable(CreateProjectRequest)` | editável | novo `EditableProject` publicado, autorizado e bloqueado |
| `open_editable(OpenProjectRequest)` | editável | `EditableProject` carregado, autorizado e bloqueado, ou indicação de focalizar a sessão existente |
| `load_persisted_revision(LoadProjectRequest)` | somente leitura | `LoadedProjectRevision` imutável, sem autoridade para montar estado local por Projeto |
| `EditableProject::save(expectedRevision)` | editável | revisão corrente publicada na Localização atual |
| `EditableProject::save_as(SaveAsProjectRequest)` | editável | a mesma Sessão passa para um novo Projeto publicado, autorizado e bloqueado |
| `save_copy_as(SaveCopyAsRequest)` | criação editável a partir de fonte validada somente leitura | novo `EditableProject` criado em destino gravável sem alterar a fonte |

Criação, abertura, `Salvar como` e `Salvar cópia como...` permanecem operações diferentes porque recebem fontes, produzem efeitos e admitem falhas diferentes. Não existe uma enumeração genérica que esconda essas diferenças. O modo somente leitura não oferece comandos, Salvamento ou Undo/Redo e nunca escreve na origem.

`ProjectStore` e `ProjectIdentityRegistry` não são entradas nem APIs usadas pelo frontend. O primeiro possui DTOs, leitura, escrita determinística, migrações, `PersistedBaseline` e a prova privada de publicação; o segundo possui a última Localização autorizada por Identidade. `ProjectCore` coordena ambos com `ProjectSession`, leases e travas.

## Posse da sessão editável

`EditableProject` mantém inseparáveis durante toda a edição:

- a única `ProjectSession` mutável;
- o pathname nativo do Arquivo de Projeto;
- a autoridade opaca da Identidade localmente resolvida;
- o `ProjectIdentityLease` cross-process, retido por toda a Sessão;
- o `PersistedBaseline` privado, que encapsula o handle e a trava do arquivo atual, sua identidade física e os bytes exatos validados;
- o estado técnico de eventual migração em memória.

Ele oferece comandos de domínio, Undo/Redo, projeções, snapshots, Salvamento e `Salvar como`. Não oferece os antigos atalhos públicos `persisted_revision` ou `confirm_saved_revision`: nenhum chamador pode obter o documento serializado para publicá-lo por conta própria nem declarar uma revisão salva.

O Host serializa comandos sobre uma instância de `EditableProject`. Fechar consome essa instância e libera seus recursos; abandonar o processo também permite ao Windows liberar tanto o lease de Identidade quanto a trava física. O lease impede que duas cópias físicas com o mesmo `projectId` mantenham Sessões editáveis simultâneas e participa da classificação segura de Cópia externa. Ele é uma posse cross-process com ciclo de vida do sistema operacional, não um `HashSet` textual dentro de `ProjectCore` nem um catálogo persistente de Sessões. O registro de última Localização sobrevive ao fechamento, mas não representa posse ativa. Coordenação de Janelas, foco e Projetos recentes permanece fora do núcleo.

O `HashSet` em memória registrado no [gate da Fase 1](../research/0022-project-core-sessoes-e-revisoes-persistidas.md) permanece evidência histórica do proprietário único naquele corte, não parte deste contrato produtivo. Ao integrar pathname e arquivo reais, sua função é substituída pelo único `EditableProject` possuído pelo Host, pelo `ProjectIdentityLease` cross-process e pela trava física encapsulada no baseline; conservar também o registro textual duplicaria a decisão sem proteger aliases ou outros processos.

## Evidência durável e classificação da abertura

`ProjectIdentityRegistry` possui um registro fechado e atômico por Identidade em `%LOCALAPPDATA%\MyAlbuns\State\ProjectIdentities\{project-key}.json`. Ele conserva a Identidade canônica e a última Localização autorizada em `windowsUtf16`. Não é o arquivo de Projeto, não acompanha uma cópia para outra máquina, não é a lista de Projetos recentes e não pode ser removido por limpeza de Cache ou Recuperação.

A Localização registrada não é Identidade nem resultado físico memorizado. Ela apenas permite abrir de novo a instância anterior e obter evidência atual por handles. Durante uma tentativa, a identidade da Instância de arquivo do Projeto pertence aos objetos resolvidos; durante a Sessão, a evidência física fica retida pelo `PersistedBaseline` e pela trava do arquivo, enquanto `ProjectIdentityLease` conserva a exclusividade da Identidade e o alvo ativo autorizado. Fechar libera essas posses vivas e conserva o registro para a próxima tentativa.

A classificação ocorre antes de criar Sessão ou montar estado local:

| Situação observada | Classificação | Consequência |
|---|---|---|
| não existe registro nem lease ativo para a Identidade | primeira observação, fora da enumeração física | reserva a Identidade, verifica o candidato, publica o primeiro registro e só então pode autorizar a abertura nesta máquina |
| o candidato e o alvo ativo ou a última Localização acessível são a mesma instância por handles | `Same` | focaliza a Sessão ativa; sem Sessão ativa, abre normalmente e pode substituir o registro pelo alias adotado |
| a última Localização retorna `NotFound` sob raiz confirmadamente acessível | movimentação confirmada, não `Different` | preserva a Identidade e substitui o registro pela nova Localização depois de verificar e bloquear o candidato |
| a última Localização acessível e o candidato são instâncias distintas por handles e repetem a mesma Identidade | `Different` | caracteriza Cópia externa; a abertura editável precisa promover outra Identidade antes de qualquer estado local |
| a última Localização agora contém outro documento, um documento inválido ou uma Identidade diferente | `Different` sem origem correspondente | pathname reutilizado não caracteriza Cópia externa nem prova movimentação; retorna identidade inconclusiva e falha de forma fechada |
| o registro existe mas está corrompido/inacessível, a origem anterior está `Unavailable` ou `AccessDenied`, ou a comparação física não conclui | `Indeterminate` | falha de forma fechada, sem reescrever Identidade, registro ou arquivo e sem montar Cache, Recuperação ou WebView2 |

Igualdade ou diferença textual nunca decide uma linha da matriz. `Same` e `Different` só vêm do módulo de caminhos sobre objetos abertos; `NotFound` só autoriza movimentação quando a raiz anterior foi comprovadamente acessível. Dois processos concorrendo pela primeira observação são serializados pelo lease: apenas o vencedor publica o registro, e o outro reavalia a evidência já estabelecida.

Criação, primeira observação, `Same` sem Sessão ativa e movimentação confirmada publicam o registro aplicável antes do terminal editável. A promoção de uma Cópia externa publica um registro sob a nova Identidade e não altera o da origem. `Salvar como` faz o mesmo para o novo Projeto. Um registro local cuja publicação não possa ser provada impede o terminal autorizado, ainda que o arquivo do usuário permaneça válido ou tenha sido tecnicamente corrigido.

O sucesso editável produz uma autoridade opaca vinculada à Identidade resolvida. Cache, Recuperação e WebView2 aceitam essa autoridade, não um UUID apenas desserializado nem um pathname. `load_persisted_revision` pode consultar e atualizar o registro local necessário à classificação, mas seu resultado imutável não carrega autoridade para montar esses namespaces.

O arquivo de lease pode permanecer depois de uma queda; sua mera existência não significa Bloqueio de abertura ativo. Se o processo proprietário morreu, o sistema operacional libera a trava, a próxima aquisição comprova o Bloqueio órfão e reutiliza com segurança a Identidade. Conflito de trava com processo vivo continua `ProjectInUse`; falha ao distinguir os estados permanece fechada.

## Criação

`CreateProjectRequest` contém os valores iniciais já convertidos para as unidades autoritativas do domínio, o pathname nativo escolhido e uma autorização de gravação congelada pelo diálogo do Windows:

| Autorização | Significado | Regra no destino |
|---|---|---|
| `CreateOnly` | o diálogo concluiu com o destino livre | se qualquer objeto surgir antes da Publicação, retorna `DestinationConflict` e não o substitui |
| `ReplaceConfirmed` | o Windows avisou sobre o conflito e o usuário confirmou a substituição | pode substituir o arquivo regular naquele destino; não autoriza substituir um Projeto protegido por outro Host |

`ProjectCore` não volta a inferir consentimento consultando o destino. A autorização acompanha a tentativa desde o bootstrap até a Publicação. `ReplaceConfirmed` é a prova da decisão do usuário; `CreateOnly` nunca se transforma silenciosamente em substituição.

Depois da confirmação do diálogo, o núcleo valida o pedido, gera e reserva uma nova Identidade com `ProjectIdentityLease`, constrói o DTO atual com `revision: 0`, publica o candidato completo pela primitiva atômica aplicável, readquire e verifica o arquivo final, instala o baseline exato com sua trava física e publica o primeiro registro local. Somente então devolve `EditableProject` e sua autoridade de Identidade. O sucesso começa com `revision = savedRevision = 0`, Histórico vazio e nenhuma mudança criativa pendente. Qualquer terminal sem Sessão libera o lease adquirido e nunca autoriza um namespace.

Cancelar o assistente ou o diálogo ocorre antes dessa operação. Falha tratável não cria Sessão nem entrada em Projetos recentes e limpa artefatos privados quando sua posse é comprovada. Se a Publicação puder ter ocorrido, mas o estado final não puder ser provado, `CreateStateIndeterminate` não devolve Sessão e informa que o destino pode conter um Projeto completo que deve ser reinspecionado; nunca declara sucesso por aproximação.

Um destino protegido por uma Sessão existente retorna `ProjectInUse`, inclusive sob `ReplaceConfirmed`. A autorização para substituir conteúdo não é autorização para quebrar o Bloqueio de abertura.

## Abertura editável e carga somente leitura

`open_editable` executa a identificação, leitura estrita, migração em memória, mapeamento para o domínio e validação definidas no contrato v1. Antes de devolver a Sessão, também aplica a matriz de evidência, adquire o `ProjectIdentityLease` e a trava física, conserva os bytes exatos validados, instala o `PersistedBaseline` e publica o registro aplicável. O lease e a evidência física distinguem alvo já protegido, alias, movimentação e Cópia externa antes de permitir edição. `Same` com Sessão ativa devolve uma indicação estruturada de focalização ao coordenador; `Indeterminate` falha sem criar Sessão.

Uma Cópia externa gravável mantém a barreira da Identidade repetida enquanto adquire a nova, reescreve estritamente somente `projectId` no mesmo schema e verifica arquivo, baseline e registro sob as duas barreiras. Apenas o terminal completo libera a barreira antiga e produz autoridade para a nova Identidade. Se a fonte não puder receber a correção por somente leitura ou falta de permissão, `open_editable` devolve `ExternalCopyNotWritable` com uma fonte opaca e validada para a ação `Salvar cópia como...`; nenhuma Sessão ou autoridade local é criada para a Identidade repetida.

`load_persisted_revision` reutiliza leitura, migração, classificação e validação, mas devolve somente um valor imutável suficiente para produzir snapshots. Ela não cria `ProjectSession`, não retém um Bloqueio de abertura editável, não promove schema, não corrige a origem e não autoriza estado local por Projeto. Uma Cópia externa que depender de nova Identidade retorna `ExternalCopyRequiresInteractiveResolution`.

O `ProjectCore` não altera Projetos recentes. O coordenador só promove uma entrada depois que o Host devolver o terminal `Ready` para uma criação ou abertura bem-sucedida. Reutilizar ou focalizar uma Janela já existente também é responsabilidade do coordenador e usa a indicação estruturada de `Same`; um bloqueio vivo que não possa ser encaminhado continua `ProjectInUse`.

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

## Contrato de `Salvar como`

`SaveAsProjectRequest` contém somente a revisão esperada, a `ProjectLocation` totalmente qualificada e já vinculada para a tentativa e a mesma autorização congelada `CreateOnly` ou `ReplaceConfirmed` produzida pelo diálogo nativo. Cancelar o diálogo acontece antes da chamada e não constitui erro do núcleo.

`EditableProject::save_as` segue estas garantias:

1. compara `expectedRevision` antes de I/O e congela exatamente o estado visível daquela revisão;
2. rejeita destino que seja `Same` em relação ao arquivo atual, pois não existe novo Projeto se a origem for substituída, e falha fechada se essa comparação for `Indeterminate`;
3. gera e reserva uma nova Identidade; ao substituir outro Projeto confirmado, também respeita o lease de sua Identidade e nunca quebra uma Sessão ativa;
4. escreve o DTO da versão atual com a nova Identidade e a mesma revisão criativa, por publicação atômica e verificação equivalente à Criação;
5. publica o registro local da nova Identidade antes de produzir sua autoridade; o registro e o arquivo da Identidade anterior permanecem intactos;
6. instala o novo `ProjectStore`, `PersistedBaseline`, pathname, lease e autoridade antes de liberar as posses da Identidade anterior;
7. conserva estado criativo, revisão corrente e Histórico da sessão, confirma essa revisão como `savedRevision` do novo Projeto e encerra qualquer `storageUpgradePending` no novo arquivo;
8. não copia Cache, WebView2 ou metadados derivados; o novo namespace começa vazio e só pode ser montado com a nova autoridade;
9. remove o checkpoint de Recuperação da Identidade anterior como parte do terminal bem-sucedido e direciona mudanças posteriores exclusivamente à nova Identidade.

O original nunca recebe o estado visível não salvo por efeito dessa operação: ele permanece byte a byte na última Revisão salva que possuía. Depois do sucesso, reabri-lo cria outra Sessão com Histórico vazio; Undo/Redo conservado no `EditableProject` corrente só altera o novo Projeto. Assim, compartilhar os mesmos Arquivos vinculados não cria sincronização entre os dois.

Até o terminal completo, Sessão, Localização, Identidade, Cache e Recuperação anteriores permanecem em vigor. Cancelamento, destino inválido ou indisponível, `StaleRevision`, conflito, falha conclusiva de escrita ou recusa da autorização não realiza a troca. Se a Publicação no destino puder ter concluído mas não puder ser provada, `SaveAsStateIndeterminate` mantém a Sessão no Projeto anterior e informa que o destino pode conter um Projeto completo a ser reinspecionado; nunca adota um arquivo por aproximação nem deixa a interface fabricar a transição.

`Salvar cópia como...` é uma operação pública distinta para o terminal `ExternalCopyNotWritable`. `SaveCopyAsRequest` recebe a fonte opaca e validada devolvida pela abertura recusada, um destino e sua autorização; não recebe JSON, tipos do `ProjectStore` nem uma `ProjectSession` fictícia. Ela publica no destino o estado persistido validado da fonte, em schema atual, com nova Identidade e mesma revisão, deixa a fonte somente leitura intacta e devolve um `EditableProject` novo com Histórico vazio. Suas garantias de destino, registro, autoridade, namespace vazio e estado indeterminado são as mesmas de `Salvar como`.

As duas operações podem compartilhar uma primitiva privada de publicação de um Projeto novo, mas não uma enumeração pública que apague a diferença entre fonte viva e fonte persistida recusada. A interface recebe apenas resultados estruturados e a nova projeção; bytes, DTOs, recibos de publicação e a troca de Identidade continuam privados.

## Cancelamento e fechamento

Criação, abertura, Salvamento, `Salvar como` e `Salvar cópia como...` são operações curtas e não canceláveis depois que `ProjectCore` aceita o pedido. Cancelar o assistente, o diálogo nativo ou a confirmação de fechamento ocorre antes da chamada. Portanto, `Cancelled` não é variante de erro de `ProjectStore`, e o núcleo não recebe um token de cancelamento que possa interromper a região de Publicação.

Ao fechar uma Sessão com mudanças criativas, a coordenação da Janela oferece:

| Escolha | Comportamento |
|---|---|
| `Salvar e fechar` | bloqueia novas edições, chama `save` para a revisão corrente e só consome `EditableProject` após `Saved` ou `AlreadyCurrent` |
| `Descartar e fechar` | consome `EditableProject` sem chamar `ProjectStore` |
| `Cancelar` | mantém a Sessão sem iniciar I/O |

Conflito ou falha conclusiva mantém a Janela aberta com os estados `creativeDirty` e `storageUpgradePending` anteriores intactos. `SaveStateIndeterminate` é terminal de segurança e encerra a Sessão mesmo quando a tentativa começou por `Salvar e fechar`. Uma Sessão sem mudanças criativas fecha diretamente.

## Erros estruturados

Cada operação possui seu próprio tipo de erro: `CreateProjectError`, `OpenProjectError`, `LoadProjectError`, `SaveProjectError`, `SaveAsProjectError` e `SaveCopyAsError`. Eles podem reutilizar valores menores, mas não formam um `CoreError` monolítico com variantes impossíveis para a operação.

As categorias compartilhadas preservam, no mínimo:

- `PathFailure`: `NotFound`, `Unavailable`, `AccessDenied`, `InvalidPath`, `UnexpectedObjectType`, `Conflict` e `IoFailure`;
- `DocumentFailure`: `InvalidDocumentType`, `UnsupportedFutureSchema`, `UnsupportedLegacySchema`, `InvalidProjectDocument` e `InvalidProjectState`;
- ciclo de vida e identidade: `ProjectInUse`, evidência ou registro de identidade inconclusivo, `ExternalCopyRequiresInteractiveResolution` e `ExternalCopyNotWritable` onde forem aplicáveis.

Criação acrescenta `DestinationConflict` e `CreateStateIndeterminate`. Salvamento acrescenta `StaleRevision`, `PersistedBaselineConflict` e `SaveStateIndeterminate`. `Salvar como` acrescenta `StaleRevision`, `SameTarget`, `DestinationConflict` e `SaveAsStateIndeterminate`; `Salvar cópia como...` não admite `StaleRevision` porque sua fonte é persistida e imutável. Cancelamento anterior à chamada não aparece em nenhum desses erros.

No limite Tauri, sucesso e erro atravessam como código estável e contexto estruturado. A interface escolhe a mensagem e as ações adequadas; strings do sistema operacional e cadeias de causas ficam disponíveis para Logs e diagnóstico, não como o contrato de controle do frontend.

## Verificação obrigatória

A implementação precisa demonstrar pela superfície de `ProjectCore`:

- criação `CreateOnly`, substituição `ReplaceConfirmed`, corrida no destino e `ProjectInUse`;
- ausência de Sessão antes da publicação, verificação e trava da criação;
- primeira observação, registro que sobrevive ao fechamento, `Same`, movimentação por `NotFound`, `Different` e todos os caminhos de `Indeterminate` sem usar igualdade textual;
- abertura estrita, migração em memória e carga somente leitura sem escrita na origem nem autoridade de namespace;
- Cópia externa gravável recebendo e registrando nova Identidade antes de estado local, e fonte não gravável oferecendo somente `Salvar cópia como...` ou cancelamento;
- pedido obsoleto recusado antes de I/O;
- sucesso de Salvamento confirmando exatamente a revisão candidata;
- conflito quando identidade, objeto ou bytes do baseline mudarem, inclusive com revisão numérica igual;
- `AlreadyCurrent` sem escrita e promoção de schema sem nova revisão criativa;
- falhas antes, durante e depois da Publicação, distinguindo estado final comprovado de estado inconclusivo;
- `Salvar como` preservando o original, Histórico e estado visível no novo Projeto, com nova Identidade, Localização, baseline e namespace vazio;
- cancelamento e falhas de `Salvar como` preservando Sessão, Cache e Recuperação anteriores, além da variante a partir de Cópia externa somente leitura;
- fechamento por salvar, descartar e cancelar;
- impossibilidade de um chamador público serializar o documento ou confirmar `savedRevision` diretamente;
- transporte Tauri dos resultados como dados estruturados, sem decisão baseada em texto.

Esses casos atravessam `ProjectCore`, não fronteiras internas inventadas apenas para testes.

## Limites do contrato

Este design define operações, resultados e garantias técnicas; não distribui trabalho executável. O escopo e os critérios de aceite de cada entrega pertencem exclusivamente ao respectivo ticket de entrega. A presença de uma operação pública neste contrato não autoriza atribuí-la a um ticket que não a declare.

O contrato não introduz um registro global mutável de sessões, uma API genérica `Store<T>`, cancelamento intermediário, mesclagem de alterações externas ou recuperação automática após resultado inconclusivo. `ProjectIdentityRegistry` registra somente a última Localização autorizada; `ProjectIdentityLease` é apenas a posse cross-process mínima exigida pela exclusividade, e nenhum dos dois vira coordenador universal.
