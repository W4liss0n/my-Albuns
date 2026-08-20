---
status: current
document: technical-research
ticket: 44-programa-03a-representacao-reduzida-e-pausa-causal-do-cache
date: 2026-08-10
updated: 2026-08-12
---

# Representação reduzida e política de decode

## Pergunta

O Programa 03A exige que formato, resolução, limites de decode, tratamento de
orientação e cor, fingerprint e eventual adoção de tiles sejam decididos por
medição reproduzível. Este spike fixa essa política antes de o contrato público
do Cache ser ampliado.

Ele combina três classes de evidência:

- um corpus sintético determinístico, versionado como teste ignorado e executado
  em perfil `release` para cobrir os formatos e metadados de borda;
- três recortes fotográficos da fixture histórica versionada
  `crates/myalbuns-imaging/tests/assets/photographic-quality-corpus.png`,
  normalizados para `1.600 px`, para
  comparar taxa e fidelidade entre qualidades JPEG sem depender de arquivos
  externos ou de caminhos locais;
- as medições reais já registradas em
  `0008-cache-com-imagens-reais.md`,
  `0011-cenario-cache-e-virtualizacao.md`,
  `0012-navegacao-em-album-longo.md`,
  `0013-baseline-de-cache-com-decorativo.md` e
  `0014-webgl2-limites-e-pressao-grafica.md`.

O corpus sintético e os recortes fotográficos não substituem as 172 Fotos reais
e o Decorativo real dessas rodadas. Eles tornam reproduzíveis as decisões que o
corpus histórico não cobria: TIFF, TIFF multipágina, orientação, perfil sRGB
explícito e a qualidade do JPEG derivado.

## Instrumento e versões

O instrumento é
`crates/myalbuns-imaging/tests/media_cache_spike.rs`. Ele só escreve no
diretório temporário do runner e é reproduzido por:

```powershell
cargo test -p myalbuns-imaging --release --test media_cache_spike measured_media_cache_policy -- --ignored --nocapture
```

A execução canônica de 10 de agosto de 2026 usou:

- Windows 11 Pro 10.0.26200, x86-64;
- Intel Core i5-13450HX, 10 cores e 16 processadores lógicos;
- Rust/Cargo 1.97.1, target `x86_64-pc-windows-msvc`;
- `image` 0.25.10 com `jpeg`, `png` e `tiff`;
- `tiff` 0.11.3;
- `png` 0.18.1 e `zune-jpeg` 0.5.15, resolvidos pelo lockfile.

As decisões que dependem das bibliotecas foram conferidas nas fontes primárias:

- [`image::Limits` 0.25.10](https://docs.rs/image/0.25.10/image/struct.Limits.html)
  define largura/altura como limites estritos e 512 MiB como limite padrão de
  alocações, que alguns decoders podem tratar como não estrito;
- [`ImageReader::limits`](https://docs.rs/image/0.25.10/image/struct.ImageReader.html#method.limits)
  aplica os limites ao decoder escolhido;
- [`ImageDecoder::orientation`](https://docs.rs/image/0.25.10/image/trait.ImageDecoder.html#method.orientation)
  e [`DynamicImage::apply_orientation`](https://docs.rs/image/0.25.10/image/enum.DynamicImage.html#method.apply_orientation)
  fornecem a normalização de EXIF/TIFF sem interpretação própria;
- [`ImageDecoder::icc_profile`](https://docs.rs/image/0.25.10/image/trait.ImageDecoder.html#method.icc_profile)
  expõe o perfil incorporado;
- [`ImageEncoder::set_icc_profile`](https://docs.rs/image/0.25.10/image/trait.ImageEncoder.html#method.set_icc_profile)
  permite publicar o perfil sRGB aceito no artefato;
- [`tiff::decoder::Decoder`](https://docs.rs/tiff/0.11.3/tiff/decoder/struct.Decoder.html)
  expõe `more_images` e `next_image`, usados para contar diretórios antes de
  aceitar o Original.

## Corpus determinístico e resultado medido

| Fonte | Massa | Resultado reduzido | Tempo relevante |
| --- | ---: | ---: | ---: |
| JPEG opaco | 6000 × 4000 px; 45.562.932 B | JPEG 1600 × 1067 px; 1.161.958 B | SHA-256 27 ms; decode 484 ms; resize 107 ms; encode 59 ms |
| PNG com alfa | 2400 × 1800 px; 4.875.661 B | PNG 1600 × 1200 px; 2.302.842 B | decode 47 ms; encode 12 ms |
| TIFF RGB de uma página | 4096 × 3072 px; 37.749.250 B | decode validado | 24 ms |
| TIFF de duas páginas | 2 páginas de 8 × 8 px | recusado pela política | contagem validada |
| JPEG com EXIF 6 | 8 × 4 px | 4 × 8 px após uma aplicação | orientação validada |
| PNG com chunk eXIf 6 | 2 × 1 px | 2 × 1 px, sem rotação implícita | metadado lido e geometria preservada |

O fingerprint integral do JPEG foi
`0baeea4b9beb00e8b62e2a31a18fe6d62fe50461de159051aa8da9ea4cf35bc8`
e foi recalculado duas vezes pelo teste. O ensaio completo, excluído o build
frio, levou 3.352 ms na rodada de 12 de agosto; 1.028 ms pertenceram ao sweep
JPEG.

Os artefatos JPEG e PNG foram reabertos pelo decoder e continham exatamente o
perfil `sRGB2014.icc` versionado no Processador. A variante com alfa permaneceu
RGBA. O JPEG com EXIF 6 foi orientado uma vez; o PNG com eXIf 6 expôs o
metadado, mas manteve 2 × 1 px porque PNG não recebe rotação implícita.

### Sweep fotográfico de qualidade JPEG

A fonte do sweep é a fixture técnica histórica
`crates/myalbuns-imaging/tests/assets/photographic-quality-corpus.png`, SHA-256
`3c42f4e7833de4d0bb280091583d1075f059b9f005cfbb8dc30ae62d03a1e52b`.
Ela é preservada apenas para tornar esta medição reproduzível e não constitui
uma referência visual vigente da interface.
O instrumento recorta três fotografias distintas nas caixas
`[x, y, largura, altura]` `grupo-familiar [0, 140, 325, 465]`,
`retrato-triplo [390, 140, 350, 465]` e
`retrato-individual [770, 140, 350, 465]`. Cada recorte é normalizado com
Lanczos3 para `1.600 px` no maior lado antes de qualquer codificação candidata.
Os SHA-256 dos rasters RGB normalizados são, respectivamente,
`0000fc047a57a007e87a584f37efdafad0d919011612dc5ff51cdd43c1f5c6f3`,
`635cf1b7d16fcb758dbd8fe69ebe899e81d3a83fbebf696de3bab06b40fbb4b7` e
`33c301506544a6a62f4e2b595c7cc4a89830e12b5b3e2c24af5be609057dcc5c`.

O mesmo encoder e ICC do Processador foram exercitados em todos os pontos. A
fidelidade compara, canal a canal, cada JPEG reaberto com o raster RGB que o
originou. PSNR maior e erro absoluto médio menor significam maior fidelidade.

| Qualidade | Massa total (3 Fotos) | Massa média | PSNR agregado | Erro absoluto médio |
| ---: | ---: | ---: | ---: | ---: |
| 72 | 644.128 B | 214.709 B | 43,186 dB | 1,265 |
| 76 | 690.905 B | 230.301 B | 43,960 dB | 1,134 |
| 80 | 767.433 B | 255.811 B | 44,752 dB | 1,034 |
| **84** | **866.283 B** | **288.761 B** | **45,734 dB** | **0,908** |
| 88 | 999.493 B | 333.164 B | 46,877 dB | 0,781 |
| 92 | 1.185.145 B | 395.048 B | 48,073 dB | 0,662 |

A escolha não fixa 84 antes da medição. O instrumento normaliza massa e PSNR
entre os pontos extremos e escolhe o ponto de maior distância acima da reta que
os liga, isto é, o joelho da curva taxa–distorção. Esse método selecionou 84:
até esse ponto, os ganhos de fidelidade compensam melhor o crescimento dos
artefatos; depois dele, 88 e 92 continuam ganhando fidelidade, mas com aumento
proporcionalmente maior de massa. O teste falha se uma mudança de corpus, codec
ou política deslocar o joelho sem que a decisão publicada seja revista.

## Decisões

### Fontes aceitas

O Cache aceita JPEG, PNG e TIFF de uma única página. Um TIFF com mais de um IFD
é recusado como variante não suportada: `MediaRef` não possui seletor de página
e escolher silenciosamente a primeira página criaria uma interpretação não
persistida. TIFF não ganha um caso especial no domínio criativo; a decisão fica
no adaptador do Processador.

### Limites de decode

O teto estrito comum permanece em 134.217.728 pixels decodificados por operação,
já exercitado pelo caminho de Exportação. O Processador recusa previamente todo
plano de decoder acima de 512 MiB e também entrega esse teto a `image::Limits`.
Como o limite da biblioteca é documentado como best effort para alguns
decoders, ele não substitui as verificações próprias de dimensões, pixels e
alocação planejada antes da materialização.

### Orientação e cor

Orientação EXIF de JPEG ou TIFF é lida pelo decoder e aplicada exatamente uma
vez antes do resize. PNG não recebe rotação implícita.

O primeiro contrato aceita fonte sem perfil ou com uma das três variantes sRGB
já allowlisted e testadas pelo Processador. Perfil incompatível ou declarações
PNG contraditórias falham de forma explícita; não há conversão silenciosa de
Adobe RGB, Display P3, CMYK ou YCCK. O derivado é normalizado para RGBA8/RGB8 e
publicado com o `sRGB2014.icc` canônico.

### Fingerprint e publicação

O fingerprint é `sha256-full-file-v1`: SHA-256 dos bytes integrais abertos pelo
Processador, acompanhado pelo tamanho observado e pelas datas de criação e
modificação disponíveis. Tamanho e datas alimentam inspeção e invalidação
reativas, mas nunca substituem o hash integral nem autorizam reuso. O artefato
imutável é publicado primeiro e o índice da mesma geração por último.

### Representação e tiles

Cada `MediaRef` possui uma representação validada de no máximo 1600 px no maior
lado:

- JPEG qualidade 84 com ICC sRGB para conteúdo opaco, selecionado pelo joelho
  medido da curva taxa–distorção do corpus fotográfico;
- PNG RGBA com ICC sRGB quando qualquer pixel possui alfa abaixo de 255.

Não há tiles, pirâmide nem níveis progressivos. A decisão não vem apenas do
spike sintético: as medições reais anteriores já navegaram 100 Lâminas por
Projeto, limitaram a residência a no máximo 14 texturas e exercitaram Pan/Zoom
com texturas de até 1600 × 1200 px; o driver consultado aceitava 16.384 px por
textura. O ensaio atual confirma que as novas variantes cabem no mesmo único
artefato. Tiles adicionariam protocolo, índice e múltiplas publicações sem
resolver um limite observado.

## Limites da conclusão

Os tempos são observações desta máquina, não SLAs. O spike não promete aceitar
todo TIFF existente, não implementa gerenciamento de cor amplo e não autoriza
pixels do Cache na Exportação final. Originais maiores ou variantes recusadas
continuam indisponíveis para a prévia até que outro ticket apresente nova
medição e decisão normativa.

## Conclusão

A política mensurável para o Programa 03A fica fixada em uma representação por
mídia, 1600 px, JPEG/PNG conforme alfa, sRGB explícito, orientação de JPEG/TIFF
aplicada uma vez e PNG sem rotação implícita, SHA-256 integral, limite estrito
de pixels e TIFF apenas de página única.
Ela preserva o Cache como derivado descartável e mantém a Exportação final
ligada ao `CompositionPlan`, ao snapshot e aos Originais exatos.
