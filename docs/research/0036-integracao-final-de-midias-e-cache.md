---
status: current
document: technical-research
ticket: 45-integracao-final-midias-cache
date: 2026-08-18
updated: 2026-08-18
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

O `CacheEngine` consome essas duas autoridades. A última representação válida
pode permanecer visível como contexto quando o Original está ausente ou
indisponível, mas não muda o estado do Original e não autoriza Exportação. Uma
observação autoritativa posterior de reaparecimento ou mudança cria nova época;
pedido, fingerprint, variante e época são revalidados antes da publicação.
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

Uma falha abrupta do Processador permite exatamente um restart para o trabalho
ainda atual. Uma segunda falha suspende novos trabalhos de Cache e emite o
evento tipado `myalbuns://cache-processor-warning`. A Tela de Projeto mostra o
aviso como status não modal; edição, Salvamento e a representação já disponível
continuam funcionando. Cada sidecar é contido, antes do dispatch, em um Job
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
reserva por namespace usa mutex nomeado do Windows e vive por toda a Sessão;
outro processo, portanto, não pode classificar aquele namespace como liberável.

A liberação parcial adquire apenas reservas disponíveis e remove somente esses
Projetos. A limpeza total exige simultaneamente o gate de operação sem
Processador/Projeto ativo, a lease de manutenção e todas as reservas de
namespace. Se isso não for possível, um marcador `create-only` agenda a ação
para o próximo início seguro. Chamadores concorrentes convergem para um único
marcador idempotente.

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
Mídia permanecem no contrato da issue 11.

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
| original alterado | duas inspeções autoritativas estáveis do `MediaResolver` | invalida somente a representação daquela ocorrência | `monitor_consolidates_rapid_observations_and_invalidates_only_stable_content_changes` |
| original ausente | inspeção autoritativa `Absent` do `MediaResolver` | mantém somente contexto visual; não valida o Original nem autoriza Exportação | `absent_or_unavailable_media_preserves_the_last_known_preview_with_its_typed_state` |
| origem de rede indisponível | inspeção inconclusiva `Unavailable` do `MediaResolver` | preserva vínculo e última representação sem confirmar ausência | `absent_or_unavailable_media_preserves_the_last_known_preview_with_its_typed_state` |
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

Os relatórios transitivos são produzidos novamente dentro do scratch da rodada;
nenhum artefato anterior é aceito. Antes do primeiro contrato Tauri, o runner
compila e prepara no próprio scratch o sidecar debug exigido pelo build script;
o release usa outro `CARGO_TARGET_DIR` exclusivo sob esse scratch. Cada critério
é derivado de provas nomeadas e não vazias nos resultados comportamentais da
própria rodada. O runner inicia ainda um processo controlado com listener TCP
dentro de um Job Object privado, observa a árvore causal e encerra o Job pelo
mesmo caminho usado no `finally`, exigindo contagens finais zero. Um
processo-sentinela concorrente, criado depois do snapshot mas fora desse Job,
precisa permanecer vivo; portanto proximidade temporal, pathname e PID parental
nunca autorizam encerramento. Cada identidade observada combina PID e instante
de criação e nenhuma identidade do snapshot pré-gate pode entrar num Job
próprio. `dist`, sidecar preparado e o target fixo de caminhos precisam estar
ausentes no preflight: o runner falha antes de escrever, em vez de sobrescrever
um output ignorado preexistente. Ele calcula hashes dos artefatos, mede
listeners reais, exige locks exclusivos, drena somente Jobs próprios, remove
todos os outputs que criou e só então captura novamente a árvore Git. O caminho
de sucesso e o `finally` usam a mesma rotina medida de encerramento e
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
negativa antes de aceitar as provas da rodada.

O artefato canônico é
[`artifacts/0036-issue-45-media-cache-integration.json`](artifacts/0036-issue-45-media-cache-integration.json).
`sourceInputsDirty=false` significa que o mesmo commit limpo foi observado
antes e depois de toda a rodada, excluindo apenas esse JSON de saída.

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
