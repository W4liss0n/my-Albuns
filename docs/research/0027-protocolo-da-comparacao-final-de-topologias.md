---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-08-01
updated: 2026-08-01
---

# Protocolo da comparação final de topologias

## Objetivo

Este documento congela os critérios da comparação final entre as duas
topologias antes da coleta que encerrará o penúltimo critério do ticket 01:

- **A — hosts independentes:** um host de Projeto por Projeto aberto;
- **B — host multiwindow:** um host de Projeto com uma Janela e uma
  `ProjectSession` isolada por Projeto.

A coleta não escolhe a topologia. Ela produz uma base comparável e
reproduzível para a recomendação e a atualização do ADR 0005 no gate seguinte.

## Condições fixas da coleta

As duas alternativas devem usar:

- o mesmo commit e a mesma build `release`;
- entradas da build limpas e identificadas por SHA-256;
- o mesmo executável do host e o mesmo Processador de Imagens;
- dois Projetos simultâneos, cada um com 100 Lâminas lógicas;
- o corpus real atual de dois Álbuns, com 172 Fotos JPEG e um Decorativo PNG,
  total de 1.469.084.414 bytes e SHA-256
  `c160ef3e3a2a1c7401f10574e3383fddb830678eb3fbc984dbca28dc59ebd01f`;
- uma representação de Cache por mídia, limitada a 1.600 px;
- o mesmo alvo de Frame, os mesmos ciclos e as mesmas barreiras de Cache,
  Canvas, interação e Exportação;
- Exportação da mesma Lâmina a 300 DPI, com igualdade de dimensões e SHA-256
  da saída entre A e B;
- o mesmo protocolo de quedas, continuidade, Salvamento e reabertura.

O corpus deve ser identificado por quantidade, volume e digest antes e depois
das execuções. Hardware, Windows, CPU e memória física pertencem à evidência.

## Neutralização da ordem

Serão executadas duas rodadas completas sobre a mesma build:

1. `AB`: A seguida de B;
2. `BA`: B seguida de A.

Cada alternativa começa com o Cache descartável do aplicativo limpo. A ordem
executada deve constar no JSON bruto. Uma rodada por ordem é suficiente para
neutralizar a vantagem sistemática de executar sempre por último; duas
amostras por alternativa não autorizam inferência estatística nem ranking por
diferenças pequenas.

Uma repetição adicional só é necessária se:

- uma alternativa falhar um gate funcional;
- os dois relatórios não pertencerem à mesma build ou ao mesmo corpus;
- a recomendação depender de uma diferença de desempenho instável entre as
  duas ordens.

## Métricas registradas

### Desempenho e recursos

- tempo até duas Janelas identificadas;
- tempo até Cache e Canvas prontos;
- duração e vazão do Cache frio;
- pior p95 e frames acima de 33 ms em Pan e Zoom;
- pior p95 da navegação longa;
- duração e integridade da Exportação;
- working set, memória privada e snapshot de memória gráfica;
- quantidade de processos, hosts e Janelas.

Os valores serão publicados por ordem e como faixa observada. Não será criado
um escore agregado, e uma diferença isolada de tempo ou memória não decide a
topologia.

Pan e Zoom têm uma meta eliminatória de p95 menor ou igual a 33,33 ms em cada
Projeto. Para abertura, Cache, navegação, Exportação e memória não será
inventado limite absoluto: os valores brutos serão comparados. Menor é melhor,
exceto para vazão. Uma diferença só será chamada de consistente quando apontar
na mesma direção nas ordens `AB` e `BA` e for maior que a faixa observada da
própria alternativa; nos demais casos, o resultado será “inconclusivo”.

Para este gate, “medição bruta” significa o objeto não arredondado emitido pelo
instrumento para cada Projeto e execução: contagem, duração, primeira latência,
média, p50, p95, p99, máximo e contagens acima de 16 e 33 ms, além das árvores
de processo e demais registros estruturados. As durações transitórias de cada
frame não são um contrato persistido do probe. Alterar agora o protocolo entre
frontend e host apenas para guardar essa série criaria outra variável entre as
rodadas sem mudar a unidade usada pelos gates já validados.

### Robustez e isolamento

- disponibilidade das duas Janelas durante a queda do processo global;
- continuidade de edição, Salvamento e releitura sem o global;
- retorno somente por reinício explícito protegido por singleton;
- quantidade de Janelas afetadas pela queda de um host;
- reabertura da última revisão explicitamente salva;
- recuperação controlada do Processador durante Cache e Exportação;
- ausência de falhas nos eventos estruturados exigidos.

### Custo de implementação e operação

O custo será registrado por responsabilidades observáveis, sem converter linhas
de código ou opinião em uma pontuação:

| Dimensão | A — hosts independentes | B — host multiwindow |
| --- | --- | --- |
| Ciclo de vida | iniciar, supervisionar e encerrar um host por Projeto | iniciar e supervisionar um host compartilhado |
| Roteamento | correlacionar Projeto, host e operações globais entre processos | rotear cada comando e evento para a Janela e a Sessão corretas dentro do host |
| Dados do WebView2 | manter namespace isolado por host | manter um namespace compartilhado pelo host |
| Coordenação global | coordenar concessões entre mais proprietários de processo | coordenar mais proprietários de Janela dentro do mesmo processo |
| Falha do host | reiniciar somente o Projeto afetado; os pares sobrevivem | reiniciar o host e reabrir todos os Projetos afetados |
| Diagnóstico | correlacionar mais PIDs e streams | distinguir Janelas e Sessões no mesmo PID e stream |

A evidência final acrescentará as contagens observadas de hosts, processos,
relações host → global, Janelas afetadas e Projetos reabertos. Esses fatos
descrevem custo; não antecipam qual compromisso é preferível para o produto.

## Gates de validade

A comparação só será aceita se:

- as rodadas `AB` e `BA` terminarem integralmente;
- ambas registrarem inputs limpos, o mesmo commit, hashes de executáveis e
  digest do corpus;
- A e B medirem os mesmos Projetos e Frames;
- as duas topologias concluírem Cache, Canvas, Pan, Zoom, navegação e
  Exportação;
- todos os p95 de Pan e Zoom forem menores ou iguais a 33,33 ms;
- a saída exportada for idêntica entre A e B em cada rodada;
- perda e restauração WebGL2, continuidade e reabertura passarem;
- a evidência correlacionada de recuperação do Processador pertencer ao mesmo
  commit;
- os JSONs brutos e a consolidação final forem publicados.

Falha de qualquer gate impede fechar o critério. Variação de desempenho, por si
só, é um resultado a registrar e não uma falha.

## Fora do escopo

Esta coleta não:

- escolhe a topologia;
- altera a arquitetura de produção para otimizar o benchmark;
- cria watchdog, eleição, reinício automático ou coordenador universal;
- induz OOM ou aloca `MAX_TEXTURE_SIZE` ao quadrado;
- transforma uma única máquina em estimativa para todo hardware Windows;
- reabre a avaliação WPF/C#, salvo se um gate técnico acordado falhar.

WPF/C# só volta à comparação se nenhuma das duas topologias Tauri permanecer
elegível; a falha isolada de A ou B não reabre a decisão de stack.

## Fontes normativas

Os critérios detalham, sem redefinir:

- o [ticket 01](../../.scratch/programa-diagramacao/issues/01-plataforma-e-arquitetura.md);
- a [especificação do produto](../specs/programa-de-diagramacao-de-albuns.md);
- o [ADR 0005 — Tauri, React e Rust](../adr/0005-adotar-tauri-react-rust.md);
- o [ADR 0007 — caminhos Windows e identidade física](../adr/0007-tratar-caminhos-windows-e-identidade-fisica.md);
- os contratos comuns de [armazenamento](../design/0010-armazenamento-local-e-cache.md),
  [caminhos](../design/0011-resolucao-e-politica-de-caminhos.md) e
  [propriedade de estado](../design/0012-propriedade-de-estado-e-modulos-do-nucleo.md).

## Evidência esperada

O resultado deve publicar um artefato JSON canônico contendo:

- referência e SHA-256 deste protocolo;
- ambiente, corpus, build e ordem de cada rodada;
- referências e SHA-256 dos dois JSONs de medições brutas;
- faixa observada por alternativa;
- fatos de robustez e custo;
- checks de validade e conclusão explícita do critério.

## Repetição

Depois que este protocolo e o runner estiverem no mesmo commit limpo, a coleta
completa é iniciada por:

```powershell
npm run spike:topology-final
```

O comando gera os JSONs `AB` e `BA` separadamente e só publica a consolidação
depois de validar build, corpus, alvos, Exportações e gates de robustez.
