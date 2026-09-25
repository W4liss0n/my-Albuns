---
status: current
document: research
date: 2026-09-24
platform: windows-11-x64
---

# Abertura de Projeto

A abertura de um Projeto com 172 fotos reais foi medida no aplicativo, do
lançamento até a interface pronta, com e sem Cache. Três mudanças reduziram o
tempo sem alterar o que a abertura garante. A referência de código é o commit
`f621cd08`.

## Resultado

| Cenário | Antes | Depois |
| --- | ---: | ---: |
| Abrir com o Cache pronto | 6,9–13,1 s | 3,7–5,2 s |
| Abrir sem Cache | 56–80 s | 22,4–23,0 s |

Com o Cache pronto, as etapas mudaram assim:

| Etapa | Antes | Depois |
| --- | ---: | ---: |
| Lançamento até o Host iniciar | 1,9–4,6 s | 1,1–2,5 s |
| Validação das prévias do Cache | 2,4–4,5 s | 0,53–0,61 s |
| Janela e carregamento da página | 0,5–0,9 s | 0,5–0,7 s |
| Interface até pronta (Projeto, Canvas, primeira demanda de prévias) | 1,0–2,6 s | 1,0–1,4 s |

Sem Cache, a confirmação das fotos caiu de 36 s para cerca de 0,5 s. O restante
é o Processador gerando as 172 prévias, que o diálogo de abertura aguarda por
decisão do [design 0010](../design/0010-armazenamento-local-e-cache.md).

## O que mudou

- **Validação das prévias do Cache.** Antes de criar a janela, o Host lê,
  decodifica e calcula o SHA-256 de cada prévia indexada. As verificações eram
  feitas uma a uma; agora são divididas entre até oito threads. Uma entrada
  inválida continua descartando o índice inteiro.
- **Confirmação das fotos sem Cache.** O Host decodificava cada Original por
  inteiro, uma foto por vez, apenas para confirmá-lo, e o Processador
  decodificava o mesmo Original de novo para gerar a prévia. Quando a
  preparação entrega o Original ao Processador logo em seguida, a confirmação
  agora lê só o cabeçalho (formato, dimensões e orientação). O Processador
  continua recusando um conteúdo danificado. Os demais fluxos (Monitor, nova
  tentativa, visualizador) mantêm a inspeção completa.
- **Lançamento do Host.** O processo global criava o diálogo de progresso e só
  depois lançava o Host. Os dois agora começam juntos; o progresso das imagens
  enviado pelo Host antes de o diálogo existir é entregue quando ele fica
  pronto.

Os registros anteriores a esta medição também mostram de 2 a 11 s na criação
do WebView2 quando cada Projeto aberto pela primeira vez recebia um perfil
novo. As vagas de perfil reutilizáveis, publicadas no mesmo dia, removeram esse
custo; nas rodadas acima o perfil já existia.

## Método

- Windows 11, Intel Core i5-13450HX, 16 processadores lógicos, 24 GB de RAM,
  com outros programas ativos; por isso os números aparecem como faixas.
- Corpus `benchmark-data/albums`: 172 JPEGs reais de 6 a 24 MP, fora do Git.
- O Projeto foi criado pelo fluxo real de importação
  (`photo_import::native_flow_tests::real_import_flow`) com o Processador real,
  salvo no formato v1 e acompanhado do Cache gerado.
- Aplicativo compilado com `tauri build --debug --no-bundle`, como os gates
  nativos. Os pacotes de pixels e o `sha2` rodam com `opt-level = 3`; o
  restante, sem otimização completa.
- Cada rodada abriu o Projeto pelo executável, numa pasta de dados isolada
  (`MYALBUNS_PROCESS_GATE_DATA_ROOT`), e terminou no evento `project_ui_ready`.
  As etapas internas do Host foram registradas por marcações temporárias, fora
  do código publicado.
- Com o Cache pronto: três rodadas depois de uma de aquecimento. Sem Cache:
  duas rodadas, removendo o Cache antes de cada uma.

## Pendências

- O Host ainda espera a janela inicial do processo global ficar pronta, cerca
  de 1 s depois do lançamento. Iniciá-lo antes exigiria reorganizar a ativação
  global, que hoje depende dessa janela.
- A primeira demanda de prévias leva de 0,7 a 1 s para 172 fotos e não foi
  alterada.
- Sem Cache, a geração das prévias é limitada pela memória estimada de cada
  foto de 24 MP; é o custo dominante restante nesse cenário.
