---
status: accepted
document: design
---

# Resolução e política de caminhos

## Objetivo

Concentrar em um módulo Rust compartilhado as regras de caminhos do MyAlbuns. A interface deve esconder diferenças entre discos locais, UNC, unidades mapeadas e caminhos verbatim de disco/UNC, sem transformar caminhos em identidade de Projeto ou em uma fonte de verdade alternativa aos arquivos.

Este design detalha o [ADR 0007](../adr/0007-tratar-caminhos-windows-e-identidade-fisica.md). Escolhas de crates são substituíveis e não fazem parte de sua interface.

## Responsabilidades

O módulo:

- descobre as raízes conhecidas do Windows usadas em `%APPDATA%` e `%LOCALAPPDATA%`;
- constrói os locais de Configurações, Layouts, Estado, Logs, Cache e Recuperação;
- classifica e valida caminhos recebidos de diálogos, Explorer, arraste e arquivos de Projeto;
- produz caminhos operacionais para Projetos, Arquivos vinculados, Photoshop, origens e destinos de lote;
- captura bindings de raiz imutáveis para todos os processos participantes da mesma tentativa;
- compara objetos existentes por identidade física quando a operação exigir;
- cria a preparação da Exportação dentro da própria pasta de Destino;
- traduz falhas de I/O em resultados tipados para os chamadores.

O módulo não:

- lê ou interpreta o documento de Projeto;
- decide se uma mídia participa de uma Exportação;
- incorpora, copia ou altera Arquivos vinculados;
- mantém um catálogo persistente de unidades ou compartilhamentos;
- transforma Cache de mídia em substituto de um original;
- oferece acesso arbitrário ao sistema de arquivos para React.

## Formas aceitas

Entradas externas precisam ser totalmente qualificadas. A primeira versão aceita:

| Forma | Exemplo | Tratamento |
|---|---|---|
| Disco local | `C:\Albuns\001.myalbuns` | aceita |
| UNC | `\\servidor\Albuns\001.myalbuns` | aceita |
| Unidade mapeada | `Z:\Albuns\001.myalbuns` | aceita e pode ser associada à raiz UNC durante uma operação |
| Verbatim local | `\\?\C:\caminho-longo\001.myalbuns` | aceita |
| Verbatim UNC | `\\?\UNC\servidor\Albuns\001.myalbuns` | aceita |
| Verbatim genérico | `\\?\GLOBALROOT\...` ou outra forma fora de disco/UNC | rejeita |
| Relativo ou dependente do processo | `Albuns\001.myalbuns`, `.\001.myalbuns`, `..\001.myalbuns`, `C:001.myalbuns` ou `\Albuns\001.myalbuns` | rejeita na interface externa |
| Namespace de dispositivo | `\\.\PIPE\...` ou equivalente | rejeita |
| Curinga | `C:\Fotos\*.jpg` | rejeita |
| Fluxo alternativo | `C:\Fotos\a.jpg:preview` | rejeita; somente o `:` do prefixo da unidade é aceito |
| Componente ou alias reservado | `C:\Fotos\NUL.jpg` ou equivalente | rejeita |

Caminhos relativos continuam permitidos internamente apenas como sufixos produzidos pelo próprio módulo sob uma raiz já validada, como na reprodução da árvore de um lote.

A validação textual não prova o tipo do alvo. Para todo caminho externo existente, a abertura autoritativa confirma que o handle representa um objeto de disco do tipo exigido pelo propósito: arquivo regular para Projeto, mídia e executável; diretório para origem, Destino e pasta escolhida. Objeto especial ou divergência entre arquivo e diretório produz erro tipado.

Quando o alvo ainda não existe porque a operação o criará, o módulo abre e valida o diretório pai existente, deriva o filho por componentes seguros e, depois da criação, confirma pelo handle o tipo esperado e a permanência sob o pai autorizado.

## Interface do módulo

A forma concreta dos tipos será validada no spike, mas a interface esconde três valores coesos:

- `AppPaths`, descoberto uma vez por processo para os dados do aplicativo;
- `OperationPathContext`, contexto efêmero do proprietário que resolve e reutiliza raízes durante o planejamento;
- `RootBindingPlan`, captura imutável produzida ao congelar o contexto e usada nas fases distribuídas da tentativa.

```rust
let app_paths = AppPaths::discover()?;
let mut owner_paths = OperationPathContext::new(OperationKind::Export);

let source = owner_paths.resolve_existing(source_path, ExpectedObject::RegularFile)?;
let destination = owner_paths.resolve_destination(destination_path)?;
let plan = owner_paths.freeze()?;

// Quando a fase seguinte existir em outro processo:
ipc.send(render_snapshot, plan.clone())?;

// No processo participante:
let worker_paths = OperationPathContext::from_plan(plan)?;
```

`ResolvedPath` é um valor opaco. Ele conserva a forma escolhida para apresentação, a forma operacional nativa e a raiz classificada sem obrigar chamadores a conhecer regras de prefixo. Somente o módulo pode derivar caminhos filhos ou locais temporários a partir dele.

A prova temporária da primeira versão também entra por `AppPaths`: o módulo prepara `%TEMP%\MyAlbuns2\ExportPreview`, valida cada filho contra a raiz temporária por handle e conserva os guards até o fim da tentativa. O comando recebe o diretório preparado, sem conhecer o namespace transitório do produto nem criar diretórios por conta própria. Esse local descartável não integra as árvores persistentes de `%APPDATA%` e `%LOCALAPPDATA%`; nos testes, sua raiz é injetada junto às demais para não escrever no `%TEMP%` compartilhado.

`RootBindingPlan` contém somente a raiz lógica, seu tipo, o binding operacional capturado e a representação nativa escolhida para I/O. Ele não contém existência, metadados ou identidade de arquivos individuais, capacidades genéricas do servidor nem promessa sobre o servidor físico que atenderá os acessos.

Quando necessário, a comparação entre dois objetos existentes produz `Same`, `Different` ou `Indeterminate`; falha ao obter identidade física nunca é convertida em `Different`. O módulo fornece a evidência e não escolhe a política funcional do chamador.

Produção usa o adaptador do sistema de arquivos do Windows. Testes usam um adaptador controlado com raízes temporárias e respostas simuladas de rede; ambos exercitam a mesma interface. Bibliotecas candidatas, sem fazer parte da interface, são `directories::BaseDirs` para localizar as raízes conhecidas, `same-file` apenas como evidência auxiliar de equivalência, `dunce` somente para simplificação segura de apresentação e bindings do Windows quando as abstrações anteriores não bastarem.

## Contexto temporário por operação

Cada operação de vários arquivos cria seu próprio contexto vazio:

- Importação de seleção ou pasta;
- geração de Cache;
- Religação com busca;
- Exportação normal;
- Geração de Projetos em lote;
- Exportação em lote.

O proprietário cria um contexto vazio e o usa durante descoberta, pré-validação e planejamento. Na primeira resolução de uma raiz, o contexto registra somente fatos obtidos com sucesso e necessários para reutilizá-la: tipo da raiz, associação de unidade mapeada, raiz operacional e representação nativa escolhida. Ele não guarda como verdade a existência, tamanho, data, conteúdo ou identidade individual de arquivos sob a raiz.

Antes de despachar a fase distribuída, o proprietário congela o contexto em um `RootBindingPlan`. Uma unidade mapeada pode estar associada à raiz UNC correspondente; esse binding operacional permanece capturado durante a tentativa, e caminhos posteriores com o mesmo prefixo de componentes reutilizam a associação e acrescentam o sufixo relativo sem repetir a descoberta completa. Capacidade de substituição atômica ou outra característica do Destino não entra como fato genérico do plano; o consumidor concreto verifica o suporte quando executa a ação e continua obrigado a tratar a falha real.

O reaproveitamento é feito por componentes, nunca por prefixo textual. `C:\Fotos` não é raiz de `C:\Fotos antigas`, e `\\servidor\A` não é raiz de `\\servidor\Album`.

Não existe TTL, arquivo de índice, limpeza ou cache global:

- um contexto proprietário nasce com a tentativa lógica e acumula somente bindings durante o planejamento;
- o contexto é congelado em um `RootBindingPlan` antes do trabalho distribuído;
- cada processo participante cria sua visão local somente leitura a partir desse mesmo plano;
- o plano pode atravessar IPC somente entre participantes da tentativa;
- plano e contextos são descartados em sucesso, falha ou cancelamento;
- uma nova tentativa, inclusive retomada após reinício, captura novamente suas raízes.

Um worker não resolve silenciosamente por conta própria uma raiz externa ausente do plano. Ele devolve erro tipado ao proprietário e encerra a tentativa; somente uma nova tentativa pode retornar ao planejamento e criar outro plano.

Esse contexto técnico é diferente do mapa temporário de Religação usado pela Exportação em lote: o primeiro reutiliza somente fatos de raiz para I/O; o segundo associa originais ausentes encontrados pelo usuário e segue seu próprio ciclo funcional.

Se um acesso pelo binding capturado falhar durante a execução, a tentativa termina com falha recuperável. Remapear a letra durante a tentativa não redireciona o trabalho já iniciado: ele continua usando a raiz operacional capturada até falhar ou concluir. Somente `Tentar novamente` explícito cria outro contexto e resolve novamente a associação atual; nenhuma ação final é repetida automaticamente.

O binding estabiliza a representação operacional escolhida pelo aplicativo; não fixa a identidade do servidor físico. DFS, DNS, cluster, SMB ou o armazenamento subjacente ainda podem redirecionar o atendimento sem que o MyAlbuns passe a seguir um novo mapeamento deliberado da letra.

## Caminho, identidade e bloqueio

O Projeto persiste sua Identidade própria e os caminhos escolhidos pelo usuário. Canonicalização não substitui nenhum dos dois.

Para um arquivo existente, duas formas textuais podem representar o mesmo alvo, como uma unidade mapeada e seu UNC. Quando essa distinção for necessária, o módulo compara a identidade física obtida por handles. A comparação pode ser inconclusiva em determinados sistemas de arquivos ou servidores.

A comparação de identidade encerra sua responsabilidade ao retornar a evidência tri-state. O módulo também expõe a primitiva nativa de trava de arquivo, sem decidir identidade, foco, conflito funcional ou duração da posse. O guardião de abertura do Projeto aplica essa política e mantém a primitiva pelo mesmo ciclo de vida da Sessão editável:

1. o arquivo é aberto e sua Identidade persistida é lida;
2. a identidade física é comparada com as sessões abertas;
3. `Same` focaliza a sessão existente;
4. somente `Different` combinado à mesma Identidade persistida caracteriza uma Cópia externa;
5. o bloqueio real do arquivo decide se uma nova sessão editável pode ser criada;
6. `Indeterminate` ou conflito de bloqueio falha de forma fechada e nunca reescreve a Identidade.

Para itens do Painel, a regra funcional continua sendo a duplicação do mesmo caminho dentro da mesma aba. A identidade física não mescla automaticamente vínculos que o usuário importou por representações diferentes.

## Rede e estados de erro

Chamadas que podem alcançar rede não executam na thread da interface. Os resultados distinguem no mínimo:

- `NotFound`: a raiz estava acessível e o alvo não existe;
- `Unavailable`: não foi possível determinar a existência porque a rede, servidor ou compartilhamento está indisponível;
- `AccessDenied`: o alvo existe ou pode existir, mas a operação não possui acesso suficiente;
- `InvalidPath`: a forma não é aceita para aquele propósito;
- `UnexpectedObjectType`: o alvo abriu, mas não é o arquivo regular ou diretório exigido;
- `UnboundRoot`: uma fase distribuída recebeu uma raiz externa que não pertence ao plano da tentativa;
- `Conflict`: o alvo está aberto, bloqueado ou mudou durante a operação;
- `IoFailure`: outra falha do sistema de arquivos.

Esses resultados são evidência sobre o disco, não estados de domínio. A tradução para Arquivo ausente ou Arquivo indisponível pertence à seção de mídias de [Propriedade de estado e módulos do núcleo](0012-propriedade-de-estado-e-modulos-do-nucleo.md), conforme a decisão registrada no [ADR 0001](../adr/0001-vincular-arquivos-externos.md). O módulo de caminhos não a executa e não classifica um vínculo por conta própria.

O que o módulo garante é que a distinção continue possível: uma falha de rede, servidor ou permissão nunca é reportada como `NotFound`.

## Integrações

### Dados do aplicativo

As raízes são obtidas pelas pastas conhecidas do Windows, nunca pelo diretório corrente do processo. A estrutura concreta permanece definida em [Armazenamento local e Cache](0010-armazenamento-local-e-cache.md).

### Cache

Cache usa a Identidade do Projeto sob a raiz local do aplicativo. A localização de Projeto ou mídia em UNC não move esse namespace. O contexto de resolução de uma geração de Cache é memória operacional, não uma nova categoria persistente.

### Monitor de Arquivos vinculados

O Monitor apenas solicita nova inspeção diante de uma possível mudança. Depois de confirmada, `MediaRuntime` atualiza o estado observado e `CacheEngine` invalida a representação aplicável. Perda do compartilhamento ou falha transitória produz `Unavailable`; não transforma em massa seus arquivos em ausentes. Ao recuperar acesso, a validação normal atualiza os estados.

### Exportação

O módulo deriva e reserva uma pasta de preparação única dentro da própria pasta de Destino, com nome reservado para a operação. Isso coloca preparação e nomes finais no mesmo volume e na mesma árvore do compartilhamento, mas não cria rollback do conjunto.

O `ExportPipeline` possui o ciclo da preparação e a remove após sucesso, falha ou cancelamento normal; resíduos após queda entram na limpeza segura da próxima operação. Dentro dele, `Publisher` possui a promoção e a limpeza de órfãos permitida. O módulo de caminhos não decide rollback, órfãos ou quando uma Publicação é considerada concluída.

Nomes finais são derivados somente por componentes relativos validados. Não podem conter nova raiz, `.`, `..`, namespace de dispositivo, fluxo alternativo (`:`), nome reservado do Windows ou qualquer escape do Destino. A validação textual não autoriza atravessar reparse points: a publicação confirma o alvo real no momento do uso.

A Publicação conserva o envelope limitado do ADR 0006 e só usa originais abertos durante a tentativa. Uma resolução reutilizada nunca substitui a validação e abertura de cada original necessário. Em UNC, o `Publisher` verifica a substituição atômica no uso; o resultado continua dependente do servidor e do sistema de arquivos, e ausência de suporte é propagada, não mascarada.

### Lotes

Origem e Destino são comparados por raízes e identidade física quando disponíveis. Destino igual ou contido na origem é rejeitado mesmo quando um caminho usa unidade mapeada e o outro usa UNC. A hierarquia relativa é calculada por componentes sob as raízes resolvidas.

Quando o validador de lote não consegue provar separação segura entre Origem e Destino, ele falha de forma fechada e apresenta um problema acionável. Essa é uma política do chamador para `Indeterminate`, não uma regra universal embutida no módulo de caminhos.

### Photoshop

O executável e o Arquivo vinculado são resolvidos para a ação corrente. Uma forma de apresentação mais simples pode ser fornecida quando segura, mas o MyAlbuns nunca altera o caminho persistido apenas para satisfazer a integração.

## Cenários de validação

| Cenário | Resultado esperado |
|---|---|
| muitos arquivos sob a mesma raiz UNC | raiz é capturada uma vez no plano; arquivos ainda são abertos individualmente |
| host e Processador participam da mesma Exportação | recebem o mesmo `RootBindingPlan` e usam bindings idênticos |
| worker recebe raiz externa não planejada | não resolve independentemente; devolve `UnboundRoot` e a tentativa termina |
| `Z:\Album` e `\\servidor\Fotos\Album` apontam para o mesmo local | identidade física e bloqueio impedem duas sessões do mesmo Projeto |
| unidade mapeada muda durante Exportação | tentativa continua no binding capturado; nunca segue o novo mapeamento deliberado |
| DFS ou SMB muda o servidor físico | o contrato conserva o binding operacional, sem prometer identidade do backend |
| servidor fica offline | itens ficam indisponíveis, não ausentes |
| servidor retorna | revalidação recupera o acesso sem Religação quando o caminho não mudou |
| caminho verbatim de disco/UNC longo | operação preserva a forma necessária e não o trunca |
| caminho relativo externo | validação rejeita antes de iniciar a operação |
| namespace de dispositivo | validação rejeita |
| curinga, fluxo alternativo ou componente reservado | validação rejeita antes da abertura |
| arquivo indicado onde se esperava diretório | abertura retorna `UnexpectedObjectType` |
| alvo novo sob pai validado | filho é derivado com segurança e o handle pós-criação confirma tipo e contenção |
| Projeto e mídia em UNC | Cache continua no namespace local por Identidade |
| destino UNC | preparação ocorre no mesmo compartilhamento antes da Publicação |
| duas tentativas consecutivas ou retomada após queda | cada uma captura e descarta seu próprio plano |
| identidade física não pode ser obtida | módulo retorna `Indeterminate`; o chamador escolhe sua política |
| caminho relativo tenta escapar do Destino | derivação é rejeitada antes de criar a saída |

## Decisões adiadas

- crates concretas e suas versões;
- representação serializada de casos extremos de nomes Windows;
- codificação concreta do `RootBindingPlan` na IPC;
- suporte futuro a outros sistemas operacionais;
- política de fallback do Monitor quando o servidor não oferecer notificações confiáveis;
- métricas quantitativas de latência e tempo de indisponibilidade.
