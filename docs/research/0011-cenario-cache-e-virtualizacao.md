---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-30
updated: 2026-07-30
---

# Cenário, baseline de Cache e virtualização

## Escopo

Esta rodada fecha o cenário representativo e avança dois gates da fundação:

- o cenário representativo que conecta o host, a interface, o Canvas, o núcleo,
  o Cache e a Exportação;
- o suporte do Processador a uma única representação visual reduzida com
  transparência, antes de qualquer adoção de tiles;
- a preservação de todas as Lâminas no modelo lógico com residência gráfica
  limitada ao viewport e à margem de pré-carga.

Ela não encerra o ticket 01. Perda de contexto WebGL2, pressão de memória
gráfica, topologia final, permissões, caminhos de rede e coordenação global de
operações continuam com critérios próprios. Nesta rodada, o baseline de Cache
permaneceu aberto até uma Imagem decorativa atravessar seus consumidores reais;
esse gate foi encerrado posteriormente em
`0013-baseline-de-cache-com-decorativo.md`. A virtualização foi encerrada
posteriormente pela medição registrada em
`0012-navegacao-em-album-longo.md`.

## Cenário representativo

A execução de 30 de julho de 2026 usou o host independente atual, protocolo de
Imaging v7, Windows 11, a composição de 12 Lâminas e o Álbum A do corpus real.
Os gates do benchmark foram abertos somente depois da conclusão do Cache.

O fluxo observado nos logs estruturados foi:

1. o Projeto `project-spike-001` abriu com 12 Lâminas;
2. o Canvas PixiJS sobre WebGL2 foi inicializado em 1148 × 618 px;
3. o Cache disponibilizou 88 de 88 representações reais;
4. o probe selecionou o Frame `frame-01-a` com textura real e executou 120
   frames de Pan e 120 de Zoom;
5. a Exportação recebeu o snapshot validado, abriu dois originais e publicou
   um PNG a 300 DPI.

O Cache quente reutilizou as 88 representações, cobrindo 647,6 MiB de originais
com 25,6 MiB derivados, em 19,861 s. Não houve
`canvas_texture_load_failed`, `media_cache_failed`,
`topology_benchmark_failed` ou `export_failed`.

As medições do Canvas foram:

| Operação | Amostras | Primeira resposta | Média | p95 | Máximo | Frames acima de 16 ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Pan | 120 | 13,9 ms | 11,569 ms | 16,0 ms | 23,5 ms | 6 |
| Zoom | 120 | 9,7 ms | 10,259 ms | 14,5 ms | 24,6 ms | 2 |

A saída publicada possui 7087 × 3543 px, 28.325.124 bytes e SHA-256
`e2a0d05bde9ffdac7e1e5ec62732e66158a2d02f12f24745bf671fb299ab26de`.
Dimensões e hash foram relidos do arquivo depois da publicação e coincidem com
o evento `export_completed`.

O probe mede a prévia visual e não cria Histórico. A consolidação documental
do mesmo gesto permanece coberta nas fronteiras públicas atuais:

- `AlbumCanvas` mantém Pan e Zoom simultâneos no mesmo preview e envia um único
  commit;
- `ProjectWorkspace` encaminha os dois deltas como uma única intenção;
- `ProjectCore` cria uma revisão e restaura Pan e Zoom juntos por Undo/Redo.

Seleção, máscara e Overlay pertencem à mesma composição usada nessa execução e
continuam cobertos pelo corte visual registrado em
`0006-evidencias-do-corte-vertical-da-plataforma.md`. Não foi criado um segundo
modelo de demonstração para reunir as provas.

## Baseline de Cache

O Processador passa a produzir exatamente um artefato reduzido de até 1600 px
por mídia recebida:

- conteúdo opaco gera JPEG;
- conteúdo com qualquer alfa abaixo de 255 gera PNG;
- o `CacheEngine` publica uma URL de asset compatível com os dois formatos;
- a Exportação continua lendo os originais e não usa a representação reduzida.

O teste de Imagem decorativa usa uma origem PNG RGBA de 2400 × 1800 px com alfa 96.
O Processador produz um único PNG de 1600 × 1200 px, preserva alfa 96, mantém o
original intacto e registra no índice `format: png`, orientação EXIF nula e o
nome terminado em `.png`. Outro teste confirma que uma origem PNG totalmente
opaca continua gerando JPEG. Caminhos finais, temporários, limpeza de
recuperação e URLs de asset cobrem os dois formatos.

O compartilhamento da mesma representação entre Painel, Canvas e Grade foi
provado para Fotos nesta rodada. A Imagem decorativa ainda não atravessava
esses consumidores neste corte; a prova complementar e o encerramento do
baseline estão registrados em `0013-baseline-de-cache-com-decorativo.md`.

A variante PNG alterou o contrato serializado do Processador; por isso o
protocolo corrente foi incrementado de v6 para v7. Evidências históricas
produzidas com v6 permanecem identificadas como tal.

As 172 Fotos JPEG e 1.401,0 MiB medidos em
`0008-cache-com-imagens-reais.md` continuam sendo a massa de desempenho do
baseline. O PNG transparente é uma prova de correção representativa, não um
segundo corpus artificial. Não foram introduzidos tiles, pirâmides, níveis
progressivos ou previews persistidos de Lâmina.

## Virtualização do Canvas

Todas as Lâminas permanecem no `CompositionPlan`. A política gráfica inicial,
agora registrada no design da Janela, funciona assim:

1. converte o viewport para as coordenadas lógicas do Álbum;
2. expande a faixa em 700 unidades lógicas para cada lado;
3. inclui ainda uma Lâmina adjacente antes e depois da faixa encontrada;
4. destrói nós PixiJS que saem da residência;
5. descarrega texturas que deixam de ser desejadas, inclusive cargas
   assíncronas concluídas com atraso;
6. reconstrói nó e textura a partir do modelo lógico quando a Lâmina retorna.

Um teste integrado de `AlbumCanvas` constrói 100 Lâminas e navega da primeira
para a 50ª, depois para a 100ª e de volta para a primeira. A composição continua
com 100 Lâminas, enquanto a cena mantém no máximo oito nós na configuração
testada. A navegação descarrega as texturas abandonadas, não reutiliza nós
destruídos e recarrega corretamente a primeira textura ao retornar.

O número oito é consequência do viewport e da margem usados nesse teste, não
um limite documental. Os parâmetros de residência podem ser recalibrados por
medições futuras sem alterar o Projeto.

O teste prova o comportamento funcional. A execução posterior com o host real,
100 Lâminas por Projeto, texturas do Cache e 30 saltos por Projeto está em
`0012-navegacao-em-album-longo.md`. Ela encontrou no máximo sete Lâminas e
14 texturas residentes, preservou a latência de Pan/Zoom e limitou o pior salto
observado a 283,2 ms. Com essa evidência, o critério de virtualização foi
encerrado sem introduzir tiles ou outro modelo de cena.

## Conclusão

O cenário representativo está concluído sem antecipar subsistemas posteriores:

- existe um único cenário editorial atravessando as fronteiras reais;
- o Processador preserva transparência na representação única; a Imagem
  decorativa que ainda faltava neste corte foi exercitada posteriormente em
  `0013-baseline-de-cache-com-decorativo.md`;
- o Álbum lógico não é truncado e o descarte, a reconstrução e a latência da
  navegação longa foram provados pela medição complementar.
