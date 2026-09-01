---
status: accepted
document: design
date: 2026-08-03
updated: 2026-09-01
---

# Contrato JPEG do primeiro fluxo

## Objetivo

Fixar o primeiro corte real e verificável de `Exportar Lâmina`: uma única saída JPEG da Lâmina selecionada, produzida a partir do estado visível e do mesmo plano de composição consumido pelo Canvas. Este design torna normativa a parte aprovada da [pesquisa técnica](../research/0031-contrato-jpeg-do-primeiro-fluxo.md), sem declarar concluídos o [diálogo final de Exportação](0004-exportacao-normal.md), o Renderizador final ou o gerenciamento profissional de cor.

O [Contrato do Renderizador final](0019-contrato-do-renderizador-final.md) é a
decisão posterior para formatos, TIFF, qualidade, subsampling, captura de
Originais, múltiplas unidades e Publicação. Este documento continua descrevendo
o corte JPEG já entregue; seu payload transitório de unidade única é substituído
pelo envelope versionado que transporta o `RenderSnapshot` inteiro no contrato
final.

## Operação visível

`Exportar Lâmina` já é uma funcionalidade real, não uma prova ou artefato interno. Neste corte:

- o alvo é exatamente a Lâmina selecionada que originou o comando;
- a operação congela um snapshot do estado atual, inclusive mudanças ainda não salvas, e nunca salva o Projeto como efeito colateral;
- a interface obtém um único pathname final explícito, filtrado e concluído com `.jpg`; o nome sugerido segue `{Nome do Projeto}_{posição da Lâmina com largura mínima de três dígitos}.jpg`;
- cancelar a escolha do destino não cria tentativa, temporário, Histórico ou mudança pendente;
- um destino já existente exige confirmação explícita antes da tentativa e nunca é renomeado silenciosamente;
- o plano leva até o `Publisher` a autorização imutável `CreateOnly` quando o nome estava livre ou `ReplaceConfirmed` quando a substituição foi confirmada; a autorização não é inferida novamente no fim da operação;
- se um arquivo surgir no destino antes da promoção de um plano `CreateOnly`, a Publicação termina em conflito e não o substitui;
- a preparação e a publicação do arquivo confirmado continuam seguindo as garantias do `ExportPipeline` e do [ADR 0006](../adr/0006-publicar-exportacao-com-transacao-limitada.md).

O mecanismo visual usado para obter esse pathname não integra o contrato JPEG. Esse corte transitório não oferece o modal completo, Álbum inteiro, intervalo, modo Por página ou escolha de formato. A experiência final definida em [Exportação normal](0004-exportacao-normal.md) permanece normativa para a entrega completa.

## Unidade de composição

Uma tentativa contém exatamente uma unidade final e imutável. Ao congelar a revisão visível, `ProjectSession` usa o `CompositionCore` uma única vez e produz um `RenderSnapshot` que já contém o `CompositionPlan` compartilhado com o Canvas. O `ExportPipeline` não recompõe esse estado: valida o snapshot, escolhe a Lâmina alvo e extrai dele uma `ComposedOutputUnit` autocontida. Essa unidade é a carga criativa da IPC.

O envelope entregue ao Processador contém a `ComposedOutputUnit`, o DPI congelado, o destino de preparação, os descritores exatos das fontes e o `RootBindingPlan`. `requestId`, `projectId` e Revisão acompanham o job apenas como contexto opaco de correlação e logging. O Processador não recebe nem interpreta `.myalbuns`, estado mutável, Cache, o Álbum inteiro ou outro `CompositionPlan`, e não invoca novamente o `CompositionCore`.

O `CompositionCore` resolve antes da fronteira de processo:

- a superfície física efetiva;
- os lados ativos e a origem normalizada da unidade;
- os escopos `bothSides` e `perSide` de Background e Overlay;
- os retângulos de desenho e recorte;
- a ordem final da Pilha visual;
- o conjunto exato de Identidades de mídia referenciadas.

Uma Lâmina com `activeSides: "both"` usa toda a largura. Uma extremidade `left` ou `right` gera somente a Página ativa, com metade da largura e origem normalizada em `0`; a metade inativa não produz pixels, espaço vazio ou linha divisória. Uma aplicação `bothSides` é reajustada à Página ativa, enquanto uma aplicação `perSide` usa apenas o valor do lado ativo.

No schema v1, a Pilha visual contém o Background obrigatório e o Overlay opcional; não existem Frames ou Fotos persistidos. Imagens de Background e Overlay são esticadas nos dois eixos até o retângulo resolvido, conforme a regra do produto. Quando uma imagem de Background possui alfa, ela é primeiro composta sobre branco canônico `#FFFFFF`; o Overlay continua preservando seu alfa até a composição sobre as camadas inferiores. Essa base branca aparece explicitamente na unidade composta — não é um fallback reimplementado separadamente pelos adaptadores de Canvas e JPEG. Assim, o Background final é sempre opaco e transparência nunca fica indefinida no JPEG. O conjunto de fontes entregue ao Processador precisa ser exatamente o conjunto referenciado pelo plano, sem ausências, extras ou duplicatas; ele pode ser vazio.

O Canvas adapta o mesmo `CompositionPlan` à cena interativa. Seleção, hover, guias, regra dos terços, máscara de segurança, zoom e deslocamento do viewport pertencem a camadas transitórias separadas e nunca entram no plano final. Equivalência entre Canvas e JPEG significa mesma geometria, escopo, recorte, ordem e conteúdo; não significa igualdade pixel a pixel entre WebGL2 e o rasterizador Rust.

## Dimensões raster

Para cada eixo físico positivo, a única conversão autorizada é:

```text
pixels = floor((micrômetros × DPI + 12.700) / 25.400)
```

O cálculo usa aritmética inteira verificada, sem ponto flutuante, e equivale ao arredondamento para o inteiro mais próximo em valores positivos. A multiplicação, a soma e a conversão ao tipo final precisam falhar de forma fechada diante de overflow. Cada eixo precisa resultar em `1..=65.535` antes de qualquer alocação ou decodificação.

O DPI vem do snapshot e precisa coincidir com a Resolução do Projeto no momento congelado. Para Lâmina dupla, a fórmula usa `sheetWidthUm` e `sheetHeightUm`; para Página única, usa `sheetWidthUm / 2` e a mesma altura. A Dimensão já inclui a Sangria interna definida pelo Projeto; guias de Sangria, corte e segurança não são desenhadas.

Retângulos internos são derivados de arestas físicas, nunca por divisão posterior do número de pixels:

```text
rasterEdge(xUm) = floor((xUm × DPI + 12.700) / 25.400)
retângulo [inícioUm, fimUm) = [rasterEdge(inícioUm), rasterEdge(fimUm))
```

Os intervalos são semiabertos. Assim, uma Lâmina dupla de `600.000 µm` a `300 DPI` possui largura `7.087 px`; sua aresta central fica em `3.543`, o lado esquerdo ocupa `[0, 3.543)` e o direito `[3.543, 7.087)`. O pixel excedente pertence ao lado direito, sem lacuna ou sobreposição. Já uma Lâmina de página única é rasterizada como superfície independente de `300.000 µm`, com largura `3.543 px`, tanto para `left` quanto para `right`; ela não é um recorte do raster duplo.

### Guardrail provisório de recursos

O teto estrutural por eixo não é uma proteção suficiente contra falta de memória. Antes de ler pixels ou alocar o raster, o Processador inspeciona os cabeçalhos com aritmética verificada e aplica dois limites independentes nesta versão:

```text
MAX_OUTPUT_PIXELS = 134_217_728
MAX_DECODED_SOURCE_PIXELS_TOTAL = 134_217_728
```

O segundo valor soma todas as fontes únicas referenciadas pela unidade, já considerando suas dimensões orientadas. Cada grupo corresponde a no máximo `512 MiB` de RGBA8; juntos, limitam os dois buffers principais a aproximadamente `1 GiB`. A implementação não pode ler o arquivo inteiro em um buffer sem limite antes desse preflight, e toda alocação posterior continua verificada e falível. Exceder um teto, detectar overflow ou não conseguir reservar memória produz `ResourceLimitExceeded`, nunca panic nem queda deliberadamente aceita do Processador.

Esse guardrail cobre uma Lâmina de `60 × 30 cm` em `300` e `600 DPI`, além de quatro fontes de `24 MP` do corpus real medido. Na mesma dimensão, `1.200 DPI` excede o teto da saída; o maior DPI aceito por este corte é `693`. O Projeto e seu DPI continuam válidos e podem ser salvos e reabertos: somente a tentativa JPEG incompatível falha de forma clara. O número é uma proteção transitória da implementação monolítica, não uma meta final de desempenho ou limite definitivo do produto.

## Fontes e política de cor

Exportação sempre reabre os Arquivos vinculados originais. Cache, thumbnail ou representação reduzida nunca servem como fonte nem como fallback. Cada descritor fixa Identidade de mídia e pathname nativo. Todo pathname autoritativo da IPC — fontes, preparação e bindings de raiz — usa o `NativePathDto` reversível `windowsUtf16`, sem string de fallback ou normalização; `PathBuf` serializado como string não integra a fronteira. O DTO da IPC tem seu próprio versionamento e compartilha somente o codec de baixo nível com o Arquivo de Projeto, não os tipos de `ProjectDocumentV1`. O Processador abre e valida o objeto autoritativo dentro da tentativa e conserva a mesma abertura durante a decodificação; este contrato não exige hash completo prévio, segunda leitura nem define o fingerprint do Cache.

Neste corte, o conteúdo detectado precisa ser JPEG ou PNG estático. TIFF/TIF e qualquer outro formato recebem `UnsupportedSourceFormat`; a extensão isolada não decide o codec. Antes de qualquer conversão genérica para RGBA8, um preflight específico do formato lê dimensões, variante, modelo de cor, profundidade, perfil e orientação. Isso impede que um decoder normalize silenciosamente CMYK/YCCK para RGB antes da validação. Não se cria registro genérico de codecs: uma representação interna fechada distingue somente JPEG e PNG.

A matriz de entrada é:

| Fonte detectada | Regra |
|---|---|
| JPEG 8-bit RGB ou YCbCr, baseline ou progressivo | aceita e normalizada para RGB8 |
| JPEG 8-bit em tons de cinza | aceita; luminância é replicada nos três canais |
| JPEG CMYK, YCCK, multibanda ou outro modelo de componentes | `UnsupportedColorModel` antes da conversão |
| PNG RGB/RGBA 8-bit | aceita |
| PNG indexado de 1, 2, 4 ou 8 bits, inclusive `tRNS` | aceito e expandido para RGBA8 |
| PNG em tons de cinza, com ou sem alfa, nas profundidades válidas de 1 a 16 bits | aceito e expandido para RGBA8 |
| PNG RGB/RGBA 16-bit | aceito como fonte e reduzido deterministicamente para RGBA8 |
| APNG | `UnsupportedSourceVariant`; nenhum frame é escolhido silenciosamente |

Na redução de 16 para 8 bits, cada canal inteiro usa `floor((valor16 + 128) / 257)`. Isso fixa a entrada do compositor, mas não promete um pipeline interno profissional de 16 bits. A orientação EXIF de um JPEG é aplicada fisicamente uma única vez durante a decodificação, e a saída não recebe tag Orientation.

A política temporária de cor é estrita:

- qualquer modelo aceito sem perfil ICC é assumido como sRGB após a normalização definida acima;
- fonte com perfil ICC só é aceita quando os bytes correspondem a uma entrada da allowlist controlada;
- a allowlist inicial contém exatamente os três perfis sRGB atuais distribuídos pelo ICC: `sRGB2014.icc` v2 (`3.024` bytes, SHA-256 `384b832de3412066743b52a75ee906b6fb9fb8d9e09e936fc2c43223815c6e0a`), `sRGB_v4_ICC_preference.icc` (`60.960` bytes, SHA-256 `83174717332326ddc198d9df188a4daec27b8979ba152cebbfc470c793d0bb11`) e `sRGB_v4_ICC_preference_displayclass.icc` (`60.988` bytes, SHA-256 `f54b145a18e4b12112750e672f1c79cac9347dc8403da3955e7f74a352816a21`);
- Adobe RGB, Display P3, perfil desconhecido, perfil malformado ou combinação contraditória de declarações de cor recebem `UnsupportedColorProfile`;
- CMYK, YCCK e demais modelos recusados permanecem `UnsupportedColorModel`, tenham ou não ICC;
- acrescentar outra entrada comprovadamente sRGB exige fixture e digest explícitos, mas não altera o Arquivo de Projeto.

Os três perfis da allowlist são incorporados ao aplicativo sem alteração, conforme os [termos e a distribuição publicados pelo ICC](https://registry.color.org/rgb-registry/srgbprofiles). A saída sempre incorpora `sRGB2014.icc` v2; os dois perfis v4 servem apenas para reconhecer entradas sRGB. Esta fase não executa transformação ICC: os pixels aceitos já são tratados como sRGB. O raster mantém alfa durante a composição de Overlay e termina obrigatoriamente em sRGB RGBA8 com alfa `255` em todos os pixels; alfa residual recebe `CompositionFailed` antes do encoder.

## Arquivo JPEG

A saída possui o contrato observável abaixo:

| Aspecto | Regra |
|---|---|
| pathname e mídia | extensão `.jpg` e MIME `image/jpeg` |
| processo | JPEG baseline, marcador SOF0 e três componentes |
| raster | RGB de 8 bits, opaco, orientado de cima para baixo |
| qualidade | parâmetro fixo `100`, sem controle ou preferência persistida |
| densidade | APP0 JFIF imediatamente após SOI, `units = 1` e X/Y iguais ao DPI do Projeto |
| cor | APP2 contém exatamente o `sRGB2014.icc` controlado, reconstituível pelo digest aprovado |
| metadados | nenhum EXIF, XMP, comentário, GPS, data, thumbnail ou metadado herdado da fonte |
| término | EOI é o último marcador no fim do arquivo, que é reaberto e inspecionado com sucesso antes da publicação |

O encoder atual produz subsampling `4:2:2`. Ele é aceito como limitação conhecida deste corte, mas não integra o contrato permanente: não se promete compatibilidade máxima nem qualidade profissional final, e uma troca futura para `4:4:4` ou outro encoder não exige migração do Projeto. O teste da fronteira comprova que qualidade `100` foi solicitada; subsampling, tabelas de quantização, tamanho e bytes comprimidos não são golden público.

O SHA-256 do arquivo recém-gravado continua útil como evidência daquela tentativa e pode integrar o resultado operacional. Ele não é um golden estável entre atualizações deliberadas do codec.

## Fronteiras de responsabilidade

| Componente | Responsabilidade neste fluxo | Não faz |
|---|---|---|
| `ProjectSession` | congela estado e revisão visíveis, compondo o snapshot uma vez | não salva automaticamente nem exporta pixels |
| `CompositionCore` | resolve o `CompositionPlan` compartilhado no momento do snapshot | não abre arquivos, não decodifica e não conhece JPEG |
| Canvas | adapta o plano à cena e sobrepõe feedback transitório | não define regra paralela de composição |
| `ExportPipeline` | extrai a unidade já composta; escolhe destino, dependências, bindings, política de substituição, preparação e publicação | não recompõe o snapshot, não interpreta pixels nem usa Cache como fonte |
| Processador | valida o job, reabre originais, normaliza orientação/cor, rasteriza, codifica e verifica a preparação | não lê Projeto, não escolhe nome final e não publica |

A mudança de PNG demonstrativo para este contrato altera a semântica da IPC e exige incremento de sua versão antes da integração. O job é específico para uma unidade JPEG; não se cria registro genérico de codecs, enumeração prematura de todos os formatos ou estratégia pública para um único encoder.

O Processador só informa sucesso depois de sincronizar, reabrir e verificar o arquivo preparado. Em produção, uma única leitura leve de marcadores confirma estrutura JPEG/JFIF, SOF0, dimensões, DPI, perfil, ausência dos metadados proibidos e EOI exatamente no fim, enquanto calcula tamanho e SHA-256; não se faz uma segunda decodificação integral de um JPEG produzido por encoder controlado. O host confirma independentemente, pelo handle autorizado, que a preparação continua sendo arquivo regular e que tamanho e digest coincidem com o terminal recebido. A suíte de testes conserva a decodificação completa e os oráculos visuais. Somente depois das duas verificações o `Publisher` pode promover o arquivo ao pathname final.

O stream produz exatamente um terminal estruturado correlacionado: `Completed` ou `Failed`. Falhas determinísticas usam a forma equivalente a `ImagingEvent::Failed { requestId, code, mediaId?, pathCode? }`. `mediaId` identifica a fonte quando aplicável; nenhum pathname ou texto livre do sistema operacional cruza como detalhe público. O protocolo é o único proprietário dos códigos do Processador; `ExportPipeline` faz uma única tradução para seu estágio operacional e o adaptador de interface localiza a mensagem. `Cancelled`, `ExportConflict` e `PublicationFailed` pertencem ao pipeline do host, não ao Processador. Exit code, ausência de terminal, terminal duplicado, resposta malformada e stderr permanecem falhas de transporte ou protocolo, não o transporte normal das falhas conhecidas.

## Falhas tipadas

Nenhuma tentativa altera Projeto, Revisão, Histórico ou Cache. Falha ou cancelamento antes de começar a Publicação preserva o arquivo final anterior; depois que o `Publisher` assume a tentativa, um pedido de cancelamento retorna `TooLate`, não interrompe a promoção e vale o envelope limitado do ADR 0006. Uma falha ou queda nesse trecho nunca é anunciada como sucesso, mas pode deixar no pathname final o arquivo anterior ou o candidato já promovido. A fronteira preserva no mínimo:

| Situação | Resultado |
|---|---|
| alvo, DPI, dimensão, plano ou conjunto de fontes inconsistente | `InvalidRenderRequest` |
| original ausente, indisponível, sem acesso ou de tipo inesperado | categoria correspondente da política central de caminhos |
| conteúdo diferente de JPEG/PNG, inclusive TIFF | `UnsupportedSourceFormat` |
| JPEG/PNG animado ou variante não aceita | `UnsupportedSourceVariant` |
| CMYK, YCCK, multibanda ou outro modelo recusado | `UnsupportedColorModel` |
| ICC não permitido, malformado ou contraditório | `UnsupportedColorProfile` |
| arquivo permitido que não pode ser decodificado | `DecodeFailed` |
| composição inválida ou alfa residual | `CompositionFailed` |
| saída, soma das fontes, aritmética ou reserva de memória excede o guardrail | `ResourceLimitExceeded` |
| falha do encoder ou da sincronização da preparação | `EncodeFailed` |
| JPEG preparado truncado ou divergente do contrato | `VerificationFailed` |
| cancelamento observado antes da publicação | `Cancelled` |
| destino surge sob política `CreateOnly` | `ExportConflict` |
| falha durante a promoção | `PublicationFailed`, com o estado final limitado informado como anterior, candidato ou inconclusivo |

Mensagens localizadas pertencem ao adaptador de interface; os códigos estáveis atravessam a fronteira sem expor erros livres do codec como contrato público.

## Casos dourados obrigatórios

### Projeto neutro real

O fixture v1 neutro, sem mídia, com `600.000 × 300.000 µm` a `300 DPI`, gera `7.087 × 3.543 px` de branco opaco. O job contém zero fontes. O arquivo comprova JFIF, DPI, perfil, SOF0, ausência de metadados proibidos e decodificação válida.

### Plano compartilhado compacto

Uma Lâmina dupla de `25.400 × 25.400 µm` a `300 DPI` gera `300 × 300 px`, com Background `perSide` de cores sólidas distintas. Canvas recebe o `CompositionPlan`; o Processador recebe exatamente o valor serializável da unidade selecionada desse plano, sem outra Lâmina ou recomposição. Os testes comparam a unidade nos dois lados da fronteira e o raster sólido anterior ao encoder, além de verificar regiões internas do JPEG com tolerância congelada.

### Divisão de largura ímpar

Uma Lâmina dupla de `600.000 × 300.000 µm` a `300 DPI` gera `7.087 × 3.543 px`. Um Background `perSide` comprova os intervalos `[0, 3.543)` e `[3.543, 7.087)`, sem pixel ausente ou sobreposto. Os casos de Página única `left` e `right` geram independentemente `3.543 × 3.543 px` e normalizam a origem.

### Orientação e transparência

Um JPEG pequeno assimétrico com Orientation EXIF diferente de `1` é usado como Background e um PNG sRGB semitransparente como Overlay. O caso comprova orientação aplicada uma vez, esticamento nos escopos resolvidos, composição de alfa, saída opaca, perfil controlado e ausência de APP1. Amostras após decodificação usam regiões internas; bordas não são oráculo por causa do `4:2:2`.

Um fixture complementar usa PNG semitransparente como Background e comprova que sua transparência revela exatamente o branco canônico, enquanto o mesmo alfa usado como Overlay revela a camada inferior. Nenhum dos dois casos deixa alfa residual no raster final.

### Matriz de fontes

Fixtures pequenas cobrem JPEG YCbCr, RGB, tons de cinza, progressivo, CMYK e YCCK; PNG RGB8, RGBA8, indexado com `tRNS`, tons de cinza e RGBA16; ICC ausente, permitido, desconhecido e malformado; e APNG. Os testes comprovam que modelos e variantes recusados falham no preflight, antes da normalização para RGBA8 e antes de gravar qualquer arquivo preparado; um diretório de tentativa eventualmente criado é limpo pelo ciclo normal. Um fixture de 16 bits congela a redução inteira por canal.

### Página única

Uma matriz cobre `activeSides: "left"` e `"right"`: cada saída usa metade da largura física, mantém a altura, normaliza sua origem e não contém metade vazia, linha central ou conteúdo do lado inativo.

### Estado visível real

Um teste ponta a ponta abre um `.myalbuns`, altera o DPI sem salvar, seleciona uma Lâmina diferente da inicial e exporta essa revisão. O JPEG comprova alvo, dimensões e DPI do estado visível; a Sessão continua com mudanças pendentes e os bytes do Arquivo de Projeto permanecem inalterados. Cancelar o seletor de destino não cria tentativa nem diretório de preparação.

### Ciclo de vida e Publicação

Uma matriz de falhas comprova: cancelamento antes da tentativa sem artefato; cancelamento ou falha de preparação preservando o final anterior; `ReplaceConfirmed` promovendo o candidato; arquivo concorrente recusado sob `CreateOnly`; cancelamento após o claim do `Publisher` respondido como `TooLate`; e falha ou queda durante a Publicação restrita ao envelope anterior/candidato, com estado inconclusivo comunicado e sem falso sucesso.

### IPC nativa e terminais

Casos de round-trip atravessam host e Processador com caminhos locais, UNC, mapeados, verbatim local/UNC e uma unidade UTF-16 não pareada em fonte, preparação e bindings. Outros casos comprovam exatamente um terminal `Completed` ou `Failed`, preservação de `code`, `mediaId` e `pathCode` até o adaptador, e classificação de terminal ausente, duplicado, malformado ou não correlacionado como falha de transporte/protocolo.

### Casos negativos

Testes recusam antes da publicação: eixo zero, maior que `65.535` ou com overflow; saída acima de `MAX_OUTPUT_PIXELS`; soma das fontes acima de `MAX_DECODED_SOURCE_PIXELS_TOTAL`; reserva de memória recusada; fonte extra, duplicada ou ausente; TIFF; APNG; perfil não permitido; CMYK/YCCK; alfa residual; JPEG truncado; marcador, DPI, dimensão, ICC ou bytes posteriores ao EOI divergentes.

Não se usa screenshot do Canvas como golden de pixels, JPEG grande versionado no repositório, hash comprimido permanente ou tamanho do arquivo como oráculo de qualidade.

## Scaffolding que precisa desaparecer

O render atual da Fase 1 ainda rejeita Lâmina sem mídia, calcula dimensões com ponto flutuante, injeta fundo bege, desenha linha central, placeholders e bordas demonstrativas e grava PNG. Esses comportamentos são evidência do spike, não compatibilidade. A implementação deste contrato precisa:

- permitir zero fontes quando o plano não referencia mídia;
- incorporar Background, Overlay e lados ativos ao plano;
- centralizar a fórmula inteira de dimensões no núcleo compartilhado;
- fazer preflight de dimensões e recursos antes de decodificar ou alocar;
- remover todos os adornos demonstrativos do raster final;
- gerar e verificar JPEG conforme este documento.

## Adiado explicitamente

- modal completo, Álbum inteiro, intervalos, modo Por página e múltiplas unidades;
- slider e mapeamento de Qualidade JPEG;
- PNG, PDF, lote, namespace completo, conflitos de conjunto e limpeza de órfãos;
- TIFF/TIF;
- APNG e qualquer tratamento de animação;
- conversão ICC, Adobe RGB, Display P3, CMYK, wide gamut, soft proof e perfil de laboratório;
- decisão definitiva de encoder, `4:4:4`, JPEG progressivo, quantização e Huffman;
- Frames, Fotos, aplicações locais e Pilha visual posterior ao schema v1;
- metas finais de desempenho, tiles, paralelismo e orçamento definitivo de memória além do guardrail provisório deste corte;
- estabilidade byte a byte entre versões do codec.
