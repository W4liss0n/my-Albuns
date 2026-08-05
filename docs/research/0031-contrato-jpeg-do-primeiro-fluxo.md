---
status: current
document: technical-research
ticket: delimitar-contrato-jpeg-fase-2
date: 2026-08-03
updated: 2026-08-03
---

# Contrato JPEG do primeiro fluxo real

> Resultado normativo: a decisão posterior está em [Contrato JPEG do primeiro fluxo](../design/0014-contrato-jpeg-do-primeiro-fluxo.md). Ela incorpora os Backgrounds e Overlays globais do Projeto v1, elimina a exceção provisória de Background apenas branco e registra as restrições de fonte aprovadas; este documento permanece como evidência técnica.

## Pergunta

Qual é o menor contrato tecnicamente defensável para exportar como JPEG a
Lâmina visível do primeiro fluxo da Fase 2, sem antecipar a Exportação completa
nem transformar este gate no ticket do Renderizador final?

## Decisão recomendada

O primeiro fluxo deve produzir **um JPEG/JFIF baseline opaco, RGB de 8 bits em
sRGB, com qualidade fixa 100, densidade física explícita e sem metadados
herdados das fontes**. A saída representa exatamente uma Lâmina escolhida no
estado visível, mas o contrato JPEG começa somente depois que a composição
entregar um raster final, orientado, opaco e em sRGB.

Essa fronteira comprova dimensões físicas, codificação, cor declarada e
interoperabilidade básica. Ela não afirma que o Renderizador final, a tela
completa de Exportação ou o gerenciamento de cor de originais arbitrários
estejam prontos.

## Evidência local

O domínio já usa micrômetros inteiros e o protocolo limita DPI a `1..=1200` em
`crates/myalbuns-imaging-protocol/src/render.rs`, publicado com a branch de plataforma.
O render atual converte cada dimensão com
`round(micrômetros × DPI / 25.400)`, compõe em RGBA e grava somente PNG em
`crates/myalbuns-imaging/src/render.rs`. A evidência da Fase
1 já registrou `60 × 30 cm` a `300 DPI` como `7.087 × 3.543 px` em
`docs/research/0028-comparacao-final-de-topologias.md`.

O decoder atual lê a orientação EXIF da fonte e a aplica fisicamente antes da
composição em `crates/myalbuns-imaging/src/source.rs`. Porém,
o `CompositionPlan` ainda não carrega Background e o render usa o bege fixo
`[239, 232, 218]`. Isso é scaffolding da prova anterior, não pode virar a regra
JPEG. A especificação exige Background obrigatório, branco por padrão
([especificação do produto](../specs/programa-de-diagramacao-de-albuns.md)).

O lockfile fixa `image 0.25.10`. Nessa versão, `JpegEncoder` aceita qualidade
de `1` a `100`, codifica com subsampling `4:2:2` e permite declarar densidade;
sem `set_pixel_density`, não grava DPI
([API oficial de `JpegEncoder`](https://docs.rs/image/0.25.10/image/codecs/jpeg/struct.JpegEncoder.html)).
A mesma versão implementa inserção de perfil ICC e EXIF no encoder JPEG
([API oficial de `ImageEncoder`](https://docs.rs/image/0.25.10/image/trait.ImageEncoder.html)).
Portanto, o contrato mínimo cabe na biblioteca já adotada e não justifica uma
segunda dependência de codec nesta fase.

## Contrato verificável

| Aspecto | Regra desta fase | Verificação obrigatória |
| --- | --- | --- |
| Formato | JPEG/JFIF baseline de três componentes, extensão `.jpg` e MIME `image/jpeg`. | O arquivo abre após a gravação, começa com SOI, tem APP0 `JFIF` imediatamente depois, termina com EOI e usa SOF0. |
| Dimensões | Para cada eixo positivo, calcular com aritmética inteira verificada `px = floor((µm × DPI + 12.700) / 25.400)`; cada resultado deve ficar em `1..=65.535`. | Largura e altura lidas do JPEG coincidem exatamente com o cálculo; overflow ou eixo fora do limite falha antes de alocar/codificar. |
| DPI | Usar o DPI do Projeto já validado em `1..=1200`; gravar X e Y iguais em pontos por polegada. | APP0 tem `units = 1`, `Xdensity = DPI` e `Ydensity = DPI`. |
| Qualidade | Fixar `100` neste fluxo. Não oferecer slider nem persistir preferência. | Teste da fronteira do encoder comprova que `100` foi passado; a integração não usa tamanho do arquivo nem igualdade de pixels como oráculo. |
| Alfa e fundo | O raster entregue ao encoder deve estar totalmente composto sobre o Background resolvido e ter alfa `255`; só então converte para RGB8. Enquanto Background não integrar o snapshot, o caso aceito é o branco canônico. | Falhar antes de codificar se restar qualquer pixel translúcido; o JPEG decodificado possui três canais e regiões transparentes do Overlay aparecem mescladas com o Background esperado. |
| Orientação | Aplicar a orientação da fonte uma vez na decodificação; a saída contém pixels fisicamente em orientação normal, de cima para baixo. Não gravar Orientation EXIF. | Fixture assimétrica com orientação de fonte diferente de `1` aparece na posição esperada e a saída não contém tag Orientation. |
| Espaço de cor | O raster de entrada do encoder é sRGB RGB8 e o arquivo incorpora um perfil sRGB fixo controlado pelo aplicativo. Recomenda-se provisoriamente o perfil v2 `sRGB2014.icc`. Nesta fase, somente fontes sem ICC, assumidas como sRGB, ou com ICC igual a um perfil sRGB da lista permitida fixa são elegíveis; qualquer outro perfil gera erro explícito. | APP2 contém exatamente o perfil controlado; um teste compara seu digest, não uma descrição textual do perfil. Uma fonte fora da lista permitida não chega ao encoder. |
| Outros metadados | Não copiar EXIF, XMP, comentário, thumbnail, data, câmera, GPS ou metadados das fontes. Os únicos metadados deliberados são JFIF/DPI e ICC/sRGB. | Inspeção dos marcadores não encontra APP1, COM nem thumbnail JFIF; APP2 contém somente o ICC esperado. |

O denominador `25.400` decorre da equivalência exata de uma polegada a
`25,4 mm` ([NIST, unidades SI de comprimento](https://www.nist.gov/pml/owm/si-units-length)).
JFIF define `units = 1` como pontos por polegada, usa campos de densidade de
16 bits e fixa orientação top-down. Também restringe o frame a um ou três
componentes de 8 bits, portanto não há canal alfa no contrato adotado, e
recomenda o processo baseline para intercâmbio
([ITU-T T.871 / ISO/IEC 10918-5](https://www.itu.int/rec/dologin_pub.asp?id=T-REC-T.871-201105-I%21%21PDF-E&lang=e&type=items)).
O tipo `PixelDensity` de `image 0.25.10` representa DPI como dois `u16`; o teto
local de 1.200 cabe sem adaptação
([API oficial de `PixelDensity`](https://docs.rs/image/0.25.10/image/codecs/jpeg/struct.PixelDensity.html)).
O encoder pinado usa SOF0 e recusa largura ou altura acima de `65.535`
([fonte oficial de `image 0.25.10`](https://github.com/image-rs/image/blob/v0.25.10/src/codecs/jpeg/encoder.rs)).

O ICC especifica a inserção de perfil em JPEG por APP2, e o próprio ICC publica
perfis sRGB v2 e v4. A escolha do `sRGB2014.icc` v2 aqui é uma hipótese estreita
de interoperabilidade para o primeiro fluxo, não uma conclusão normativa nem
uma decisão sobre todo o pipeline profissional; destinos reais de laboratório
ainda precisam ser validados
([inserção de perfis em JPEG](https://www.color.org/profile_embedding/),
[perfis sRGB oficiais](https://registry.color.org/rgb-registry/srgbprofiles)).
Incorporar o perfil apenas declara como interpretar os pixels; não converte uma
fonte Adobe RGB, Display P3 ou CMYK para sRGB.

## Casos dourados mínimos

### 1. Metadados e dimensão exata

Uma Lâmina de `25.400 × 25.400 µm`, `300 DPI`, Background branco e quatro áreas
sRGB opacas gera `300 × 300 px`. O oráculo exige JFIF, densidade `300 × 300` em
polegadas, ICC sRGB esperado, ausência dos demais metadados e decodificação
válida.

### 2. Dimensão de produção e arredondamento

Uma Lâmina de `600.000 × 300.000 µm` a `300 DPI` gera exatamente
`7.087 × 3.543 px`. Esse caso de integração preserva a medida já observada na
Fase 1 e impede regressão silenciosa da regra de arredondamento. Ele não precisa
ser executado em toda suíte unitária rápida.

### 3. Orientação e transparência resolvidas

Uma fonte pequena com quadrantes assimétricos e Orientation EXIF diferente de
`1`, coberta por um Overlay semitransparente, é composta sobre Background
branco. Após decodificar o JPEG, cada quadrante deve permanecer no lado
esperado e amostras coletadas no interior das áreas devem ficar dentro de uma
tolerância de canal congelada pelo fixture. Não se compara pixel de borda,
porque o encoder atual usa `4:2:2`.

### Oráculo, não hash do JPEG

JPEG é uma saída com perdas, e `quality = 100` significa a melhor qualidade do
encoder, não ausência de perda. Os testes devem ser exatos para estrutura,
dimensões, densidade, perfil, ausência de metadados e raster **antes** da
codificação; depois da decodificação, devem usar regiões internas e tolerâncias
congeladas. Duas codificações na mesma build pinada devem produzir o mesmo
SHA-256, descartando não determinismo acidental. O hash continua útil para
verificar o arquivo que acabou de ser gravado, como já faz `RenderCompletion`,
mas não deve ser o golden permanente entre atualizações deliberadas do codec.

## Limite de cor desta fase

O próprio `image 0.25.10` registra como limitação que a informação de espaço de
cor dos pixels não é comunicada de forma clara
([notas oficiais da versão](https://docs.rs/crate/image/0.25.10/source/CHANGES.md)).
Assim, o gate deve usar fontes canônicas já conhecidas como sRGB. No fluxo
exposto, a lista permitida compara o digest do ICC incorporado com os perfis sRGB
controlados pelo aplicativo; perfil diferente falha como não suportado nesta
fase. A fonte não pode ser apresentada como colorimetricamente correta só
porque a saída recebeu um perfil sRGB. A conversão de perfis pertence ao
Renderizador final.

## O que fica explicitamente adiado

- slider e mapeamento de Qualidade JPEG, preferência por operação e regra do lote;
- escolha definitiva de encoder, `4:4:4`, JPEG progressivo, tabelas de
  quantização, otimização Huffman e política de upgrades do codec;
- conversão ICC de originais, wide gamut, CMYK, soft proof e perfil de gráfica;
- preservação de EXIF/XMP, thumbnails ou autoria de metadados do aplicativo;
- PNG, PDF, modo Por página, intervalos, Álbum inteiro e Exportação em lote;
- nomes finais, conflitos múltiplos, órfãos e publicação de conjuntos;
- Backgrounds e Overlays completos, Pilha visual final, Sangria, divisões de
  Página e equivalência integral Canvas/Exportação;
- estabilidade byte a byte entre versões do encoder e golden JPEG grande no
  repositório;
- orçamentos finais de memória, paralelismo, desempenho e limites de produto
  adicionais ao teto estrutural de `65.535 px` por eixo.

## Gate de aceitação proposto

O ticket pode ser considerado resolvido quando um teste ponta a ponta cria ou
abre o Projeto do primeiro fluxo, altera seu DPI, exporta a Lâmina visível e
reabre o `.jpg`, comprovando os oito campos exatos da tabela e os três casos
dourados acima. O gate deve declarar a restrição sRGB das fontes usadas e não
deve exigir nenhuma das capacidades adiadas.

O `4:2:2` fixo do encoder atual é aceito como limitação conhecida apenas deste
gate. A T.871 desencoraja subsampling JFIF diferente de `4:2:0` porque alguns
leitores podem não aceitá-lo; portanto esta pesquisa não atribui à saída
“compatibilidade máxima” nem “qualidade profissional final” antes de testes com
os destinos reais.
