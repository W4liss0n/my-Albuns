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

O `MediaRuntime` é o produtor das observações de Mídia da issue 18. Ele separa
`Missing`, obtido por inspeção autoritativa de ausência, de `Unavailable`, que
não permite inferir ausência nem mudança. Duas amostras estáveis continuam
necessárias para uma mudança de conteúdo. A Religação inclui o pathname lógico
da ocorrência, de modo que duas ocorrências do mesmo arquivo físico não sejam
invalidadas em conjunto.

O `CacheEngine` consome essas duas autoridades. A última representação válida
pode permanecer visível como contexto quando o Original está ausente ou
indisponível, mas não muda o estado do Original e não autoriza Exportação. Uma
observação autoritativa posterior de reaparecimento ou mudança cria nova época;
pedido, fingerprint, variante e época são revalidados antes da publicação.
Falha de protocolo ou validação descarta apenas o candidato e preserva a última
geração publicada.

## Ciclo de vida descartável

O índice do Cache é derivado e inteiramente descartável. Schema incompatível,
JSON corrompido, entrada duplicada ou artefato inválido provocam reconstrução a
partir de trabalho novo, nunca aceitação parcial. Temporários pertencem à
geração e ao processo que os criou; o sweep não remove temporários estrangeiros
nem uma geração publicada por outro trabalho.

Uma falha abrupta do Processador permite exatamente um restart para o trabalho
ainda atual. Uma segunda falha suspende novos trabalhos de Cache e emite o
evento tipado `myalbuns://cache-processor-warning`. A Tela de Projeto mostra o
aviso como status não modal; edição, Salvamento e a representação já disponível
continuam funcionando. O estado externo é deliberadamente estreito: somente
`suspended` atravessa IPC, sem expor o supervisor ou criar um coordenador geral.

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
reimplementa promoção, movimento, alias, `Salvar como…` ou `Salvar cópia
como…`; essas decisões permanecem nas fronteiras das issues 10, 16, 11 e 18.

## Matriz do design 0010

| Transição | Produtor autoritativo | Efeito no consumidor Cache |
| --- | --- | --- |
| Projeto novo | `ProjectIdentityAuthority` / `ProjectCore` | reserva namespace novo, independente e vazio |
| Mesmo arquivo por alias | comparação física da issue 10 | `FocusExisting`; nenhum namespace novo |
| Projeto movimentado | `ProjectCore` conserva o UUID | conserva namespace e dados derivados |
| Cópia externa gravável | promoção técnica da issue 10 | recebe namespace novo e vazio |
| Cópia externa somente leitura | resultado opaco da issue 10 | nenhuma Sessão e nenhum namespace |
| Original ausente | inspeção do `MediaRuntime` | mantém só contexto visual; não invalida |
| Original indisponível | inspeção inconclusiva do `MediaRuntime` | mantém contexto e aguarda nova inspeção |
| Religação de uma ocorrência | ação/observação da issue 18 | invalida somente a ocorrência religada |
| Mudança estável | duas observações do `MediaRuntime` | expira a época e deriva nova geração |
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
nenhum artefato anterior é aceito. O release usa um `CARGO_TARGET_DIR` exclusivo
sob esse scratch. O runner calcula hashes antes de remover os outputs, exige
zero processo próprio, zero listener próprio e locks exclusivos disponíveis,
remove seus diretórios e só então captura novamente a árvore Git.

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
