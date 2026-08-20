---
status: current
document: technical-research
ticket: 45-integracao-final-midias-cache
date: 2026-08-18
updated: 2026-08-19
---

# Integração final de Mídias externas e Cache

## Pergunta

Como integrar Mídias externas e Cache sem transferir ao Cache a autoridade de
Identidade, Salvamento ou Exportação, preservando a edição quando o Original ou
o Processador se tornam temporariamente indisponíveis?

## Fronteiras adotadas

O `ProjectCore` continua sendo o produtor autoritativo da Identidade, inclusive
nos resultados de movimentação, alias físico, Cópia externa gravável e fonte
somente leitura da issue 10. O Host só reserva um namespace de Cache depois de
receber uma `EditableProject` autorizada. Uma nova Identidade, portanto, chega
ao consumidor Cache com namespace independente e vazio; movimentar o mesmo
Projeto conserva o namespace, enquanto focalizar um alias não cria outro Host
nem outra reserva.

No escopo da issue 11, o `MediaResolver` produz inspeções autoritativas e
propostas imutáveis. Ele separa `Absent`, obtido somente quando uma origem
acessível confirma a ausência, de `Unavailable`, que não permite inferir
ausência nem mudança. O `MediaRuntime` apenas registra as observações aceitas.
Duas amostras estáveis continuam necessárias para uma mudança de conteúdo. Na
Religação pública, a WebView envia somente o `mediaId` e o Host oferece o seletor
nativo apenas para uma ocorrência cuja ausência foi confirmada. O pathname
escolhido não atravessa IPC: `MediaResolver` reabre o candidato pela fronteira
central de caminhos, valida JPEG, PNG ou TIFF e produz uma proposta imutável;
`ProjectSession` possui e aplica `RelinkMedia` com Histórico e mudanças
pendentes. Antes de aplicar, o Host confirma novamente que a mesma ocorrência e
o mesmo vínculo continuam atuais. O monitor reinspeciona a nova referência, e
o Cache invalida somente esse `mediaId`, mesmo quando duas ocorrências apontavam
para o mesmo arquivo físico.

Uma ocorrência confirmada como `Unavailable` oferece `Tentar novamente`, nunca
Religação. A WebView envia somente o `mediaId`; o Host relê o binding atual da
ocorrência e o `MediaResolver` cria outro `OperationPathContext` para uma nova
inspeção autoritativa. `MediaRuntime` substitui somente essa observação e o
`CacheEngine` reage à nova geração antes de outra demanda de prévia. A adoção é
uma transição única: se essa reação falhar, `MediaRuntime` conserva
`Unavailable` e a próxima tentativa continua autorizada. Se a raiz
continuar inacessível, o estado e a ação permanecem; se ela voltar e o arquivo
não existir, o resultado passa a `Absent` e somente então oferece Religação; se
o Original reaparecer, a demanda normal reconstrói ou revalida a representação.
O fluxo não chama `ProjectSession`, não cria Histórico e não altera caminho,
`MediaRef`, revisão, dirty, Undo/Redo ou conteúdo salvo.

Uma primeira amostra ainda não consolidada não é uma observação sobre a origem:
a demanda omite a prévia dessa ocorrência e a WebView não oferece ação de
recuperação. O comando de repetição falha fechado quando o `MediaRuntime` não
tem snapshot, quando a ocorrência não aparece no snapshot ou quando a
observação atual é `Candidate` ou `Absent`; somente uma observação exatamente
`Unavailable` autoriza `Tentar novamente`.

Falha do Processador, de validação do protocolo, do armazenamento do Cache ou
da captura técnica usada para preparar um job não confirma indisponibilidade do
Original. Nesses terminais a prévia usa o estado distinto
`CacheUnavailable`, pode manter a última representação apenas como contexto e
não oferece `Tentar novamente` nem Religação. Somente a inspeção autoritativa do
`MediaResolver` produz `Unavailable` e sua ação explícita. A leitura e a
validação finais do artefato pelo Registry obedecem à mesma regra: uma falha
retorna `CacheUnavailable` na resposta da demanda, em vez de rejeitar o comando
e deixar a WebView conservar um estado antigo da origem.

O `CacheEngine` consome essas duas autoridades. A última representação válida
pode permanecer visível como contexto quando o Original está ausente ou
indisponível, mas não muda o estado do Original e não autoriza Exportação. Uma
observação autoritativa posterior de reaparecimento ou mudança cria nova época;
pedido, fingerprint, variante e época são revalidados antes da publicação.
Essa invalidação revoga demanda e publicação residente, mas conserva a entrada
e o arquivo da geração publicada anterior. O planejamento não pode reutilizá-la
porque revalida o fingerprint atual; somente a publicação atômica de uma
sucessora troca o índice e então coleta a geração superada.
Falha de protocolo, correlação, transporte ou validação descarta o candidato
ainda não referenciado pelo índice e preserva a última geração publicada. Isso
também cobre a perda da resposta depois que o Processador promoveu o arquivo
candidato.

## Ciclo de vida descartável

O índice do Cache é derivado e inteiramente descartável. Schema incompatível,
JSON corrompido, entrada duplicada ou artefato inválido provocam reconstrução a
partir de trabalho novo, nunca aceitação parcial. Temporários pertencem à
geração e ao processo que os criou; o sweep não remove temporários estrangeiros
nem uma geração publicada por outro trabalho.

No Windows, criação, abertura, descarte, substituição e coleta usam um único
componente relativo ao handle físico do diretório já validado. O temporário
permanece aberto desde a criação até o `sync` e a promoção; abandono marca esse
mesmo handle para exclusão. Limpeza recursiva abre e remove cada arquivo pelo
handle do pai, e remoção de diretório rejeita um pathname que passou a nomear
uma junction. Assim, a troca concorrente de `Media` ou do namespace não pode
apagar, truncar ou promover um homônimo fora do Cache autorizado.

Há uma única exceção deliberada para a recuperação após queda total do Host:
o novo Host primeiro adquire com exclusividade a reserva nomeada do namespace.
Antes de cada dispatch de Cache, o Host anterior publica atomicamente uma claim
com PID e instante de criação do Processador já contido. Essa autoridade não é
inferida do PID retornado pelo launcher: o Host injeta um desafio único no
sidecar recém-criado, recebe pelo pipe causal de `stdout` um handshake limitado
a uma linha com o `ProcessInstanceId` e compara desafio, PID e instante de
criação antes de adquirir o handle e associá-lo ao Job. Se o PID já foi
reciclado, a associação falha sem conter ou encerrar o novo ocupante. Esse
envelope condicional pertence ao ciclo de vida Host–Processador; ele não altera
os comandos ou eventos do protocolo de imagens v17. O fechamento do Job Object
solicita a terminação, mas ela é assíncrona; por isso o novo Host reabre essa
instância exata e aguarda seu estado sinalizado antes de inspecionar ou remover
qualquer conteúdo. PID isolado e mutex abandonado não são tratados como
prova de término. Sob essa exclusão confirmada, a recuperação remove os nomes
temporários bem-formados abandonados. Se o índice atual for válido, preserva
somente suas gerações referenciadas e coleta finais órfãos; se estiver ausente,
incompatível ou corrompido, coleta todas as gerações finais e reconstrói o índice
em trabalho novo. Claim incompatível, falha de consulta, timeout ou erro de
inspeção fecham a abertura do namespace, em vez de aceitar estado parcial.
Toda leitura, publicação e remoção da claim passa por
`CacheWriterClaimStorage`, da biblioteca central `myalbuns-paths`. Essa guarda
mantém aberta a cadeia física validada de diretórios durante a espera da
instância exata e abre cada arquivo por um nome relativo a essa autoridade. A preparação da
espera adquire essa guarda e o handle da instância exata como uma única
fronteira produtiva antes de qualquer readiness; a prova concorrente confirma
que o Processador continua vivo e o waiter bloqueado antes de substituir o
pathname. Publicação create-only
usa `NtSetInformationFile`, a entrada indicada pela documentação Win32 para
user mode, com o diretório físico como raiz relativa; remoção condicional usa
`SetFileInformationByHandle` no mesmo handle aberto com `DELETE`. Se o pathname do
namespace for trocado por junction durante a espera, a operação rejeita o novo
alvo e preserva suas claims e temporários externos.

Uma falha abrupta do Processador permite exatamente um restart para o trabalho
ainda atual. Uma segunda falha suspende novos trabalhos de Cache e emite o
evento tipado `myalbuns://cache-processor-warning`. A Tela de Projeto mostra o
aviso como status não modal; edição, Salvamento e a representação já disponível
continuam funcionando. A primeira demanda de prévia só é enviada depois que a
Promise de registro desse listener e a Promise do listener de mudança estável
de mídia resolvem para o mesmo Projeto e port. Assim nem a própria falha que
causa a suspensão nem a confirmação autoritativa do Monitor podem preceder seus
observadores na WebView. Falha ao registrar suspende somente novos pedidos de
Cache e não bloqueia edição ou Salvamento. Cada sidecar é contido, antes do
dispatch, em um Job
Object privado com `KILL_ON_JOB_CLOSE`; ele não sobrevive à queda do Host que
possui a reserva do namespace. O estado externo é deliberadamente estreito:
somente `suspended` atravessa IPC, sem expor o supervisor ou criar um
coordenador geral.

## Serviço de Cache

`CacheService` possui três operações públicas: consultar uso, liberar Cache de
Projetos fechados e limpar tudo ou agendar a limpeza. O serviço mede bytes sob
o `AppPaths` local e trata reparse points e tipos inesperados como erro, em vez
de segui-los. Primeiro ele enumera apenas os namespaces; para cada namespace
fechado, adquire a reserva, aguarda a instância exata registrada na claim e só
então inspeciona os bytes. Um namespace que permanece ativo é medido por um
snapshot guardado, mas nunca entra no volume liberável. Se o proprietário fecha
durante esse snapshot, o serviço repete a inspeção depois da quiescência. Uma
medição ativa usa os metadados já capturados pela enumeração do diretório no
Windows, sem reabrir arquivos que o Processador mantém sem compartilhamento;
renomes e remoções concorrentes podem tornar a contagem momentaneamente
aproximada, mas não indisponível. Reparse points e tipos inesperados continuam
falhando fechados. Não há operação bulk pública que inspecione todos os
namespaces sem antes classificá-los e coordená-los. Uma
reserva por namespace usa mutex nomeado do Windows e vive por toda a Sessão. Um
único construtor de mutex aplica o case mapping não linguístico do próprio
Windows à raiz local e normaliza o namespace ASCII antes do hash. `CacheService`
e `OperationGate` compartilham essa identidade: casing diferente da raiz ou o
casing armazenado que `read_dir` devolve não pode criar uma segunda reserva para
o mesmo diretório. Outro processo, portanto, não pode classificar aquele
namespace como liberável.

A liberação parcial adquire apenas reservas disponíveis e remove somente esses
Projetos. A limpeza total exige simultaneamente o gate de operação sem
Processador/Projeto ativo, a lease de manutenção e todas as reservas de
namespace. Se isso não for possível, um marcador `create-only` agenda a ação
para o próximo início seguro. Chamadores concorrentes convergem para um único
marcador idempotente.
O marcador fixo também pertence a uma guarda estreita de `myalbuns-paths`: a
cadeia até `State` permanece aberta, publicação é create-only relativa ao handle
e remoção exige os mesmos bytes pelo mesmo arquivo aberto. Uma junction em
`State` falha fechada e nunca transforma um marcador externo em autoridade de
limpeza.

No início do Global, a WebView é criada oculta dentro do `setup`, mas a limpeza
agendada é executada no executor dedicado a operações bloqueantes. Um gate de
prontidão retém os comandos automáticos e as ações de abertura/criação até o
resultado seguro; a janela só segue sua inicialização depois desse resultado.
Assim, aguardar uma claim de Processador e percorrer ou remover Cache não bloqueia
o loop do Tauri. Falha de consulta ou limpeza mantém a janela oculta, devolve
falha tipada aos comandos pendentes e encerra o processo em vez de liberar ações
sobre estado não inspecionado.

Esse recorte não cria store, registry ou coordenador universal. Ele também não
reimplementa promoção, movimento, alias ou `Salvar cópia como…`, pertencentes à
issue 10, nem `Salvar como…` e a nova Identidade da issue 18. A issue 16 consome
somente as três operações estreitas de serviço; as referências e transições de
Mídia permanecem no contrato da issue 11. Os três comandos são permitidos
explicitamente apenas pela capability da janela Global; a capability do
Projeto permanece disjunta e não herda administração de Cache.

## Matriz do design 0010

Cada linha abaixo corresponde, sem omissões, à matriz normativa de cenários do
design 0010. A última coluna nomeia uma prova comportamental executada pelo gate;
o próprio runner rejeita linha ausente, extra, duplicada ou associada a outra
prova.

| Cenário normativo | Produtor autoritativo | Efeito no consumidor Cache | Prova comportamental |
| --- | --- | --- | --- |
| Projeto renomeado ou movido | `ProjectCore` da issue 10 conserva a Identidade | conserva o namespace e os dados derivados | `cache_consumes_authoritative_identity_transitions_without_owning_them` |
| `Salvar como` | `ProjectCore` da futura issue 18 produz nova Identidade | recebe a nova autoridade e reserva namespace independente e vazio | `a_new_authorized_identity_reserves_an_independent_empty_namespace` |
| Cópia externa gravável | promoção autoritativa da issue 10 produz nova Identidade | recebe namespace independente e vazio | `cache_consumes_authoritative_identity_transitions_without_owning_them` |
| Cópia externa somente leitura | resultado opaco da issue 10 nega autoridade editável | não monta Sessão nem namespace duplicado | `cache_consumes_authoritative_identity_transitions_without_owning_them` |
| original alterado | duas inspeções autoritativas estáveis do `MediaResolver` | invalida reuse/publicação residente somente daquela ocorrência e conserva a geração indexada até a sucessora verificada | `monitor_consolidates_rapid_observations_and_invalidates_only_stable_content_changes` |
| original ausente | inspeção autoritativa `Absent` do `MediaResolver` | mantém somente contexto visual; não valida o Original nem autoriza Exportação | `absent_or_unavailable_media_preserves_the_last_known_preview_with_its_typed_state` |
| origem de rede indisponível | inspeção inconclusiva `Unavailable` do `MediaResolver`; `Tentar novamente` abre contexto novo | preserva vínculo e última representação sem confirmar ausência nem oferecer Religação | `absent_or_unavailable_media_preserves_the_last_known_preview_with_its_typed_state` |
| índice corrompido | validação integral do `CacheEngine` | descarta o índice completo e reconstrói em trabalho novo | `corrupted_or_incompatible_index_is_discarded_and_rebuilt` |
| job obsoleto termina | demanda autoritativa revalidada pelo `CacheEngine` | descarta a geração candidata sem publicá-la | `obsolete_job_that_finishes_does_not_publish_and_discards_its_candidate_generation` |
| queda durante geração | Host e supervisor da issue 44 publicam a claim da instância contida | aguarda o Processador terminar, coleta temporário e preserva a geração publicada | `reopening_after_host_death_recovers_the_contained_processors_temporary` |
| Projeto abre durante `Liberar espaço` | Host e serviço disputam a reserva atômica do namespace | quem reserva primeiro exclui o outro; nunca há remoção concorrente | `project_open_during_free_space_is_serialized_by_namespace_reservation` |
| limpeza total com Projeto ativo | `OperationGate` e reservas registram atividade real | agenda limpeza para o próximo início seguro | `schedules_total_cleanup_while_a_project_is_active_and_runs_it_at_safe_startup` |
| Exportação | `ExportPipeline` usa snapshot validado e Originais | Cache pausa e nunca se torna fonte final nem fallback | `export_plan_rejects_missing_originals_at_the_typed_plan_stage` |
| Projeto ou mídia em UNC | autoridade de Identidade e `RootBindingPlan` preservam os paths | mantém Cache sob a raiz local do aplicativo | `real-mapped-unc` |

O sufixo temporário de desenvolvimento `MyAlbuns2` permanece conforme o design
0010. Migrá-lo para `MyAlbuns` é uma decisão de distribuição separada; esta
integração mantém a árvore normativa relativa `Cache/<namespace>/Media` sob o
local data root escolhido por `AppPaths`.

## Evidência e repetição

O runner `scripts/Test-Issue45MediaCacheGate.ps1` exige `HEAD` limpo baseado no
fixed point `f6518d63b2c75656a58b6769e87abc318a913e23`. Ele executa contratos
Rust→TypeScript, frontend, Rust, `fmt`, `clippy -D warnings`, recuperação real
de Processador/Cache/Canvas, caminhos locais/UNC/mapeados/longos e build release
com bundle NSIS. Contagens vazias fecham o gate.

O script da issue conserva apenas a ordem dos checks, os comandos de build e a
composição da matriz. Mecanismos com razões de mudança próprias ficam em módulos
concretos: `Gate-ProcessScope.ps1` contém Job, processos e listeners;
`Issue45-MediaCacheGateScratch.ps1` contém preflight, limpeza causal e a
sentinela independente; `Issue45-MediaCacheGateProof.ps1` contém parsers,
conjuntos exatos e validação da matriz; `Gate-EvidenceReport.ps1` contém hashes,
locks, snapshots terminais e publicação restaurável. Eles reutilizam os helpers
menores de Job, scratch e proveniência já compartilhados, sem introduzir um
registro ou framework universal de gates.

Os relatórios transitivos são produzidos novamente dentro do scratch da rodada;
nenhum artefato anterior é aceito. Antes do primeiro contrato Tauri, o runner
fixa um `CARGO_TARGET_DIR` absoluto e exclusivo sob o `runRoot`; contratos,
testes Rust, quality, recovery, Tauri debug e paths Windows herdam esse mesmo
target isolado. O sidecar debug exigido pelo build script é preparado ali, e o
release usa outro target exclusivo sob o mesmo scratch. Consumidores de
executáveis resolvem `CARGO_TARGET_DIR` pela mesma regra do Cargo, inclusive
quando relativo. O gate Windows aninhado recebe ainda um scratch exclusivo sob
o `runRoot`; seu scratch standalone e seu target padrão nunca são limpos pelo
runner da issue 45 depois que o mutex do processo filho é liberado. Uma
sentinela byte a byte no scratch standalone precisa sobreviver à limpeza da
rodada e somente o arquivo da própria prova pode ser removido depois. A jornada
WebView2 de recuperação aguarda ainda todos os processos vinculados ao
seu scratch antes de remover o perfil e repete locks transitórios sem abandonar
o diretório. Cada
critério é derivado de provas nomeadas e não vazias nos
resultados comportamentais da própria rodada. O runner inicia ainda um processo
controlado com listener TCP
dentro de um Job Object privado, observa a árvore causal e encerra o Job pelo
mesmo caminho usado no `finally`, exigindo contagens finais zero. Um
processo-sentinela concorrente, criado depois do snapshot mas fora desse Job,
precisa permanecer vivo; portanto proximidade temporal, pathname e PID parental
nunca autorizam encerramento. Cada identidade observada combina PID e instante
de criação e nenhuma identidade do snapshot pré-gate pode entrar num Job
próprio. `dist`, sidecar preparado e todo o `target` compartilhado da worktree
precisam estar ausentes no preflight: o runner falha antes de escrever, em vez
de sobrescrever um output ignorado preexistente. Ele calcula hashes dos
artefatos, mede listeners reais, exige locks exclusivos, drena somente Jobs
próprios, remove todos os outputs que criou e só então monta as provas derivadas
das fontes normativas. Uma prova terminal exige que o target isolado e o
`runRoot` tenham sido removidos, que os contêineres de scratch criados pela
rodada também tenham sido descartados e que o `target` compartilhado continue
ausente; o runner não remove roots globais que um gate Windows independente
possa ter recriado. Qualquer escrita fora dos paths causais falha o gate. Depois
da última leitura e da construção do relatório, captura novamente a árvore Git;
após o `finally` e, imediatamente antes e depois de publicar o JSON, repete essa
verificação. Se a fonte mudar
durante a publicação, o artefato anterior é restaurado. Uma fixture Git
negativa altera um input
versionado justamente depois da leitura da prova e precisa ser rejeitada. O
caminho de sucesso e o `finally` usam a mesma rotina medida de encerramento e
verificação; qualquer falha também passa por ela antes da limpeza do scratch.

Resultados Rust são aceitos apenas por uma linha terminal exata `... ok`, e o
frontend por uma asserção JSON exata com estado `passed`; `ignored`, pendente,
duplicata ou simples substring fecham o gate. Fixtures negativas exercitam o
parser. Os nove checks transitivos de recuperação, protocolo, sidecar,
temporários, cancelamento, pausa causal e Canvas/WebView2 também formam um
conjunto nominal exato: remover qualquer um, duplicar um nome ou marcar um deles
como falho rejeita a evidência e o critério do tracer 44. `passed` precisa ser o
Booleano `true` exato — string, inteiro, `null` e propriedade ausente são
rejeitados — e `sourceInputsDirty` precisa ser o Booleano `false` exato. O mesmo
contrato cobre os onze checks do gate Windows. As fixtures somam 16 asserções
para recuperação e 18 para paths Windows. A matriz acima também
é validada como conjunto exato e cada remoção unitária é usada como fixture
negativa antes de aceitar as provas da rodada. Antes da publicação, o conjunto
de checks de topo também precisa conter exatamente os nomes esperados, todos
com `passed` Booleano `true`.

O artefato canônico é
[`artifacts/0036-issue-45-media-cache-integration.json`](artifacts/0036-issue-45-media-cache-integration.json).
`sourceInputsDirty=false` significa que o mesmo commit limpo foi observado
antes da rodada, depois de todas as leituras que alimentam o relatório e nos
dois lados da publicação terminal, excluindo apenas esse JSON de saída.

Em Windows, a repetição é:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass `
  -File scripts\Test-Issue45MediaCacheGate.ps1 `
  -OutputPath docs\research\artifacts\0036-issue-45-media-cache-integration.json
```

## Limites

- o gate UNC usa compartilhamento administrativo loopback; não simula WAN,
  DFS, perda de credenciais nem latência de rede;
- a suspensão cobre falhas repetidas do Processador na Sessão atual; retomada
  automática sem restart do aplicativo não faz parte deste contrato;
- a última representação é apenas auxílio visual e pode estar desatualizada;
  qualquer operação que precise do Original deve revalidá-lo por sua própria
  fronteira autoritativa;
- a instalação do bundle não é executada no host de desenvolvimento; o gate
  prova build release, payload do Processador e produção do instalador NSIS.
