---
status: accepted
document: design
date: 2026-09-01
updated: 2026-09-01
ticket: 3-programa-04-renderizador-final
---

# Contrato do Renderizador final

## Objetivo, autoridade e limite

Este documento fecha o contrato do Programa 04 para que Canvas, Exportação
normal e lote possam depender de uma única interpretação da composição. Ele
consolida as decisões já aceitas sobre estado, caminhos, primeiro JPEG e
Publicação e completa as regras de raster, PNG, PDF, captura dos Originais,
falhas, numeração e provas douradas.

O contrato é normativo para qualquer implementação posterior do
`CompositionCore`, `ExportPipeline` e Processador de Imagens. Quando houver
diferença, ele substitui somente os recortes transitórios do
[primeiro fluxo JPEG](0014-contrato-jpeg-do-primeiro-fluxo.md), preservando os
comportamentos que aquele fluxo já entregou. Não escolhe crate, biblioteca de
PDF, quantidade de processos, paralelismo ou orçamento definitivo de memória.

Este Programa não implementa a integração completa de caminhos da issue #13,
JPEG/PNG da issue #35, PDF da issue #37, remoção de Saídas órfãs da issue #38
nem execução e retomada do lote da issue #39. Ele define as fronteiras que essas
entregas precisam compartilhar.

## Valores imutáveis na fronteira

`RenderSnapshot` é a única autoridade criativa da tentativa. Ele contém a
Revisão visível congelada, inclusive mudanças ainda não salvas, o DPI e um
`CompositionPlan` já resolvido pelo `CompositionCore`, acompanhado do conjunto
exato de descritores de fonte `mediaId + NativePathDto` que originou seus fatos
de geometria e de uma `SourceObservation` por descritor. A observação congela
formato e variante detectados, dimensões codificadas e orientadas,
`embeddedOrientation`, `appliedOrientation` e identidade do ICC permitido ou
ausência assumida como sRGB, além do digest `sha256-full-file-v1`. O pathname
reversível permanece no descritor, não é duplicado no plano. Criar o snapshot é
o único ponto em que o estado do Projeto é interpretado para aquela tentativa.
`ProjectCore` é a única interface pública que o produz e valida, coordenando a
revisão da `ProjectSession` com as observações autoritativas do `MediaResolver`.
Inspeção e hashing dos Originais são I/O assíncrono fora da thread da interface;
a revisão só é congelada como snapshot depois que o resultado completo volta ao
proprietário e continua atual.

`RenderSnapshot` é a entrada lógica pública de `ExportPipeline::plan` e a única
entrada criativa de `MyAlbuns.Imaging.exe`. Depois de validar o snapshot,
`ExportPipeline` o conserva inteiro em um `ImagingRenderEnvelopeV1` owned e
versionado; não cria uma segunda projeção criativa capaz de divergir. O wrapper
acrescenta `selectedUnitIds`, o mesmo `RootBindingPlan` congelado e os valores
operacionais e de correlação necessários à tentativa. `selectedUnitIds` é uma
lista ordenada, sem repetição, cujos IDs precisam existir no `CompositionPlan`
do snapshot. JPEG e PNG selecionam exatamente uma unidade; PDF seleciona uma ou
mais unidades ordenadas para sua única saída.

Os demais valores formam um schema fechado: `attemptId` UUID v4 canônico e
`cancellationId` opaco; `formatOptions` como união marcada
`jpeg { quality: 1..=100 }`, `png` ou `pdf`, sem `quality` nos dois últimos; e
`preparation` com o Destino, sua filha direta reservada
`.myalbuns-export-{attemptId}.tmp` e o pathname da saída dentro dessa filha.
`projectId` e Revisão permanecem dentro do snapshot como proveniência opaca para
o Processador, não como autorização para reabrir o Projeto. O envelope rejeita
documento `.myalbuns`, Sessão e Cache, além de `revision`, `dpi`,
`CompositionPlan`, unidades ou fontes duplicadas fora de `renderSnapshot`.

Todo pathname autoritativo desse grafo reutiliza diretamente o `NativePathDto`
owned por `myalbuns-paths`. No wire Windows, sua única forma é
`{"encoding":"windowsUtf16","units":[...]}`; o corpus e o envelope não
definem alias, campo abreviado nem segundo codec para preparação, fontes ou
raízes.

O snapshot conserva todos os descritores e observações exigidos pelo seu plano,
inclusive os de unidades fora da seleção. O `ExportPlanner` calcula o fecho
transitivo dos `mediaId`s usados por `selectedUnitIds`; somente essas fontes são
abertas e revalidadas, e somente suas raízes precisam estar no
`RootBindingPlan`. Uma fonte não selecionada não bloqueia a tentativa. Os bytes
dos Originais vêm do `CapturedSourceSet` separado e só podem preencher esse
fecho quando `mediaId + NativePathDto + SourceObservation` coincidem exatamente
com o snapshot. Dados operacionais não podem alterar a composição.

O Processador não lê `.myalbuns`, não consulta a Sessão, não usa Cache como
fonte e não invoca `CompositionCore`: ele consome o `CompositionPlan` já
validado dentro do snapshot e não resolve uma segunda versão de Background,
Frame, Foto ou Overlay. A IPC carrega valores owned e versionados; nenhum
participante conserva uma segunda cópia mutável do Projeto.

## Medidas físicas, DPI e unidades de saída

Todas as medidas que chegam ao núcleo são micrômetros inteiros. `mm`, `cm` e
`pol` são apenas representações de entrada e apresentação: a conversão da
interface precisa produzir um micrômetro exato, e trocar a Unidade do Projeto
não altera os inteiros autoritativos. O renderizador nunca converte o texto
exibido nem arredonda uma medida já persistida.

O DPI é um inteiro positivo congelado no snapshot. Para qualquer aresta física
assinada, a única conversão é, com aritmética inteira verificada:

```text
rasterEdge(um, dpi) = floor((um * dpi + 12.700) / 25.400)
```

Uma dimensão positiva usa a mesma fórmula e precisa resultar em pelo menos um
pixel. Multiplicação, soma, quantidade de pixels e conversão ao tipo final
falham de forma fechada em overflow ou quando ultrapassam os limites de recurso
da versão implementada. Nenhuma etapa usa ponto flutuante para decidir arestas.

Retângulos físicos são semiabertos: `[x0, x1) × [y0, y1)` vira
`[rasterEdge(x0), rasterEdge(x1)) × [rasterEdge(y0), rasterEdge(y1))`. Medir as
duas arestas separadamente impede lacunas e sobreposições entre elementos
adjacentes.

Uma Lâmina dupla de `600.000 × 300.000 µm` a `300 DPI` mede
`7.087 × 3.543 px`. A divisão física está em `300.000 µm`, portanto a aresta
central é `3.543`: a esquerda ocupa `[0, 3.543)` e a direita
`[3.543, 7.087)`. O pixel excedente pertence à direita.

As Unidades de Exportação são derivadas antes da rasterização:

- `Por lâmina` produz uma unidade para cada Lâmina selecionada, usando toda a
  sua superfície ativa;
- `Por página` produz uma unidade para cada Página ativa, na ordem da
  Numeração de Página;
- a numeração percorre somente lados ativos, sem reservar índice para lado
  inativo: uma abertura direita seguida de Lâmina dupla e fechamento esquerdo
  produz `right #1 | left #2, right #3 | left #4`, sem lacuna nem unidade para
  os lados ausentes;
- uma Página de Lâmina dupla é o recorte físico da metade correspondente e tem
  a origem traduzida para `(0, 0)` sem mudar as transformações dos elementos;
- uma Lâmina estruturalmente de página única já possui somente sua Página
  ativa; aplicações de Ambos os lados preenchem essa superfície;
- cada Página de `300.000 × 300.000 µm` é rasterizada independentemente como
  `3.543 × 3.543 px`; ela não é um corte de `3.543` ou `3.544 px` feito depois
  de criar o raster da Lâmina.

Um elemento que cruza o centro é recortado no domínio físico em cada unidade.
Seu mapeamento de fonte continua sendo o da composição original; traduzir a
origem da Página não reestica nem recentraliza a Foto. Background ou Overlay de
Ambos os lados também conserva o mapeamento contínuo da Lâmina quando uma
Lâmina dupla é exportada Por página. Conteúdo por lado já possui um mapeamento
independente para cada Página.

## Contrato puro do `CompositionCore`

`CompositionCore` é uma transformação pura e total sobre estado criativo
validado e fatos imutáveis de geometria de mídia:

```text
compose(creativeState, sourceGeometryFacts) -> CompositionPlan | validationError
```

Ele não abre arquivos, não decodifica codecs, não consulta relógio, sistema de
arquivos, Cache ou GPU e não cria nomes de saída. Para a mesma entrada
versionada, devolve o mesmo plano ordenado. Fatos transitórios de uma fonte só
entram depois de associados ao mesmo `mediaId + path` que os originou; ausência
ou divergência bloqueia o snapshot em vez de usar uma dimensão de outra versão.
O construtor do snapshot projeta de cada `SourceObservation` somente
`mediaId + NativePathDto`, dimensões orientadas e `appliedOrientation` para essa
entrada pura; `embeddedOrientation`, formato, variante e perfil continuam
anexos ao snapshot para a validação de captura e não influenciam a composição.
`CompositionCore` transporta a orientação efetiva no plano, mas não abre nem
gira pixels.

O plano fixa, em micrômetros:

- superfície ativa, recortes e origem de cada unidade possível;
- base opaca, Backgrounds, Frames e Overlays já resolvidos por escopo;
- ordem estável da Pilha visual;
- transformações e recortes de cada Foto;
- Borda e Opacidade de cada Frame;
- conjunto exato de `mediaId`s referenciados.

Frames são ordenados por `zIndex` crescente; empate usa `frameId` canônico em
ordem lexical. A ordem presente no plano é definitiva para Canvas e
Exportação. Nenhum adaptador pode recorrer à ordem de um mapa, à ordem de
decode ou ao momento em que uma textura ficou disponível.

### Transformação afim congelada

Valores criativos não chegam ao plano como ponto flutuante aberto. Pan e Zoom
são inteiros em milionésimos, o Ângulo é um inteiro em décimos de grau e o
Giro é normalizado para `0..=3`. Para `angleTenths`, a tabela trigonométrica
versionada contém:

```text
sinQ32[k] = roundHalfAwayFromZero(sin(pi * k / 1.800) * 2^32)
cosQ32[k] = roundHalfAwayFromZero(cos(pi * k / 1.800) * 2^32)
```

O domínio de `k` cobre o Ângulo somado ao Giro. Esses inteiros, e não a função
trigonométrica da plataforma em tempo de execução, são os valores usados pelo
`CompositionCore`. Uma coordenada Q32.32 é um inteiro assinado dividido por
`2^32`; conversão final usa arredondamento para o mais próximo, com empate para
longe de zero.

As operações canônicas, sempre com intermediários `i128` verificados, são:

```text
ONE = 4.294.967.296
roundSigned(n, d) = floor((n + d/2) / d), se n >= 0;
                    -floor((-n + d/2) / d), se n < 0
qRatio(n, d) = roundSigned(n * ONE, d)
qInt(n) = n * ONE
qMul(a, b) = roundSigned(a * b, ONE)
qDiv(a, b) = roundSigned(a * ONE, b)
linearQ(a, x, b, y) = roundSigned(a * x + b * y, ONE)
```

`d` é sempre positivo. Soma linear arredonda depois de somar os produtos, não
cada parcela separadamente. Divisão por zero, overflow em qualquer intermediário
ou resultado fora de `i64` é `ResourceLimitExceeded`. Em JSON/IPC, cada inteiro
Q32.32 é uma string decimal canônica para não perder precisão em JavaScript.

No sistema físico, X cresce para a direita e Y para baixo. Para Giro visual
anti-horário, `R = [[C, S], [-S, C]]`, onde `C` e `S` são as entradas Q32.32
da tabela. Pan positivo move o centro da Foto nos eixos locais já girados;
Espelhamento não inverte o sentido do controle. Para eliminar qualquer escolha
implícita de quantização, a direta e a inversa são construídas exatamente nesta
ordem (`max` compara os inteiros Q32.32 e cada chamada arredonda uma vez):

```text
m = -ONE, se mirrorHorizontal; senão ONE
W = qInt(innerWidthUm); H = qInt(innerHeightUm)
Sw = qInt(orientedWidthPx); Sh = qInt(orientedHeightPx)

requiredU = linearQ(abs(C), W, abs(S), H)
requiredV = linearQ(abs(S), W, abs(C), H)
baseScale = max(qDiv(requiredU, Sw), qDiv(requiredV, Sh))
scale = qMul(baseScale, qRatio(userZoomMillionths, 1.000.000))

overflowU = qDiv(max(qMul(Sw, scale) - requiredU, 0), qInt(2))
overflowV = qDiv(max(qMul(Sh, scale) - requiredV, 0), qInt(2))
panU = qMul(qRatio(panXMillionths, 1.000.000), overflowU)
panV = qMul(qRatio(panYMillionths, 1.000.000), overflowV)

centerX = qInt(innerXUm) + qDiv(W, qInt(2))
centerY = qInt(innerYUm) + qDiv(H, qInt(2))
photoCenterX = centerX + linearQ(C, panU, S, panV)
photoCenterY = centerY + linearQ(-S, panU, C, panV)

Dxx = qMul(qMul(m, C), scale)
Dxy = qMul(qMul(m, S), scale)
Dyx = qMul(-S, scale)
Dyy = qMul(C, scale)
sourceCenterX = qDiv(Sw, qInt(2))
sourceCenterY = qDiv(Sh, qInt(2))
Dtx = photoCenterX - linearQ(Dxx, sourceCenterX, Dxy, sourceCenterY)
Dty = photoCenterY - linearQ(Dyx, sourceCenterX, Dyy, sourceCenterY)

Ixx = qDiv(qMul(m, C), scale)
Ixy = qDiv(-S, scale)
Iyx = qDiv(qMul(m, S), scale)
Iyy = qDiv(C, scale)
Itx = sourceCenterX - linearQ(Ixx, photoCenterX, Ixy, photoCenterY)
Ity = sourceCenterY - linearQ(Iyx, photoCenterX, Iyy, photoCenterY)
```

O plano conserva `physicalFromSourceQ32 = [Dxx, Dxy, Dtx, Dyx, Dyy, Dty]`
e `sourceFromPhysicalQ32 = [Ixx, Ixy, Itx, Iyx, Iyy, Ity]`, sempre como seis
inteiros Q32.32. A primeira leva coordenadas de borda da fonte ao espaço físico;
a segunda é construída pela inversa analítica na mesma sequência inteira, sem
inverter a matriz já quantizada nem usar ponto flutuante. As duas são autoridade
do Canvas e da Exportação; nenhum consumidor refaz preenchimento, Pan, Ângulo ou
trigonometria. Para uma unidade com origem física `(ox, oy)`, o
`ComposedOutputUnit` deriva e congela a matriz destino→fonte usando os centros:

```text
physicalX(x) = ox + (2 * x + 1) * 12.700 / dpi
physicalY(y) = oy + (2 * y + 1) * 12.700 / dpi
```

Essa projeção não combina novamente os racionais. A partir de
`sourceFromPhysicalQ32 = [Ixx, Ixy, Itx, Iyx, Iyy, Ity]`, sua ordem exata é:

```text
step = qRatio(25.400, dpi)
p0x = qInt(ox) + qRatio(12.700, dpi)
p0y = qInt(oy) + qRatio(12.700, dpi)

Sxx = qMul(Ixx, step); Sxy = qMul(Ixy, step)
Syx = qMul(Iyx, step); Syy = qMul(Iyy, step)
Stx = Itx + linearQ(Ixx, p0x, Ixy, p0y)
Sty = Ity + linearQ(Iyx, p0x, Iyy, p0y)
```

`sourceFromDestinationQ32 = [Sxx, Sxy, Stx, Syx, Syy, Sty]` é congelada no
`ComposedOutputUnit`. Seus seis coeficientes também são Q32.32; aplicá-los aos
índices inteiros `(x, y)` não introduz novo arredondamento. Em uma Página direita,
`ox` continua sendo a origem física dessa Página na Lâmina, embora o índice
raster local reinicie em zero. Assim o recorte não recentraliza a Foto.

## Composição canônica

Cada Unidade de Exportação começa com base branca opaca `#FFFFFF`. A Pilha
visual é processada nesta ordem:

1. Backgrounds resolvidos, na ordem do plano;
2. grupos de Frame, na ordem estável entre Frames;
3. Overlays resolvidos, na ordem do plano.

Background e Overlay de imagem são esticados independentemente nos eixos X e Y
até seu retângulo resolvido. Não aplicam preservação de proporção, Pan ou Zoom.
O alfa da fonte é preservado até `source-over`: um Background transparente
revela a base branca e um Overlay transparente revela as camadas inferiores.

Para retângulo físico `[rx, ry, rw, rh]` e fonte já orientada `Sw × Sh`, o
stretch usa coordenadas de borda e a ordem inteira abaixo:

```text
xx = qRatio(Sw, rw); yy = qRatio(Sh, rh)
xy = yx = 0
tx = -qMul(xx, qInt(rx)); ty = -qMul(yy, qInt(ry))
```

Essa `sourceFromPhysicalQ32` passa pela mesma projeção
`sourceFromDestinationQ32` definida acima para Fotos. Portanto um decorativo
de Ambos os lados conserva uma única parametrização contínua ao cruzar o centro
e ao gerar Páginas independentes; conteúdo por lado deriva sua própria matriz.

Cada Frame é composto primeiro em uma camada transparente própria:

1. o Original já orientado recebe Giro anti-horário de `90°` em quartos de
   volta;
2. recebe Ângulo contínuo anti-horário ao redor do centro;
3. recebe Espelhamento horizontal no sistema já girado;
4. recebe o Zoom de preenchimento que cobre a área interna;
5. recebe o multiplicador de Zoom do usuário;
6. recebe Pan limitado para não revelar vazio;
7. recebe Efeitos, atualmente Preto e branco;
8. é recortado à área interna do Frame;
9. a Borda é desenhada por cima, inteiramente para dentro do retângulo do
   Frame;
10. a Opacidade do Frame é aplicada uma única vez ao grupo Foto mais Borda;
11. o grupo é composto sobre as camadas inferiores.

A Borda reduz somente a área visível disponível à Foto. Um placeholder em uma
unidade selecionada invalida o plano de Exportação; ele nunca vira vazio,
branco ou transparência no arquivo final.

O rasterizador aplica a matriz inversa a partir do centro de cada pixel de
destino. Coordenadas da fonte usam bordas inteiras e centro do texel `(i, j)`
em `(i + 0,5, j + 0,5)`. Uma coordenada fora de
`[0, Sw) × [0, Sh)` é transparente. Dentro dela, subtrai-se `0,5`; a parte
fracionária Q32.32 vira peso Q16 por arredondamento half-up. Resultado `65.536`
avança o índice e vira fração zero. Os quatro vizinhos são limitados à borda
da fonte depois dessa confirmação, portanto o kernel de um ponto interno usa
`CLAMP_TO_EDGE` sem criar franja transparente.

Os quatro pesos bilineares são produtos Q16 e somam exatamente `2^32`. Para
cada texel RGBA8 não associado `k`, interpola-se alfa e cor premultiplicada sem
arredondamento intermediário:

```text
sumA = sum(weightQ32[k] * A[k])
sumPC = sum(weightQ32[k] * C[k] * A[k])
sampleA = floor((sumA + 2^31) / 2^32)
sampleC = 0, se sumA = 0; senão floor((sumPC + sumA / 2) / sumA)
```

Se `sampleA = 0`, seus canais também são zero. Efeitos, hoje Preto e branco,
são aplicados ao resultado não associado da amostragem e preservam seu alfa.
Essa regra distingue o contrato de nearest-neighbor, interpolação em canais
não associados e convenções diferentes de centro de pixel. Mudança deliberada
de amostrador exige nova versão do corpus dourado e não pode chegar como efeito
colateral de trocar encoder.

Preto e branco usa luminância sRGB inteira, igualmente nos três canais:

```text
y = floor((54 * r + 183 * g + 19 * b + 128) / 256)
rgb = (y, y, y)
```

Opacidade apresentada em percentuais vira um byte somente no plano:

```text
opacityByte = floor((percent * 255 + 50) / 100)
```

O compositor recebe e devolve RGBA8 sRGB não associado em cada operação
normativa. Uma implementação pode manter valores premultiplicados internamente,
mas precisa reproduzir os mesmos arredondamentos. Para opacidade de grupo `G`,
inclusive a Opacidade aplicada conjuntamente à Foto e à Borda:

```text
effectiveAlpha = floor((sampleAlpha * G + 127) / 255)
groupAlpha = effectiveAlpha
groupColor = sampleColor, ou zero quando groupAlpha = 0
```

A Borda é uma fonte opaca dentro do anel do Frame e usa `source-over` sobre a
Foto na camada transparente antes dessa opacidade de grupo. O inset satura
separadamente como `insetX = min(borderWidthUm, floor(frameWidthUm / 2))` e
`insetY = min(borderWidthUm, floor(frameHeightUm / 2))`; se a área interna
ficar vazia, a Foto não contribui. Retângulos da Borda seguem `rasterEdge`: uma
Borda física positiva pode ocupar zero pixels e não é forçada a um pixel.

Para fonte `s` sobre destino `d`, em canais não associados, a operação completa
é:

```text
alphaDen = sA * 255 + dA * (255 - sA)
outA = floor((alphaDen + 127) / 255)
outC = 0, se alphaDen = 0; senão
       floor((sC * sA * 255 + dC * dA * (255 - sA) + alphaDen / 2) /
             alphaDen)
```

Para destino opaco, a fórmula se reduz a
`floor((sC * sA + dC * (255 - sA) + 127) / 255)`. A forma completa cobre Foto,
PNG, TIFF e Borda sem assumir alfa opaco intermediário.

Com base opaca, todo pixel ao fim da Pilha precisa ter alfa `255`. Alfa residual
é `CompositionFailed`; JPEG, PNG e PDF nunca escolhem fundos diferentes.

## Originais e normalização de fonte

O contrato de entrada é o resultado do Programa 03 — Mídias externas e Cache
(issue #11), não o artefato de Cache. O `CompositionPlan` leva `mediaId` e os
fatos observados necessários à geometria; o `RenderSnapshot` associa cada ID ao
descritor separado com pathname nativo reversível. Um fato só é aceito quando
seu par `mediaId + NativePathDto` coincide exatamente com a `MediaRef` criativa
que o solicitou. A `SourceObservation` congelada registra formato, variante,
dimensões antes e depois da orientação, `embeddedOrientation`,
`appliedOrientation`, identidade ou ausência do perfil e
`sha256-full-file-v1`; ela é evidência a ser revalidada, não autorização para
reutilizar Cache. `embeddedOrientation` é o valor `1..=8` encontrado no
metadado permitido, normalizado para `1` quando ausente. `appliedOrientation` é
a instrução efetiva que o normalizador deve aplicar exatamente uma vez: igual
ao valor embutido para JPEG e TIFF e sempre `1` para PNG, mesmo quando existe
um chunk `eXIf`. Somente esse segundo campo entra em `SourceGeometryFacts` e no
plano; ele não declara que `CompositionCore` já girou pixels. Metadados do
Monitor, fingerprint e representação reduzida podem antecipar problemas, mas
não autorizam pixels finais.

Cada tentativa reabre os Originais e detecta o formato pelo conteúdo. A matriz
final aceita:

| Fonte | Normalização |
|---|---|
| JPEG RGB, YCbCr ou tons de cinza, 8-bit, baseline ou progressivo | RGB8; `appliedOrientation = embeddedOrientation`, aplicada exatamente uma vez |
| PNG estático RGB, RGBA, indexado ou tons de cinza, nas profundidades válidas até 16 bits | RGBA8; `tRNS` preservado; `embeddedOrientation` é observado, mas `appliedOrientation = 1` |
| TIFF de uma única página, RGB, RGBA ou tons de cinza, 8 ou 16 bits | RGBA8; `appliedOrientation = embeddedOrientation`, aplicada exatamente uma vez; alfa associado ou não associado declarado por `ExtraSamples` |
| APNG, TIFF multipágina, CMYK, YCCK, multibanda ou variante fora da matriz | falha tipada antes da composição |

Redução inteira de 16 para 8 bits usa
`floor((value16 + 128) / 257)`. Fonte sem ICC é assumida sRGB. Fonte com ICC só
é aceita quando corresponde à allowlist sRGB e aos digests fixados no
[primeiro fluxo JPEG](0014-contrato-jpeg-do-primeiro-fluxo.md#fontes-e-política-de-cor).
Perfil desconhecido, malformado, Adobe RGB, Display P3 ou declarações PNG
contraditórias produzem `UnsupportedColorProfile`; não existe conversão
silenciosa de gamut. A normalização gera RGBA8 sRGB e toda saída incorpora o
`sRGB2014.icc` v2 canônico.

Em TIFF, duas amostras `Gray + A` ou quatro `RGB + A` exigem exatamente uma
entrada `ExtraSamples` para o canal alfa, declarada como `AssociatedAlpha` ou
`UnassociatedAlpha`. Ausência, quantidade incompatível ou `Unspecified`
produz `UnsupportedSourceVariant`; o decoder não presume que a segunda ou a
quarta amostra seja alfa. Com `ExtraSamples=AssociatedAlpha`, cada canal é desassociado na
profundidade original antes da redução: para máximo `M`, alfa zero produz cor
zero; nos demais casos, `C = min(M, floor((P * M + A / 2) / A))`. Em
`UnassociatedAlpha`, os canais já são retos. Canal associado maior que o alfa
é `DecodeFailed`. PNG fornece alfa não associado conforme seu formato.

Se formato, variante, dimensões codificadas ou orientadas,
`embeddedOrientation`, `appliedOrientation`, perfil ou ausência de perfil
diferir da `SourceObservation` congelada, a tentativa não recompõe com os novos
fatos. Ela falha como `SourceChanged`; uma nova tentativa atualiza a observação
e cria outro snapshot.

## Captura estável dos Originais

Uma tentativa usa um `CapturedSourceSet` imutável. Depois de congelar todas as
raízes e antes de renderizar a primeira unidade, o Processador:

1. ordena as referências únicas por `mediaId`;
2. resolve cada pathname somente pelo `RootBindingPlan` congelado;
3. abre cada arquivo regular com leitura compartilhada, mas negando escrita e
   exclusão enquanto a captura estiver em uso;
4. obtém identidade física, tamanho e datas pelo handle aberto;
5. faz o preflight de codec, calcula `sha256-full-file-v1` no mesmo handle e
   compara todos os campos, inclusive o digest, à `SourceObservation`;
6. confirma que identidade, tamanho e datas não mudaram durante a leitura;
7. mantém o handle e os fatos normalizados até todas as preparações que usam a
   fonte estarem verificadas.

Referências diferentes cuja identidade física é `Same` apontam para a mesma
captura. `Different` conserva capturas distintas. Se a comparação necessária
for `Indeterminate`, se o filesystem não puder sustentar a posse estável ou se
qualquer evidência mudar durante a captura, a preparação inteira termina com
`SourceIdentityIndeterminate` ou `SourceChanged`; nenhum nome final é tocado.

Todas as Unidades de Exportação leem a mesma captura. Uma edição externa não
pode fazer a primeira Página usar uma versão e a segunda usar outra. Depois que
os arquivos preparados foram sincronizados e verificados, seus bytes já são a
representação da captura e não voltam ao pathname. Uma nova tentativa sempre
captura novamente a versão então corrente.

## Raster canônico e adaptadores de formato

Cada `ComposedOutputUnit` é rasterizada exatamente uma vez para um
`CanonicalRaster { widthPx, heightPx, dpi, colorSpace: srgb2014, pixels:
RGBA8-opaque }`. O adaptador de formato recebe esse valor; nenhum encoder
recompõe, redimensiona, aplica orientação ou consulta Originais.

### JPEG

O slider público produz um inteiro `quality` em `1..=100`, passo `1`. O valor é
entregue sem curva ou remapeamento ao parâmetro de qualidade `1..=100` do
encoder controlado. Cada abertura da Exportação normal começa em `100`, dois
cliques restauram `100` e lote usa sempre `100`.

JPEG final é RGB8, baseline sequencial SOF0, três componentes e subsampling
`4:4:4`. APP0 JFIF registra X/Y iguais ao DPI; APP2 contém o perfil
`sRGB2014.icc`. EOI termina o arquivo. Nenhum EXIF, Orientation, XMP, GPS,
thumbnail, comentário, data ou metadado de fonte é copiado.

Uma troca de encoder pode mudar tabelas de Huffman e quantização, ordem interna
de segmentos permitidos, tamanho, SHA-256, bytes comprimidos e erros dentro da
tolerância decodificada do corpus. Não pode mudar dimensões, DPI, perfil,
baseline, `4:4:4`, ausência de metadados, raster de entrada nem ordenação das
unidades. Hash do JPEG é recibo daquela tentativa, nunca golden permanente.
O erro de cada amostra é `abs(decodedChannel - canonicalChannel)` depois de
confirmar dimensões e perfil. O máximo considera todos os canais de todos os
pixels; a média é `sum(error) / (width * height * 3)`. Ambos precisam respeitar
os limites do caso; a comparação inteira é
`sum(error) <= meanLimit * width * height * 3`, sem arredondar a média para
baixo.

### PNG

PNG final é estático, não entrelaçado, `truecolor` RGB8 e sem canal alfa, pois o
raster canônico já é opaco. Ele incorpora `sRGB2014.icc` e registra densidade
com `pHYs`:

```text
pixelsPerMeter = floor((dpi * 10.000 + 127) / 254)
```

O arquivo decodificado precisa reproduzir exatamente o RGB do raster canônico.
Nível de Deflate, filtros por scanline, tamanho, chunking e SHA-256 podem mudar
com o encoder. APNG, chunks de texto, EXIF, horário e metadados herdados não
integram a saída.

### PDF

PDF não possui compositor próprio. Ele recebe, na ordem das Unidades de
Exportação, os mesmos rasters canônicos que alimentariam JPEG ou PNG. Cada
unidade vira exatamente uma página; não há round-trip por JPEG ou PNG, nova
amostragem, margem, linha central, marca de corte ou recomposição.

Cada página usa `MediaBox` e `CropBox` iguais à dimensão física da unidade:

```text
points = micrometers * 72 / 25.400
pointsScaled6 = floor((micrometers * 72 * 1.000.000 + 12.700) / 25.400)
```

O valor racional é emitido com até seis casas decimais, arredondado para o mais
próximo com empate para cima conforme `pointsScaled6`, removendo somente zeros
decimais finais e sem alterar os pixels. Uma imagem RGB8 lossless, associada a um
`ICCBased` sRGB2014, cobre exatamente a caixa. O teste extrai e compara o raster
embutido e a caixa; renderização por visualizador ou screenshot não é oráculo.
Compressão Flate, números de objetos, xref, ID e bytes do documento não são
golden.

A ordem de páginas é exatamente a ordem das Unidades de Exportação. Cada página
referencia um único raster correspondente e usa `CropBox = MediaBox`, sem
`Rotate`; a matriz de posicionamento é `[widthPoints 0 0 heightPoints 0 0]`, de
modo que a imagem cobre a caixa sem margem, recorte adicional ou rotação. O
corpus representa números PDF como decimais canônicos para também provar o
arredondamento de seis casas.

## `ExportPipeline`: planejamento, preparação e Publicação

A interface pública permanece em duas etapas:

```text
plan(renderSnapshot, exportOptions) -> ExportPlan
execute(exportPlan, rootBindingPlan, cancellation, progress) -> ExportResult
```

`ExportPlanner` valida o snapshot e fecha antes da execução: escopo, modo,
formato, qualidade, unidades e sua ordem, dependências, todas as raízes, nomes,
Destino, conflitos e autorização `CreateOnly` ou `ReplaceConfirmed`. Nenhum
worker resolve uma raiz tardia. Capturar `RootBindingPlan` e todo I/O que possa
alcançar disco ou rede ocorre fora da thread da interface.

`ExportExecutor` captura Originais, materializa cada raster canônico, entrega-o
ao adaptador escolhido e sincroniza e verifica todas as saídas na mesma pasta
de preparação da tentativa. PDF prepara um arquivo com todas as páginas;
JPEG/PNG preparam um arquivo por unidade. Falha ou cancelamento nesta fase
preserva todos os nomes finais.

Antes da primeira promoção, o Destino passa por uma prova descartável de
criação atômica e, quando o plano contém `ReplaceConfirmed`, também de
substituição atômica, dentro da própria pasta de preparação no volume real.
Falta de suporte à primitiva exigida produz `AtomicReplacementUnsupported`; nunca se usa
`delete + move`, cópia por cima ou outro fallback destrutivo. A primitiva real é
validada novamente no momento de cada promoção.

Somente depois de todas as preparações e da prova de capacidade o `Publisher`
assume a tentativa. Ele promove arquivos na ordem do plano, com atomicidade por
arquivo e sem backup. Não existe atomicidade, rollback ou manifesto do conjunto,
conforme o [ADR 0006](../adr/0006-publicar-exportacao-com-transacao-limitada.md).
Cancelamento antes desse claim remove a preparação; depois dele retorna
`TooLate` para o pedido e não interrompe a sequência nem substitui o terminal
final da Exportação.

O adaptador da primitiva devolve `Committed` ou uma falha com evidência do alvo:
`UntouchedByAttempt`, `CandidateAtFinal` ou `Indeterminate`.
`UntouchedByAttempt` e `CandidateAtFinal` exigem prova positiva por identidade e
conteúdo; um erro de I/O sem essa prova é sempre `Indeterminate`. Essa distinção
é necessária porque a própria documentação de
[`ReplaceFileW`](https://learn.microsoft.com/windows/win32/api/winbase/nf-winbase-replacefilew)
define erros nos quais nomes ou streams já foram alterados.

O `Publisher` incrementa `attemptedCount` antes de chamar a primitiva e
`confirmedPromotedCount` depois de `Committed` ou de
`Failed(CandidateAtFinal)`. Ele para no primeiro resultado diferente de
`Committed` e classifica o estado acumulado como:

- `NotStarted`, antes de qualquer chamada contra nome final;
- `UntouchedByAttempt`, somente quando nenhuma promoção foi confirmada e a
  falha provou que o primeiro alvo ficou intacto;
- `AllCandidatesConfirmed`, quando todos os candidatos foram confirmados nos
  respectivos nomes finais, seja após todos os resultados `Committed`, seja
  porque a falha final ainda provou `CandidateAtFinal`;
- `PossiblyMixed`, quando já existe candidato confirmado e faltam saídas, ou
  quando qualquer alvo ficou `Indeterminate`.

`0 <= confirmedPromotedCount <= attemptedCount <= totalFileCount` é invariante.
Conclusão de todas as chamadas como `Committed` em `AllCandidatesConfirmed` é
`Completed`. Falha em `NotStarted` preserva seu código de preparação. Falha em
`UntouchedByAttempt`, antes de qualquer promoção confirmada, é
`PublicationFailed` e leva `failedOutput`, causa e evidência final. Depois da
primeira promoção confirmada, qualquer falha da primitiva termina como:

```text
PartialPublication {
  totalFileCount,
  attemptedCount,
  confirmedPromotedCount,
  failedOutput,
  failedTargetEvidence,
  cause
}
```

Logo, a primeira chamada inconclusiva já é `PartialPublication` com contagens
`1/0/N`, mesmo sem promoção confirmada. A segunda chamada comprovadamente
intacta depois de uma promoção também é parcial, com `2/1/N`. Uma falha final
com `CandidateAtFinal` conserva `AllCandidatesConfirmed` como evidência dos
nomes planejados, mas também é `PartialPublication`: o lote não concluiu todas
as chamadas como `Committed`, nenhuma Saída órfã pode ser removida e o contrato
aceito não permite apresentar esse refinamento como sucesso ou dispensar a
nova Exportação integral. A propriedade `possibleMixedSet: true` é implícita no
variante e não é um booleano que possa divergir.

Esse terminal nunca é sucesso, não remove Saídas órfãs e orienta uma nova
Exportação integral. Ele não tenta restaurar os arquivos já promovidos. Saídas
órfãs finais só podem ser tratadas, pela issue #38, depois de sucesso integral e
confirmação aplicável.

## Falhas estáveis

Mensagens localizadas pertencem à interface. A fronteira conserva códigos e
contexto estruturado, no mínimo:

| Evidência | Terminal |
|---|---|
| raiz confirmada, Original inexistente | `SourceNotFound` |
| rede, servidor ou compartilhamento sem resposta conclusiva | `SourceUnavailable` |
| acesso insuficiente | `SourceAccessDenied` |
| objeto não é arquivo regular | `UnexpectedObjectType` |
| raiz não pertence ao plano | `UnboundRoot` |
| identidade ou posse estável não pode ser provada | `SourceIdentityIndeterminate` |
| conteúdo ou fatos mudam durante a captura | `SourceChanged` |
| codec, variante, modelo ou perfil fora da matriz | código `Unsupported*` correspondente |
| dimensão, aritmética ou memória fora do limite | `ResourceLimitExceeded` |
| destino indisponível ou sem acesso | `DestinationUnavailable` ou `DestinationAccessDenied` |
| nome final surge sob `CreateOnly` | `ExportConflict` |
| Destino não sustenta a primitiva exigida | `AtomicReplacementUnsupported` |
| falha da primitiva com alvo comprovadamente intacto e zero promoções | `PublicationFailed` com contagens, causa e evidência |
| falha final com `CandidateAtFinal` e todos os candidatos confirmados | `PartialPublication` com contagens, alvo, causa e evidência; nova Exportação integral recomendada |
| falha em `PossiblyMixed`: candidatos faltantes depois de promoção, ou alvo `Indeterminate`, inclusive na primeira chamada | `PartialPublication` com contagens, alvo e causa |
| cancelamento antes do claim | `Cancelled` |
| pedido de cancelamento depois do claim | `TooLate` para o pedido; a Exportação segue até seu terminal |

`NotFound` só é emitido quando a raiz estava acessível. `Unavailable`,
`AccessDenied` e `Indeterminate` nunca são convertidos em ausência ou Religação
automática.

## Preparação e resíduos

Uma tentativa possui uma única pasta física direta sob o Destino, com nome
reservado `.myalbuns-export-{attemptId}.tmp`; `attemptId` é UUID v4 canônico.
Ela contém somente saídas do plano, arquivos descartáveis da prova atômica e
um marcador de ownership versionado. Um handle exclusivo do marcador fica
aberto enquanto a tentativa é ativa.

Sucesso, falha e cancelamento tratáveis removem os arquivos preparados, o
marcador e a pasta depois de validar novamente que são filhos físicos diretos,
regulares e sem reparse point. Falha de limpeza não autoriza ampliar o alvo:
preserva o terminal principal e acrescenta `PreparationResidue` com o pathname
opaco da tentativa.

No próximo uso do mesmo Destino, antes de criar outra preparação, o pipeline
inspeciona somente nomes que correspondam exatamente ao namespace reservado.
Ele só recolhe uma pasta quando:

- é filha física direta do Destino aberto e não é reparse point;
- o UUID do nome coincide com o marcador interno versionado;
- o marcador pode ser adquirido com exclusividade, provando que não existe
  proprietário ativo;
- todo descendente é um arquivo regular direto permitido pelo schema do
  marcador, sem subdiretório, link ou reparse point.

Ausência de qualquer prova preserva a pasta e reporta
`PreparationCleanupFailed`; nunca provoca remoção por prefixo, busca recursiva
ou caminho textual não verificado. Esse resíduo temporário é diferente de uma
Saída órfã final.

## Nomes e índices acima de 999

JPEG e PNG usam `{Nome do Projeto}_{índice}.{ext}`. O índice é decimal positivo,
sem sinal, com largura mínima de três dígitos:

```text
1 -> 001
9 -> 009
10 -> 010
998 -> 998
999 -> 999
1000 -> 1000
1001 -> 1001
```

Não há truncamento, retorno a zero, sufixo adicional ou limite funcional em
`999`; limites do tipo inteiro e do pathname continuam falhando antes da
preparação. `000` e zeros à esquerda além da largura mínima não são nomes
canônicos. A gramática usada pela limpeza de órfãos é equivalente a
`00[1-9] | 0[1-9][0-9] | [1-9][0-9]{2,}`, depois de escapar exatamente o Nome
do Projeto e exigir a extensão selecionada.

No modo Por lâmina o índice é a posição original da Lâmina; no modo Por página
é a Numeração de Página. Exportação parcial não renumera. PDF continua sendo um
único `{Nome do Projeto}.pdf` e não usa índice.

## Corpus dourado executável

O manifesto
[`tests/fixtures/final-renderer-cases-v1.json`](../../tests/fixtures/final-renderer-cases-v1.json)
é parte deste contrato. `schemaVersion: 1` congela entradas pequenas e oráculos
antes do encoder. O teste desserializa um schema fechado e tipado; campo
ausente, desconhecido, terminal livre ou variante não reconhecida falha. Uma
versão futura é aditiva ou publica novo schema; não edita silenciosamente o
significado de um caso existente.

O corpus possui três seams complementares, sem fingir que o tipo parcial hoje
implementado já é o contrato final:

1. `CreativeStateV1 + MediaRefV1 + SourceObservationV1 +
   SourceGeometryFactsV1 -> ExpectedCompositionPlanV1` traz estado criativo
   consumível, associação exata de pathname, projeção pura dos fatos e o plano
   criativo completo esperado;
2. `ComposedOutputUnitV1 + NormalizedSourceV1 -> CanonicalRasterV1` traz a
   projeção imutável, fontes normalizadas e pixels esperados;
3. `ImagingRenderEnvelopeV1` transporta o `RenderSnapshotV1` completo e liga
   seus `selectedUnitIds` ao mesmo `RootBindingPlanV1`, à união fechada de
   formato e opções, à tentativa e seu cancelamento, e à preparação canônica sob
   o Destino. O schema rejeita documento, Sessão, Cache e qualquer cópia de
   Revisão, DPI, plano, unidades ou fontes fora do snapshot.

O primeiro seam prova interpretação, ordem e geometria. O segundo prova a
matriz Q32.32, centros de pixel, bilinear, alfa premultiplicado, Borda e
Opacidade sem reimplementar `CompositionCore`. Assets codificados reais levam
pathname relativo, tamanho e SHA-256, e congelam fatos antes e depois da
orientação. O terceiro prova que o adapter futuro recebe o snapshot inteiro,
que a seleção aponta somente para unidades dele e que nenhuma projeção criativa
paralela pode divergir; ele é oráculo do contrato final, não alegação de que o
protocolo JPEG transitório já transporta todos esses campos.

O corpus cobre:

- largura raster ímpar, centro e Páginas independentes;
- entrada criativa completa, ordem por `zIndex` e desempate por `frameId`,
  transformação fracionária e unidades resolvidas;
- abertura com somente a Página direita, Lâmina interna dupla e fechamento com
  somente a Página esquerda, registrando o mesmo ID para `CompositionCore`,
  Canvas e `ExportPipeline` e fixando as Páginas ativas como `1, 2, 3, 4`;
- Pilha visual, stretch anisotrópico de decorativo de Ambos os lados contínuo
  na Página direita, bilinear por centro de pixel, transparência
  premultiplicada, Borda, Opacidade de grupo e Frame cruzando o centro;
- asset JPEG real assimétrico com Orientation `6` aplicada uma vez e na direção
  correta; separadamente, a composição cobre Giro, Espelhamento e Preto e
  branco sem alegar que são o mesmo seam;
- asset PNG real com alfa e chunk `eXIf` Orientation `6`, provando que a
  observação congela `embeddedOrientation = 6`, projeta
  `appliedOrientation = 1` e não aplica a orientação implicitamente; o caso de
  normalização referencia diretamente a observação, o fato geométrico e o
  digest integral desse asset, além da `NormalizedSource` consumida pelo raster
  canônico ligado ao plano;
- assets TIFF reais de 16 bits com alfa associado RGBA e não associado GrayA,
  além de rejeição tipada de quatro amostras sem `ExtraSamples`;
- rasters canônicos na mesma ordem para JPEG, PNG e PDF, inclusive caixa PDF
  com arredondamento decimal não inteiro;
- qualidade, metadados, perfil, densidade, caixas de PDF e tolerâncias
  observáveis por formato;
- índices `998`, `999`, `1000` e `1001`;
- empate half-up e carry da conversão Q32.32 para Q16;
- alteração de Original, falta de atomicidade, primeira e última promoções com
  evidência, Publicação parcial e limpeza de órfãos somente após sucesso.

Cada caso de composição declara `adapterRegistrations` com os mesmos IDs para
`CompositionCore`, Canvas e `ExportPipeline`. Esse registro atribui o caso a
cada fronteira futura, mas não é evidência de que uma implementação a executou:
um adapter só prova consumo quando realmente projeta sua entrada e compara sua
saída. Neste Programa, o oráculo Rust independente compara o plano completo e
as unidades, enquanto o adapter real do Canvas compara geometria e ordem sem
camadas transitórias. A materialização de `CompositionCore` e
`ExportPipeline` deverá executar esses IDs nos respectivos owners posteriores,
sem transformar o registro em cobertura fictícia. PNG e o raster embutido no
PDF são exatos. JPEG compara estrutura e amostras decodificadas dentro da
tolerância declarada, nunca bytes comprimidos.

Não são oráculos: screenshot do Canvas, hash permanente de JPEG/PDF, tamanho de
arquivo, artefato de Cache ou render de PDF feito por um visualizador. Uma
mudança de encoder que respeite as invariantes não incrementa o schema; mudança
de geometria, amostragem, cor, composição ou raster canônico exige revisão
explícita do contrato e do corpus.

## Reconciliação e ownership posterior

Este contrato resolve as decisões antes adiadas de pipeline final, orientação,
cor, JPEG/PNG/PDF, numeração e captura estável. Permanecem variáveis internas a
biblioteca concreta, otimizações, paralelismo e limites de recurso medidos,
desde que falhem fechados e não alterem o comportamento acima.

As entregas posteriores possuem limites inequívocos:

| Owner | Implementa sem redefinir |
|---|---|
| issue #13 | integração real de caminhos, UNC, mapeados, longos e códigos até a interface |
| issue #35 | materialização do adapter e protocolo do envelope já fechado, plano Q32.32, JPEG/PNG, diálogo final, progresso, preparação e Publicação, sem redefinir o payload |
| issue #37 | adaptador PDF sobre os mesmos rasters e ordem |
| issue #38 | detecção, confirmação e remoção de Saídas órfãs finais pela gramática aceita |
| issue #39 | `BatchRunner`, lease único, revalidação do Projeto, checkpoint e retomada por item |

`BatchRunner` pode reutilizar `plan` e `execute_group`, mas não recompõe,
reabre uma versão diferente do mesmo Original dentro do item nem altera a
transação limitada. A revalidação do Arquivo de Projeto entre itens pertence ao
lote e é separada da captura de Originais desta tentativa.
