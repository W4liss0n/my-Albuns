---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-29
updated: 2026-07-30
---

# Cache com imagens reais

## Resumo

Esta rodada substitui as paletas procedurais por representações derivadas de
dois Álbuns reais no corte comparativo das topologias. O corpus contém 172
Fotos JPEG, totaliza 1.401,0 MiB e permanece local, fora do Git. Nenhum nome de
pasta ou caminho original entra nos artefatos de pesquisa versionados.

O baseline produz uma única representação JPEG de até 1600 px por Foto. A
mesma representação atende ao Painel de imagens, ao Canvas PixiJS e às
miniaturas de Lâmina. Não foram introduzidos tiles, pirâmides ou previews
persistidos de Lâmina.

Os dados brutos estão em
[0002-topology-real-images.json](artifacts/0002-topology-real-images.json). O
[resumo gerado](artifacts/0002-topology-real-images.md) é a fonte canônica dos
valores desta coleta.

## Fronteiras implementadas

O preparador local inspeciona exatamente duas pastas diretas, aceita somente
JPEG, lê dimensões e orientação, calcula SHA-256 de cada original e grava um
manifesto ignorado sob `.scratch`. O fingerprint do conjunto permite confirmar
que as duas alternativas receberam a mesma massa.

Os caminhos permanecem na infraestrutura nativa:

- o scaffolding privado do ensaio monta um documento persistido com identidade,
  nome e dimensões das Fotos, e `ProjectCore::open_editable_session` o abre pela
  mesma entrada de produção;
- `ProjectHost` mantém as fontes nativas fora do documento e as associa à sessão
  e à Janela corretas;
- `MyAlbuns.Imaging.exe` recebe um pedido imutável, aplica orientação EXIF,
  reduz e publica o artefato por temporário irmão sincronizado;
- `metadata.json` guarda identidade, geração, dimensões, tamanho e fingerprint,
  sem caminho do original;
- `AppPaths` valida o artefato dentro do Cache autorizado e devolve a URL de
  asset já restrita; o bridge TypeScript apenas a encaminha;
- o domínio TypeScript e os componentes visuais recebem apenas a URL da
  representação, sem acesso genérico ao sistema de arquivos.

O protocolo de asset do Tauri está restrito à árvore
`%LOCALAPPDATA%\MyAlbuns2\Cache`. A Exportação de prova continua usando seu
snapshot validado e não foi desviada para o Cache.

## Método

O comando `npm run spike:topology` executou:

1. inventário e fingerprint do corpus;
2. build `release` do host e do Processador de Imagens, com manifesto contendo
   commit, digest das entradas e hash dos dois executáveis;
3. limpeza nativa somente dos dois namespaces descartáveis do ensaio,
   redescobertos por `AppPaths`; a operação recusa reparse points, descendentes
   inesperados e qualquer alvo fora da raiz física autorizada;
4. abertura sequencial dos dois hosts independentes, validação integral do
   SHA-256 de cada Foto e reconstrução das 172 representações;
5. espera dos eventos nativos de conclusão e amostragem da árvore de processos,
   memória e contadores gráficos;
6. queda forçada de um host e verificação da Janela restante;
7. nova limpeza nativa e repetição com duas Janelas no host multiwindow;
8. novo inventário completo do corpus e gravação do relatório somente depois de
   conferir os hashes dos executáveis e o digest das entradas da build.

A prontidão do Cache não é inferida pelo aparecimento da Janela. Ela termina
somente quando cada Projeto registra a contagem, o volume de origem, o volume
derivado e a duração de sua operação correlacionada.

## Resultado

As duas alternativas geraram 172 de 172 representações, sem reutilização, e
produziram o mesmo volume derivado de 49,3 MiB. A verificação independente
encontrou exatamente 172 JPEGs, todos decodificáveis e com aresta máxima
observada de 1600 px. Os dois índices não contêm caminhos de origem e o digest
do corpus antes e depois permaneceu
`4da593ff7d12f861a8b3aba74249726e70131468a68af7a7d06200f5340871cc`.

Nesta amostra:

- A deixou as duas Janelas com Cache pronto em 45,3 s, teve 43,4 s de trabalho
  de Cache frio e processou os originais a 32,3 MiB/s;
- B deixou as duas Janelas com Cache pronto em 22,8 s, teve 22,2 s de trabalho
  de Cache frio e processou os originais a 63,1 MiB/s;
- A ocupou 1.121,7 MiB de working set agregado; B, 1.067,8 MiB;
- a queda de um host de A preservou a outra Janela; a queda do host de B
  encerrou ambas.

Esses números são uma amostra com ordem fixa, não uma conclusão estatística.
O resultado confirma que o instrumento distingue carga real, armazenamento,
memória e domínio de falha, mas não autoriza escolher a topologia. A ordem deve
ser alternada e repetida na aceitação final.

## Limites

Este corte não é o `CacheEngine` completo. Cada uso já recalcula o SHA-256 e
recusa uma fonte alterada, inclusive quando o tamanho permanece igual, mas
ainda faltam invalidação e reconstrução automáticas, agrupamento e cancelamento
de jobs, pausa pelo `OperationLease`, recuperação após queda do Processador,
limpeza de gerações órfãs e o mesmo ciclo para Imagens decorativas.

Também permanecem sem medição:

- latência e suavidade de Pan/Zoom com as texturas reais;
- efeito da política já implementada de residência no viewport e descarte via
  `Assets.unload` durante navegação prolongada;
- perda e restauração do contexto WebGL2;
- Exportação a partir dos originais reais;
- recuperação persistida e custo operacional da IPC.

Por isso o ticket 01 e o critério geral de Cache continuam abertos. A evidência
atual apenas estabelece que uma representação única é tecnicamente viável
como baseline e deve ser medida graficamente antes de qualquer adoção de
tiles.

## Estado posterior

Em 30 de julho de 2026, o corte registrado em
`0011-cenario-cache-e-virtualizacao.md` acrescentou a
representação PNG para conteúdo com transparência, manteve JPEG para conteúdo
opaco e executou o Canvas com textura real. Isso remove a lacuna de
transparência do Processador, mas o critério permanece aberto até uma Imagem
decorativa atravessar os consumidores reais. Os limites históricos desta coleta
permanecem válidos; o `CacheEngine` completo continua sendo trabalho posterior.

## Repetição

Preparar ou validar somente o corpus:

```powershell
npm run spike:corpus
```

Executar build e coleta completas:

```powershell
npm run spike:topology
```

Repetir com o executável já validado:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/Measure-TopologySpike.ps1 -SkipBuild
```
