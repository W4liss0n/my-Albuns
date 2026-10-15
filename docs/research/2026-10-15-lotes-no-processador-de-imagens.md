---
status: current
document: research
date: 2026-10-15
platform: windows-11-x64
---

# Lotes no Processador de Imagens

Medição da preparação de prévias da abertura fria com lotes de até 16 imagens
por Processador de Imagens, contra o `main` em `b0e93baa`, que abre um
Processador por imagem. A
[medição anterior](2026-10-13-progresso-de-abertura-em-pc-fraco.md) e a
divisão de custos de 15/10/2026 mostravam que o intervalo entre um processo e o
seguinte ocupava cerca de 27% da preparação e que ~130 ms por foto dentro do
processo não tinham dono.

## O que mudou

- O Host agrupa a preparação de mídias já vinculadas (abertura, recuperação de
  espaço, mídias reencontradas) em lotes de até 16 imagens, divididos por igual
  entre as vagas, conforme o
  [contrato de importação](../design/0020-importacao-com-decode-unico-e-lotes.md).
  A demanda do viewport continua com um Processador por imagem.
- O Processador registra, por imagem, o tempo de cada etapa no evento
  `cache_job_completed` e a soma no `cache_request_completed`: resolução do
  caminho, leitura com SHA-256, verificação para reaproveitar, cabeçalho,
  decodificação, redução, codificação e publicação.

## Como foi medido

Laboratório Hyper-V, perfis `cliente-ssd` (2 vCPU, 4 GB, SSD SATA) e
`referencia` (4 vCPU, 8 GB), álbum de teste de 300 fotos de 24 a 30 MP,
instalação limpa e abertura fria seguida de morna, como na medição anterior. As
rodadas de cada perfil foram feitas na mesma sessão, alternando os instaladores
(A, B, A, B), porque o limite de CPU da VM não vale neste computador e a carga
do computador muda ao longo do dia. Os tempos da preparação saem dos logs do
app: do primeiro processo de imagem ao último. "Por imagem e vaga" é a
preparação vezes o número de vagas (2 no `cliente-ssd`, 4 no `referencia`)
dividida por 300.

Rodadas em `D:\HardwareLab\runs` (fora do Git): `cliente-ssd` em
`20261015-122947`, `20261015-123759`, `20261015-124604` e `20261015-125430`;
`referencia` em `20261015-131922`, `20261015-132636`, `20261015-133306`,
`20261015-145226`, `20261015-150233`, `20261015-151050`, `20261015-151733` e
`20261015-152410`.

## Resultado no `cliente-ssd`

| Rodada | Instalador | Editor visível (fria) | Preparação | Processos | Por imagem e vaga | Abertura morna |
|---|---|---:|---:|---:|---:|---:|
| A1 | `main` | 89,7 s | 75,2 s | 300 | 501 ms | 8,0 s |
| B1 | lotes | 61,5 s | 50,8 s | 20 | 339 ms | 7,2 s |
| A2 | `main` | 87,0 s | 76,5 s | 300 | 510 ms | 7,3 s |
| B2 | lotes | 68,7 s | 56,6 s | 20 | 378 ms | 7,5 s |

Na média, o editor aparece 23 s antes (88,4 → 65,1 s, −26%) e a preparação cai
de 75,9 para 53,7 s (−29%). A abertura morna não muda: ela não prepara
imagens. Os valores absolutos do `main` ficaram acima dos 79,7 s da medição da
manhã; a comparação vale entre rodadas da mesma sessão.

O ganho passou da estimativa de 27%. Além do intervalo entre processos, cada
Processador gastava tempo próprio para iniciar (DLLs, log, handshake e o
registro de escritor no Cache) que agora acontece uma vez por lote.

## Resultado no `referencia`

Com 4 vagas, 300 imagens viram 20 lotes de 15, cinco por vaga. Foram duas
sessões: a primeira (A1, B1, A2) e uma segunda rodada B2 oitenta minutos depois,
que não serve de par; depois uma sessão completa (A3, B3, A4, B4).

| Rodada | Instalador | Editor visível (fria) | Preparação | Processos | Por imagem e vaga | Abertura morna |
|---|---|---:|---:|---:|---:|---:|
| A1 | `main` | 54,3 s | 44,1 s | 300 | 588 ms | 3,8 s |
| B1 | lotes | 38,0 s | 31,0 s | 20 | 413 ms | 3,7 s |
| A2 | `main` | 44,1 s | 35,7 s | 300 | 476 ms | 4,4 s |
| B2 | lotes | 72,0 s | 50,5 s | 20 | 673 ms | 4,5 s |
| A3 | `main` | 65,1 s | 50,9 s | 300 | 679 ms | 3,7 s |
| B3 | lotes | 41,7 s | 31,3 s | 20 | 417 ms | 5,0 s |
| A4 | `main` | 46,6 s | 37,5 s | 300 | 500 ms | 4,6 s |
| B4 | lotes | 49,8 s | 38,8 s | 20 | 517 ms | 3,8 s |

Na sessão completa, o editor aparece 10 s antes (55,9 → 45,8 s, −18%) e a
preparação cai de 44,2 para 35,1 s (−21%). Entre rodadas vizinhas, os lotes
ganham 4,7 a 19,6 s de preparação, com uma exceção: B4 ficou 1,3 s (3%) acima
de A4. B2 e B4 são as rodadas em que a máquina estava mais lenta antes de
qualquer lote: a decodificação, que é o mesmo código nos dois instaladores, teve
mediana de 305 e 239 ms por imagem, contra 164 e 174 ms em B1 e B3; em B2 a
preparação só começou 17,6 s depois da abertura, contra 5 a 13 s nas outras.

Os lotes não deixam vagas paradas no fim: nas quatro rodadas com lotes, as
vagas terminam a menos de 1,6 s umas das outras, contra menos de 0,4 s no
`main`. O ganho com 4 vagas é menor que com 2 porque o custo de iniciar um
processo por imagem já se dividia entre mais vagas.

### Onde vai o tempo de cada imagem

Mediana por imagem nas rodadas com lotes do `cliente-ssd`, dentro do
Processador:

| Etapa | B1 | B2 |
|---|---:|---:|
| Leitura e SHA-256 | 34 ms | 38 ms |
| Cabeçalho e EXIF | < 1 ms | < 1 ms |
| Decodificação (em escala) | 172 ms | 187 ms |
| Redução e orientação | 12 ms | 13 ms |
| Codificação JPEG 84 | 45 ms | 47 ms |
| Publicação (gravação, conferência da origem, renomeação) | 15 ms | 16 ms |
| Total | 301 ms | 327 ms |

O total por imagem dentro do processo era de 347 ms no `main`; com lotes ele
fica em 301 a 327 ms e as etapas somam o total. Os ~130 ms sem dono se dividiam
entre a decodificação mais lenta no app que no teste isolado (172–187 contra
~150 ms, com duas decodificações ao mesmo tempo e a máquina a 100%), a leitura
com SHA-256 (34–38 ms) e o custo de iniciar um processo por imagem. A
decodificação continua sendo a maior etapa, cerca de 57% de cada imagem.

### Memória e falhas

- Pico de memória dos Processadores de Imagens: 99 e 91 MB com lotes, contra
  72 e 98 MB no `main`, no `cliente-ssd`; 112 a 170 MB com lotes, contra 105 a
  155 MB no `main`, no `referencia`. Um Processador mantém um Original por
  vez, e a admissão continua reservando a maior estimativa do lote, como no
  [design 0032](../design/0032-revisao-da-admissao-por-memoria.md).
- Nenhuma imagem falhou nas rodadas. Os testes cobrem uma imagem danificada no
  meio de um lote (só ela falha) e uma imagem que fica obsoleta durante o lote
  (não é publicada; as outras são).

## Limites

- Máquinas com mais de 4 núcleos não foram medidas. Com mais vagas, os lotes
  ficam menores (300 imagens em 8 vagas dão 24 lotes de 12 ou 13) e o ganho
  deve cair, como caiu de 2 para 4 vagas.
- A variação entre rodadas no laboratório chega a 20 s na abertura fria do
  mesmo instalador; uma diferença de poucos por cento entre duas rodadas não é
  sinal.
- Um lote ocupa sua vaga da máquina até a última imagem (cerca de 5 s nos dois
  perfis). Com a janela do Projeto oculta durante a abertura, ninguém espera por
  ela; numa preparação com o editor visível, uma prévia pedida pelo viewport
  pode esperar até um lote terminar.
- Se o Processador cair no meio de um lote, a recuperação refaz o lote inteiro
  uma vez, como já acontecia com a importação. Um pânico do decodificador numa
  imagem agora vira falha só dessa imagem.
- As prévias continuam entrando no índice só no fim da preparação, como antes.
  Publicar ao fim de cada lote preservaria o trabalho feito se o app fechar no
  meio da abertura; ficou fora desta mudança.
