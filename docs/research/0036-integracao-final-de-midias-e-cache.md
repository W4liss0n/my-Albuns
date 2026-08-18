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
o novo Host primeiro adquire com exclusividade a reserva nomeada do namespace e,
antes de iniciar qualquer Processador, remove todos os nomes temporários
bem-formados abandonados. A reserva prova que o Host anterior terminou; o Job
Object prova que seu Processador não sobreviveu. Sob essa exclusão, não é
necessário tratar PID reciclável como autoridade. Se o índice atual for válido,
a recuperação preserva somente suas gerações referenciadas e coleta finais
órfãos; se estiver ausente, incompatível ou corrompido, coleta todas as gerações
finais e reconstrói o índice em trabalho novo. Erro de inspeção fecha a abertura
do namespace, em vez de aceitar estado parcial.

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
de segui-los. Uma reserva por namespace usa mutex nomeado do Windows e vive por
toda a Sessão; outro processo, portanto, não pode classificar aquele namespace
como liberável.

A liberação parcial adquire apenas reservas disponíveis e remove somente esses
Projetos. A limpeza total exige simultaneamente o gate de operação sem
Processador/Projeto ativo, a lease de manutenção e todas as reservas de
namespace. Se isso não for possível, um marcador `create-only` agenda a ação
para o próximo início seguro. Chamadores concorrentes convergem para um único
marcador idempotente.

Esse recorte não cria store, registry ou coordenador universal. Ele também não
reimplementa promoção, movimento, alias ou `Salvar cópia como…`, pertencentes à
issue 10, nem `Salvar como…` e a nova Identidade da issue 18. A issue 16 consome
somente as três operações estreitas de serviço; as referências e transições de
Mídia permanecem no contrato da issue 11.

## Matriz do design 0010

| Transição | Produtor autoritativo | Efeito no consumidor Cache |
| --- | --- | --- |
| Projeto novo | `ProjectIdentityAuthority` / `ProjectCore` | reserva namespace novo, independente e vazio |
| `Salvar como` | `ProjectCore`, na futura issue 18 | recebe a nova autoridade e reserva namespace novo e vazio |
| Mesmo arquivo por alias | comparação física da issue 10 | `FocusExisting`; nenhum namespace novo |
| Projeto movimentado | `ProjectCore` conserva o UUID | conserva namespace e dados derivados |
| Cópia externa gravável | promoção técnica da issue 10 | recebe namespace novo e vazio |
| Cópia externa somente leitura | resultado opaco da issue 10 | nenhuma Sessão e nenhum namespace |
| Original ausente | inspeção autoritativa do `MediaResolver`, registrada pelo `MediaRuntime` | mantém só contexto visual; não valida o Original nem autoriza Exportação |
| Original indisponível | inspeção inconclusiva do `MediaResolver`, registrada pelo `MediaRuntime` | mantém contexto e aguarda nova inspeção |
| Religação de uma ocorrência | proposta validada do `MediaResolver`, seguida de `ProjectSession::RelinkMedia` e reinspeção | invalida somente o `mediaId` religado |
| Mudança estável | duas inspeções autoritativas do `MediaResolver`, registradas pelo `MediaRuntime` | expira a época e deriva nova geração |
| Cache corrompido/incompatível | validação do consumidor | descarta índice e reconstrói |
| Falha repetida do Processador | supervisor preservado da issue 44 | suspende Cache sem bloquear edição/Salvamento |
| Projeto fechado | término da reserva do Host | namespace passa a ser potencialmente liberável |

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
própria rodada. O runner inicia ainda um processo controlado com listener TCP,
observa ambos, encerra a árvore pelo mesmo caminho usado no `finally` e exige
contagens finais zero. Ele rastreia as identidades PID+instante de criação de
todos os descendentes dos comandos, calcula hashes antes de remover os outputs,
mede listeners reais, exige locks exclusivos disponíveis, remove seus
diretórios e só então captura novamente a árvore Git. Qualquer falha também
passa pelo encerramento e pela verificação da árvore antes da limpeza do
scratch.

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
