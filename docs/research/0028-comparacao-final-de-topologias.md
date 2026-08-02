---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-08-02
updated: 2026-08-02
---

# Comparação final das topologias de processo

## Resultado

As duas ordens balanceadas `AB` e `BA` terminaram integralmente no commit
`e5e3c8d4bc2009092a043baf359fa2dc3907fce6`. Os 11 checks do protocolo
congelado passaram. Portanto, o critério do ticket 01 que exige hardware,
massas de teste, medições brutas, falhas e custo das duas alternativas está
encerrado:

- `ticketCriterionSatisfied=true`;
- `criterionClosed=true`.

Esta coleta não escolhe a topologia. Conforme o protocolo:

- `performanceRankingAllowed=false`;
- `topologyRecommendationDeferred=true`.

As classificações deste relatório pertencem a cada métrica isoladamente. Elas
não formam uma pontuação global e não substituem a recomendação arquitetural
nem a atualização do ADR 0005, reservadas ao último critério do spike.

## Rastreabilidade

| Evidência | Schema | SHA-256 |
| --- | ---: | --- |
| [Protocolo congelado](0027-protocolo-da-comparacao-final-de-topologias.md) | — | `cec596df2140903c7ddac3d01f1d6d50e715cd034e43d8dc1d1f47a2a465f90b` |
| [Rodada AB — JSON bruto](artifacts/0019-topology-final-ab.json) | 13 | `af1bd0249decce907ec1d66edf5109810611019b995968a4cb45d843c64dadba` |
| [Rodada AB — referência legível](artifacts/0019-topology-final-ab.md) | — | `3d2694bf2927e0131f02fdf653d3d7fa5f4c1a3f3c7b028667aad6630e99d3a1` |
| [Rodada BA — JSON bruto](artifacts/0020-topology-final-ba.json) | 13 | `3184b2b110b1025703f379388b35474baefbffb0a8e58681912d94c09b34871c` |
| [Rodada BA — referência legível](artifacts/0020-topology-final-ba.md) | — | `f1b447df660d735bc247e6cda5679098a1f6833947f3f04491f9792d4835aad7` |
| [Comparação consolidada canônica](artifacts/0021-topology-final-comparison.json) | 3 | `d79c632534c36282c36b3ef95c21190a7c3f447d4571a41bdad502989ef8e8f1` |

A evidência correlacionada de recuperação do Processador foi gerada em
`target/topology-final-comparison/imaging-recovery.json`, no mesmo commit e
com schema 1. Seu SHA-256 é
`3694123e77d2187b59ec36d8fcf9af8b5e89153dd1fc295c51107fa901fd7997`.
Ela é um produto reproduzível da coleta e não é uma nova fonte normativa.

## Build, ambiente e corpus

### Build medida

- commit: `e5e3c8d4bc2009092a043baf359fa2dc3907fce6`;
- perfil: `release`;
- executável desktop:
  `0f5cd2b6608200c18a4290787eef7921a732b702cab5a10ddb4ff28ee0e300a5`;
- Processador de Imagens:
  `3fa637332bc57ad6ebeeb1f0e7e9d3f4ec36ef0efd873e9700c333cffe7c61d0`;
- 226 inputs de build, com digest
  `fb47bba04e56af7a21e62636ffd862559308c1127d605e8832d9b3f021e84d3f`;
- `buildInputsDirty=false`, `sourceInputsDirty=false` e
  `currentBuildInputsMatchManifest=true` nas duas ordens.

O manifesto registra `workingTreeDirty=true` porque havia arquivos do usuário
fora das entradas da build. Isso não misturou código ou binários entre as
rodadas: as duas ordens usaram o mesmo commit, o mesmo manifesto e os mesmos
executáveis `release`.

### Máquina da coleta

- Microsoft Windows 11 Pro, versão `10.0.26200`, build `26200`;
- Intel Core i5-13450HX;
- 25.439.191.040 bytes de memória física;
- NVIDIA GeForce RTX 3050 6GB Laptop GPU, driver `32.0.15.8195`;
- Intel UHD Graphics, driver `32.0.101.7077`.

### Massa de teste

- dois Álbuns e dois Projetos, cada um com 100 Lâminas lógicas;
- 173 mídias físicas: 172 Fotos JPEG e um Decorativo PNG;
- 1.469.084.414 bytes de originais;
- digest antes e depois:
  `c160ef3e3a2a1c7401f10574e3383fddb830678eb3fbc984dbca28dc59ebd01f`;
- integridade do corpus confirmada.

Cada alternativa gerou 174 representações: as 172 Fotos e uma materialização
do mesmo Decorativo em cada Projeto. Foram 174 gerações, zero reutilizações e
uma representação por mídia e Projeto, limitada a 1.600 px.

## Medições consolidadas

Os valores estão no formato `AB / BA [mínimo–máximo; mediana]`. Menor é melhor,
exceto para vazão. Os objetos não arredondados por Projeto e execução estão nos
JSONs brutos.

| Métrica | A — hosts independentes | B — host multiwindow | Interpretação do protocolo |
| --- | ---: | ---: | --- |
| Duas Janelas prontas | 3.384 / 1.980 ms [1.980–3.384; 2.682] | 2.632 / 2.527 ms [2.527–2.632; 2.579,5] | inconclusiva |
| Cache pronto | 31.336 / 33.668 ms [31.336–33.668; 32.502] | 56.179 / 53.617 ms [53.617–56.179; 54.898] | consistente para A |
| Tempo efetivo do Cache | 26.887 / 30.736 ms [26.887–30.736; 28.811,5] | 52.864 / 50.293 ms [50.293–52.864; 51.578,5] | consistente para A |
| Vazão do Cache | 54.641.077 / 47.798.498 B/s [47.798.498–54.641.077; 51.219.787,5] | 27.790.834 / 29.211.513 B/s [27.790.834–29.211.513; 28.501.173,5] | consistente para A |
| Canvas pronto | 31.421 / 33.938 ms [31.421–33.938; 32.679,5] | 56.362 / 53.782 ms [53.782–56.362; 55.072] | consistente para A |
| Pan, pior p95 | 17,0 / 18,6 ms [17,0–18,6; 17,8] | 15,7 / 18,0 ms [15,7–18,0; 16,85] | inconclusiva |
| Frames de Pan acima de 33 ms | 0 / 0 | 0 / 0 | inconclusiva; empate |
| Zoom, pior p95 | 22,7 / 24,6 ms [22,7–24,6; 23,65] | 18,0 / 19,6 ms [18,0–19,6; 18,8] | consistente para B |
| Frames de Zoom acima de 33 ms | 0 / 2 [0–2; 1] | 0 / 0 | inconclusiva |
| Navegação, pior p95 | 227,2 / 272,5 ms [227,2–272,5; 249,85] | 313,7 / 324,8 ms [313,7–324,8; 319,25] | consistente para A |
| Exportação | 2.376 / 2.485 ms [2.376–2.485; 2.430,5] | 2.458 / 2.482 ms [2.458–2.482; 2.470] | inconclusiva |
| Working set | 2.517.135.360 / 2.359.042.048 B [2.359.042.048–2.517.135.360; 2.438.088.704] | 2.354.089.984 / 2.320.809.984 B [2.320.809.984–2.354.089.984; 2.337.449.984] | inconclusiva |
| Memória privada | 1.937.010.688 / 1.784.885.248 B [1.784.885.248–1.937.010.688; 1.860.947.968] | 1.948.401.664 / 1.926.492.160 B [1.926.492.160–1.948.401.664; 1.937.446.912] | inconclusiva |
| GPU pós-probe | 713.195.520 / 725.217.280 B [713.195.520–725.217.280; 719.206.400] | 688.177.152 / 687.755.264 B [687.755.264–688.177.152; 687.966.208] | consistente para B |
| Árvore de processos | 14 / 14 | 8 / 8 | consistente para B |

O valor de GPU é um snapshot posterior aos probes, não um pico temporal nem
um orçamento global do driver. O p95 de Pan e Zoom ficou abaixo do limite
eliminatório de 33,33 ms em todos os Projetos das duas ordens.

## Igualdade da Exportação e Canvas

As quatro Exportações foram idênticas entre topologias e ordens:

- 7.087 × 3.543 px a 300 DPI;
- 24.382.534 bytes;
- três originais, totalizando 23.777.710 bytes;
- SHA-256
  `ee2cdbb84b9d5eb8f9d1d1691da53c4ed4b57a10e2f517fdf91738b41cd78e1b`.

Cada execução confirmou WebGL2, exercitou uma textura real de 1.600 × 1.200 px
e usou 24 frames de aquecimento por Projeto. Em cada topologia e ordem houve
duas perdas e duas restaurações de contexto, com `glError=0`. Os mínimos
observados foram 16.384 px para textura e renderbuffer e 16 unidades de
textura.

## Robustez e isolamento

### Processo global leve

Nas duas alternativas e ordens, o processo global:

- começou sem Janela visível, em uma única árvore e com prioridade
  `BelowNormal`;
- preservou o singleton; a segunda instância terminou com código 73;
- não reiniciou automaticamente após a queda;
- deixou as duas Janelas concluírem 2/2 edições, Salvamentos e releituras
  offline, sem duplicação ou ausência;
- voltou somente por reinício explícito, com novo PID, e então concluiu 2/2
  operações online;
- não deixou descendentes nem processos inesperados.

### Host de Projeto

Na alternativa A, a queda identificou seis descendentes. A limpeza levou 1.293
ms em `AB` e 3.465 ms em `BA`; o outro host e uma Janela sobreviveram, a
continuidade local concluiu 1/1 intenção e o reinício reabriu exatamente o
Projeto afetado na revisão salva.

Na alternativa B, a queda identificou sete descendentes. A limpeza levou 3.822
ms em `AB` e 1.876 ms em `BA`; nenhuma Janela sobreviveu e o reinício reabriu
exatamente os dois Projetos nas revisões salvas.

Nos quatro casos, todos os descendentes identificados encerraram naturalmente:
zero encerramentos forçados, zero descendentes restantes, zero processos
inesperados e nenhum reinício automático. O Cache das Janelas reabertas ficou
pronto novamente.

### Processador de Imagens e logs

A evidência correlacionada confirmou recuperação do Cache após reinício
explícito do Processador e falha segura da Exportação até uma nova tentativa
explícita. Os checks `protocol`, `cache-temporary-cleanup`,
`imaging-sidecar-build` e `production-recovery-integration` passaram.

A alternativa A produziu quatro streams globais e três streams dos hosts por
ordem; B produziu quatro streams globais e dois streams do host. Não houve
campo obrigatório ausente nem evento de falha de continuidade.

## Custo observável

O instrumento registra responsabilidades e contagens; não estima esforço nem
cria escore sintético.

| Fato observável | A — hosts independentes | B — host multiwindow |
| --- | ---: | ---: |
| Hosts de Projeto | 2 | 1 |
| Processos na árvore | 14 | 8 |
| Vínculos host → global | 2 | 1 |
| Streams dos hosts | 3 | 2 |
| Janelas preservadas após queda de um host | 1 | 0 |
| Projetos reabertos após a queda | 1 | 2 |

Em A, o custo observado inclui mais hosts, processos, vínculos e streams, mas
a falha de um host preserva o outro Projeto. Em B, há menos dessas unidades,
mas o único host amplia o domínio da falha para as duas Janelas. Os hosts de
Projeto usaram prioridade `Normal`; o processo global, `BelowNormal`, com
working set entre 14.823.424 e 14.831.616 bytes.

## Falhas e execuções rejeitadas

Uma primeira execução completa no commit
`18c99417f19ab8d0a4404a63cd055e7fa4cb94ea` foi rejeitada. O pior p95 de
Pan/Zoom foi, respectivamente:

| Ordem | A — hosts independentes | B — host multiwindow |
| --- | ---: | ---: |
| AB | 10,8 / 10,5 ms | 51,8 / 59,9 ms |
| BA | 15,4 / 19,2 ms | 36,5 / 57,8 ms |

O papel global que deveria ser leve e headless ainda criava um WebView oculto.
Isso introduzia trabalho gráfico alheio à topologia definida no protocolo. O
commit `8db69cd53c9a9ecbcf86fcfeb656f0d0101341c0` tornou esse papel realmente
headless e aplicou prioridade `BelowNormal`. Os números rejeitados não foram
misturados aos artefatos finais.

Tentativas posteriores, interrompidas durante limpeza e reinício de hosts,
também foram operacionalmente inválidas e não publicaram resultados parciais.
Os sintomas indicaram uma corrida entre o encerramento da raiz e dos
descendentes WebView2. A versão final captura a identidade da árvore, aguarda
por até cinco segundos o encerramento natural dos descendentes identificados e
força apenas sobreviventes; a espera do Cache falha imediatamente se as
Janelas esperadas desaparecerem. O sucesso das duas ordens com zero
encerramento forçado sustenta essa correção operacional, mas não prova uma
causa interna específica do WebView2.

Não foram adicionadas dependências COM nem recuperação automática apenas para
instrumentar uma falha que deixou de ocorrer no protocolo válido.

## Checks de validade

| Check consolidado | Resultado |
| --- | --- |
| Critérios congelados antes da execução final | passou |
| Ordens `AB` e `BA` balanceadas | passou |
| Mesma build `release` com inputs limpos | passou |
| Mesmo hardware e corpus | passou |
| Mesmos alvos de Canvas | passou |
| Exportações idênticas entre execuções | passou |
| Fatos de robustez registrados | passou |
| Gates de resiliência | passou |
| p95 de Pan e Zoom ≤ 33,33 ms | passou |
| Medições brutas preservadas | passou |
| Custo de implementação registrado | passou |

## Limites

A coleta não mediu:

- alocação sintética de `MAX_TEXTURE_SIZE²` nem indução de OOM;
- pico temporal de GPU ou orçamento global do driver;
- checkpoint automático de alterações não salvas;
- restauração de um gesto em andamento.

Esses limites não pertencem ao critério encerrado e não invalidam os gates
congelados.

## Conclusão

O protocolo final foi cumprido integralmente: hardware, corpus, build,
medições brutas, falhas, robustez e custo observável estão correlacionados e
reproduzíveis. A e B permanecem tecnicamente elegíveis. A recomendação deve
agora ponderar o domínio de falha, o custo operacional e as diferenças por
métrica, explicitar os riscos e atualizar o ADR 0005. Até esse gate seguinte,
o ADR continua `proposed` e nenhuma topologia está escolhida.

## Repetição

Na raiz do repositório:

```powershell
npm run spike:topology-final
```

O comando gera primeiro as rodadas brutas `AB` e `BA` e publica a consolidação
somente depois de validar build, corpus, alvos, Exportações e robustez.

## Fontes normativas

- [Ticket 01 — Plataforma e arquitetura](../../.scratch/programa-diagramacao/issues/01-plataforma-e-arquitetura.md)
- [Protocolo congelado](0027-protocolo-da-comparacao-final-de-topologias.md)
- [ADR 0005 — Tauri, React/TypeScript e Rust](../adr/0005-adotar-tauri-react-rust.md)
- [Especificação do produto](../specs/programa-de-diagramacao-de-albuns.md)
- [ADR 0007 — caminhos Windows e identidade física](../adr/0007-tratar-caminhos-windows-e-identidade-fisica.md)
- [Armazenamento local e Cache](../design/0010-armazenamento-local-e-cache.md)
- [Resolução e política de caminhos](../design/0011-resolucao-e-politica-de-caminhos.md)
- [Propriedade de estado e módulos do núcleo](../design/0012-propriedade-de-estado-e-modulos-do-nucleo.md)
