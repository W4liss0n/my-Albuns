---
status: current
document: research
date: 2026-09-29
platform: windows-11-x64
---

# Desempenho com Projetos e imagens em rede

Esta pesquisa mede, com o Host e o Processador reais, as mudanças P1 a P6 do plano
`.scratch/planos/2026-09-29-desempenho-com-projetos-e-imagens-em-rede.md`. A
comparação é entre a versão de referência (commit `1760350f`, sem as mudanças) e a
árvore de trabalho com as mudanças.

## Resultado

| Fluxo (90 fotos, 816 MiB, Wi‑Fi) | Antes | Depois | Mediana |
| --- | ---: | ---: | ---: |
| Primeira abertura sem Cache | 54,2–63,9 s (63,1) | 32,4–38,2 s (36,8) | 1,7× |
| — primeira prévia pronta | 25,8–34,8 s (28,2) | 1,5 s (1,5) | 18× |
| Importação das 90 fotos, cache frio | 66,4–70,6 s (68,5) | 36,0–44,4 s (40,2) | 1,7× |
| — primeira prévia pronta | 30,9–31,4 s (31,2) | 2,2–2,7 s (2,5) | 13× |
| Reabertura: observação dos Originais | 2,8–3,1 s (2,9) | 0,7–0,8 s (0,8) | 3,5× |
| Varredura do Monitor | 3,1–3,3 s (3,1) | 0,7–0,8 s (0,7) | 4,2× |
| Exportação de 6 lâminas para a rede, cache frio | 63,6–67,3 s (65,4) | 53,3–62,3 s (57,8) | 1,1× |
| Exportação de 6 lâminas para disco local, Originais na rede | 14,2–14,7 s (14,5) | 11,1–11,8 s (11,5) | 1,3× |

A abertura, a importação, a reabertura e o Monitor ganharam o esperado. A
Exportação para a rede ganhou pouco: o gargalo encontrado é a gravação em blocos de
8 KiB, descrita em [Gravação na rede](#gravação-na-rede), que nenhuma das mudanças
atacou. A gravação em blocos de 1 MiB, feita depois, levou a mesma Exportação para
a rede de 53,7 s para 22,3 s ([Gravação em blocos de 1 MiB](#gravação-em-blocos-de-1-mib)).

Estes números medem o Host e o Processador sem WebView, diálogos ou janela. Não
são latência de janela nem promessa de tempo para outra rede ou máquina.

## Método

- Windows 11, Intel Core i5-13450HX, 16 processadores lógicos, 24 GB de RAM.
- Rede: Wi‑Fi 6 (Intel AX201, 567 Mbps) até `\\DESKTOP-EDUCMUU`; `ping` de 1 a 7 ms.
- Fotos: cópia, numa pasta de teste do mesmo servidor, dos 90 JPEGs de um trabalho
  real (6 a 12 MiB, 816 MiB). A pasta do cliente só foi lida; a pasta de teste foi
  apagada no fim.
- Dois builds de perfil `dev` (pacotes de pixels em `opt-level = 3`), cada um com
  seu Processador: a referência extraída de `1760350f` e a árvore atual. Cada rodada
  roda num processo novo, alternando os builds.
- Teste `network_bench::network_measurement` (`src-tauri/src/network_bench.rs`,
  ignorado por padrão):
  - **abertura**: segue `product_runtime` e `image_processing`, na ordem: observação
    inicial, duas observações do Monitor, confirmação por cabeçalho, jobs de Cache
    (estimativa, turno remoto e Processador real) e publicação em lote. Depois,
    32 s parado e duas observações (reabertura), mais 32 s e uma varredura do
    Monitor. Três rodadas por build.
  - **exportação**: 30 fotos em 6 lâminas de 600 × 300 mm a 300 DPI, 5 por lâmina,
    JPEG qualidade 100, pelo `ExportPipeline` e pelo Processador real. Duas rodadas
    por build e por destino.
- Importação: `photo_import::native_flow_tests::real_import_flow` com
  `MYALBUNS_IMPORT_MEASUREMENT_INPUTS`, ajustado para seguir a importação de
  produção, inclusive o limite de leituras remotas. Duas rodadas por build.
- Ferramentas e resultados brutos em `.scratch/rede-20260929/` (`tools/run-bench.ps1`,
  `tools/run-p5-import.ps1`, `tools/run-cold.ps1`, `p0/`, `p0-cold/`).

### Cache do cliente SMB

O cliente SMB manteve em memória os arquivos lidos pouco antes: uma importação que
seguia a leitura das mesmas fotos terminou em cerca de 7 s, impossível pela vazão do
Wi‑Fi. Por isso a importação e a Exportação foram medidas com cache frio. Cada
rodada leu uma cópia nova das fotos, feita pelo próprio servidor e apagada em
seguida. A preparação da Exportação passou a ler só o cabeçalho de cada foto, e o
teste de importação deixou de calcular o hash das fotos antes de importar
(`MYALBUNS_IMPORT_MEASUREMENT_COLD`).

A abertura foi medida sobre as mesmas fotos em todas as rodadas; os tempos frios se
repetiram entre rodadas. Dentro de uma rodada, a versão de referência lê cada foto
duas vezes seguidas (cabeçalho e Processador) e a segunda leitura aproveita esse
cache, como aconteceria no aplicativo.

## Detalhes

### Primeira abertura sem Cache

| Etapa | Antes | Depois |
| --- | ---: | ---: |
| Observação inicial | 0,2–3,4 s (0,3) | < 0,1 s |
| Estabilização (duas observações) | 0,4–0,6 s (0,4) | 0,1 s |
| Confirmação por cabeçalho | 23,1–28,3 s (24,1) | 0,1 s |
| Jobs de Cache | 30,5–38,3 s (31,5) | 32,3–38,0 s (36,6) |
| Décima prévia pronta | 28,4–37,7 s (31,2) | 4,5–5,7 s (4,7) |

A confirmação por cabeçalho era uma leitura completa de cada foto (P1). Os jobs de
Cache ficaram cerca de 5 s mais lentos porque passaram a ler as fotos frias; na
referência, eles aproveitavam a leitura feita pela confirmação logo antes.

### Limite de leituras remotas (P5)

A mesma versão atual, com e sem o limite de três leituras completas, três rodadas
alternadas:

| | Com limite | Sem limite |
| --- | ---: | ---: |
| Primeira abertura | 34,2–37,2 s (36,4) | 33,3–34,2 s (34,1) |
| Jobs de Cache | 34,0–37,1 s (35,6) | 33,2–34,0 s (34,0) |
| Primeira prévia | 1,3–2,2 s (1,4) | 2,5–2,9 s (2,5) |
| Décima prévia | 3,9–5,7 s (5,2) | 4,7–5,1 s (4,9) |

O limite antecipa a primeira prévia em cerca de 1 s e custa cerca de 5% do tempo
total dos jobs. Na importação, ele também reduz de 8 para 3 os processos do
Processador.

### Importação

| | Antes | Depois |
| --- | ---: | ---: |
| Observação da seleção no Host | 3,2–3,8 s (3,5) | 0,8–1,2 s (1,0) |
| Processador (lotes) | 57,2–60,6 s (58,9) | 32,4–40,7 s (36,6) |
| Décima prévia pronta | 34,5–35,0 s (34,8) | 5,5–7,1 s (6,3) |

A referência lia cada foto duas vezes pela rede: uma na estimativa de memória de
cada lote e outra no Processador.

### Gravação na rede

A mesma Exportação levou 11,5 s com destino local e 57,8 s com destino na rede, com
as mesmas 30 fotos lidas da rede e os mesmos 76,6 MB gravados. Gravar 13 MiB (um
JPEG exportado) na pasta de rede, com sincronização no fim:

| Bloco de escrita | Tempo | Vazão |
| --- | ---: | ---: |
| 8 KiB | 5,6–6,3 s | 2,1–2,3 MiB/s |
| 64 KiB | 1,4 s | 9,4 MiB/s |
| 1 MiB | 0,8 s | 16,0–16,5 MiB/s |

Os codificadores gravavam por um `BufWriter` com o tamanho padrão de 8 KiB (JPEG e
PDF). Cada bloco vira uma escrita no servidor, e 6 arquivos de ~13 MB somam cerca
de 35 s. O PNG já grava o arquivo inteiro de uma vez.

P4 (uma releitura a menos) e P6 (leitura antecipada das fotos) não mudaram essa
parte. A diferença de 7 s na Exportação para a rede e de 3 s na local é o que eles
renderam.

### Gravação em blocos de 1 MiB

As saídas JPEG e PDF passaram a gravar por um buffer de 1 MiB
(`export_output::output_writer`). A comparação usou o mesmo Host e dois
Processadores: o da árvore com P1 a P6 e o mesmo com o buffer novo. Mesma Exportação
de 6 lâminas, cache frio, ordem alternada a cada rodada.

| Destino (tempo de execução) | 8 KiB | 1 MiB | Mediana |
| --- | ---: | ---: | ---: |
| Rede, Wi‑Fi estável (3 rodadas) | 51,1–57,3 s (53,7) | 20,7–23,3 s (22,3) | 2,4× |
| Disco local, Originais na rede (2 rodadas) | 11,3–11,4 s (11,3) | 11,4–11,6 s (11,5) | igual |

Nas duas primeiras rodadas, o Wi‑Fi estava degradado (`ping` médio de 126 ms, até
330 ms). Elas ficaram fora da tabela, mas o buffer venceu também ali: 62,8 s contra
324,7 s e 88,2 s contra 125,3 s. As saídas somaram os mesmos 76.601.346 bytes em
todas as rodadas. Resultados brutos em `.scratch/rede-20260929/buffer/`, script em
`tools/run-buffer.ps1`.

O ganho ficou perto do previsto: a Exportação para a rede passou a custar cerca de
duas vezes a local, contra cinco vezes antes.

### Servidor desligado (P8)

Com o servidor desligado e o Projeto usando 90 Originais nele, cenário `offline` do
mesmo teste, antes da correção descrita adiante. Cada etapa começou 45 s depois da anterior, para medir o pior caso
(`.scratch/rede-20260929/offline/`).

O Windows espera o servidor uma vez e depois guarda a falha por 20 a 30 s: nesse
intervalo, qualquer acesso ao mesmo servidor falha em 1 ms. Passado esse tempo, o
próximo acesso espera de novo. Abrir um arquivo direto levou 21 s; o caminho que o
programa usa, que consulta a raiz depois da falha, levou 37 s.

| Etapa | Tempo |
| --- | ---: |
| Preparar os caminhos das 90 fotos | 6 ms |
| Observar 1 foto | 37,1 s |
| Observar 10 fotos em sequência | 37,1 s (37,1 s na primeira; 1 ms nas outras) |
| Observar as 90 fotos | 37,2 s (todas indisponíveis) |
| Observar as 90 fotos logo depois | 19 ms |
| Listagem de pastas do Monitor | 37,1 s |
| Duas verificações do Monitor | 39,0 s |
| Pausa do Cache pedida durante a listagem do Monitor | espera 36,6 s |
| Pausa do Cache pedida durante a verificação completa | espera 36,7 s |

O tempo não cresce com o número de fotos: a primeira espera paga por todas, em
sequência ou em paralelo. A hipótese do plano, de N esperas somadas, não se
confirmou, e verificar a raiz antes de cada foto não encurtaria os 37 s.

O problema é outro. O Monitor lista as pastas a cada segundo e verifica todas as
fotos a cada 30 s, segurando uma permissão do Cache enquanto espera o servidor. A
listagem não pode ser interrompida, e uma verificação interrompida ainda espera as
fotos em andamento. Como o Windows esquece a falha em 20 a 30 s, o Monitor volta a
esperar 37 s mais ou menos a cada minuto. Tudo o que pausa o Cache espera junto,
até 37 s, antes de começar: Exportação, Exportação em lote, Salvar como e aplicar a
correção de olhos. Salvar uma cópia do Projeto em disco local, por exemplo, pode
demorar até 37 s só para começar.

### Correção para o servidor desligado

Três mudanças, medidas com o servidor desligado de novo:

- o Monitor lista as pastas antes de segurar a permissão do Cache;
- uma pausa do Cache deixa de esperar as observações em andamento: elas terminam
  sozinhas e o resultado é descartado;
- uma raiz de rede cujo servidor não respondeu fica marcada, e todo acesso a ela
  falha na hora até uma sonda própria, a cada 2 s, encontrar o servidor de novo. Só
  falhas de rede ou de servidor marcam a raiz; um arquivo aberto em outro programa
  não marca.

| Etapa | Antes | Depois |
| --- | ---: | ---: |
| Pausa do Cache durante o primeiro contato com o servidor | 36,7 s | 30 ms |
| Observar as 90 fotos, 45 s depois | 37,2 s | 4 ms |
| Listagem de pastas do Monitor | 37,1 s | 0 ms |
| Duas verificações do Monitor | 39,0 s | 3 ms |

O primeiro contato ainda espera o servidor, mas longe da pausa e de quem a pediu.
Uma observação feita enquanto ele esperava levou 19,3 s: a raiz só fica marcada
quando essa espera termina, e uma conexão aberta pouco antes com o servidor ligado
alongou a espera.

Observando as 90 fotos a cada segundo, como o Monitor, com o servidor desligado:
só o primeiro ciclo esperou o servidor (21 s); os seguintes, durante 7 minutos,
levaram milissegundos. Religado o servidor, as fotos voltaram a ficar disponíveis
22 s depois de ele responder ao `ping`, tempo que inclui o servidor iniciar o
compartilhamento de arquivos. Com o servidor ligado, as 90 fotos continuaram
disponíveis e nenhuma raiz foi marcada.

## Recomendações

- **Manter o limite de três leituras remotas por enquanto**: ele troca ~5% do tempo
  total por uma primeira prévia ~1 s mais cedo. Reavaliar em rede cabeada, onde a
  vazão por leitor muda.

## Limites

- Perfil `dev`: a composição e a codificação rodam mais devagar que no `release`;
  as partes limitadas pela rede não mudam.
- Uma rede e uma máquina. O Wi‑Fi variou entre rodadas, por isso cada comparação
  alternou os builds.
- A Exportação comparou o tamanho total das saídas (idêntico), não o SHA-256 de cada
  arquivo; a igualdade de bytes está coberta pelos testes do Processador.
