---
status: current
document: research
date: 2026-10-14
platform: windows-11-x64
---

# Redução na leitura do JPEG

Num computador de dois núcleos, a preparação das prévias na abertura é limitada
pelo processador
([medição de 2026-10-13](2026-10-13-progresso-de-abertura-em-pc-fraco.md#orçamento-fixo-de-memória)).
O Processador de Imagens decodifica o JPEG inteiro, de 24 a 30 MP, e só depois
reduz para a prévia, de no máximo 1600 pixels no lado maior. Esta pesquisa mede
quanto se ganha decodificando já em escala reduzida, como fazem a libvips e o
libjpeg-turbo, e quanto a prévia muda.

## Como foi medido

- Um programa de teste fora do produto reproduz a prévia de hoje: decodificação
  com o `image` 0.25.10, que usa o `zune-jpeg` 0.5.15, redução por média de
  área (`FilterType::Box` do `fast_image_resize` 5.5.0, como `cache.rs`) e
  codificação JPEG com qualidade 84 (`CACHE_JPEG_QUALITY`).
- As alternativas trocam só a decodificação e mantêm a mesma redução e a mesma
  codificação:
  - `jpeg-decoder` 0.3, em Rust, que decodifica em 1/8, 1/4, 1/2 ou tamanho
    cheio; usa-se a menor escala que ainda cobre a prévia;
  - libjpeg-turbo 3 com SIMD, pelo crate `turbojpeg` 1.5.1, em tamanho cheio e
    na menor fração n/8 que ainda cobre a prévia.
- Tudo roda numa thread, como o Processador de Imagens. Cada foto passa três
  vezes por cada variante, e vale a mais rápida.
- Fotos: 8 fotos reais de câmera, de 6000 × 4000 pixels e 5 a 8 MB, do acervo
  local de testes, e as fotos do álbum de teste do laboratório, de 6000 × 4000
  e 6720 × 4480 pixels e cerca de 10 MB.
- Computador: i5-13450HX. Ele estava em uso durante a medição, então os tempos
  absolutos variaram entre rodadas. As comparações usam a razão entre variantes
  na mesma foto e na mesma rodada.
- A qualidade foi medida contra duas referências: a prévia de hoje e uma
  redução de alta qualidade, com Lanczos3 a partir da decodificação inteira.

O crate `cmake` compilou o código C do libjpeg-turbo sem otimização no MSVC, o
que deixou a primeira medição injusta. Os números abaixo usam o libjpeg-turbo
compilado com o CMake em modo Release e ligado estaticamente.

## Resultado

Tempo de uma prévia, em relação à prévia de hoje na mesma foto (mediana):

| Variante | Fotos reais | Álbum de teste |
|---|---:|---:|
| `jpeg-decoder` em escala (1/2 ou 1/4) | −27% | −29% |
| libjpeg-turbo, tamanho cheio | −19% | −18% |
| libjpeg-turbo em escala (n/8) | −41% | −38% |

Numa rodada com o computador menos carregado, a prévia de hoje levou cerca de
215 ms por foto real de 24 MP, dos quais cerca de 150 ms na decodificação.

Qualidade (PSNR, quanto maior, mais próximo):

| Variante | Contra a prévia de hoje | Contra Lanczos3 (fotos reais) | Contra Lanczos3 (álbum de teste) |
|---|---:|---:|---:|
| Hoje | — | 45,2 dB (mín. 41,7) | 44,0 dB (mín. 42,5) |
| `jpeg-decoder` em escala | 40,7 dB (mín. 37,5) | 40,9 dB (mín. 37,6) | 39,9 dB (mín. 34,5) |
| libjpeg-turbo, tamanho cheio | 62,8 dB (mín. 61,8) | 45,2 dB (mín. 41,7) | 43,9 dB (mín. 42,4) |
| libjpeg-turbo em escala | 40,9 dB (mín. 37,7) | 40,9 dB (mín. 37,7) | 39,6 dB (mín. 34,5) |

- O libjpeg-turbo em tamanho cheio dá praticamente a mesma prévia de hoje. As
  diferenças são de arredondamento, de até 3 níveis.
- A decodificação em escala fica cerca de 4 dB mais longe da referência. Numa
  foto real ampliada duas vezes, as duas prévias não se distinguem; 0,12% dos
  pixels diferem mais de 20 níveis. A própria codificação da prévia em
  qualidade 84 introduz erro dessa ordem.
- O pior caso é o álbum de teste, feito de ruído colorido com texto. Nas fotos
  de 6720 pixels, a escala 1/4 já dá 1680 pixels, e a prévia em escala sai um
  pouco mais suave no ruído. O texto fica igual.

## O que isso significa para a abertura

- O `zune-jpeg` atual é mais rápido que o libjpeg-turbo sem otimização, mas
  cerca de 20% mais lento que o libjpeg-turbo compilado em Release.
- Decodificar em escala corta de 27% a 41% do tempo de cada prévia. Numa foto
  real de 24 MP, a decodificação de entropia, igual em qualquer escala,
  continua sendo boa parte do custo.
- No laboratório, cada Processador de Imagens vive cerca de 280 a 330 ms por
  foto. Inferência não medida: se a prévia cair cerca de 90 ms, a preparação de
  300 fotos com dois Processadores de Imagens encurta em torno de 13 s numa
  abertura de cerca de 80 s. O restante é o início de um processo por foto, a
  leitura e a verificação do arquivo e a gravação no Cache.
- A memória da decodificação cai para cerca de um quarto na escala 1/2. A
  estimativa de admissão (6 bytes por pixel do tamanho cheio) passaria a
  sobrar; não foi medida.

## Custos de cada caminho

- **`jpeg-decoder`.** É Rust puro e não muda a compilação. O `image` 0.25
  trocou esse decodificador pelo `zune-jpeg`. Só valeria para o JPEG sequencial colorido que hoje já tem
  caminho próprio; os outros casos continuariam no caminho atual. A manutenção
  atual do crate não foi verificada.
- **libjpeg-turbo.** É C com assembly, usado por navegadores e pela libvips.
  Exige CMake e NASM em toda máquina que compila o aplicativo e um ajuste para
  o MSVC compilar o C otimizado. Traz uma dependência nativa a mais para o
  Processador de Imagens, que hoje é todo Rust.

## Resultado no aplicativo

A pedido do autor, o Processador de Imagens passou a usar o `jpeg-decoder` em
escala para a prévia de JPEG sequencial YCbCr maior que a prévia
(`decode_scaled_jpeg` em `source.rs`). JPEG com componentes RGB, progressivo
ou em tons de cinza continua no caminho anterior. Se o `jpeg-decoder` recusar
o arquivo ou entrar em pânico com um arquivo malformado, a decodificação
inteira também continua valendo. O tamanho da
prévia ainda é calculado a partir do tamanho original, então as dimensões não
mudam.

Na primeira versão, o ganho desapareceu: o `jpeg-decoder` lê o arquivo quase
byte a byte, e cada leitura passava pelo leitor genérico do Processador de
Imagens. A decodificação em escala ficou cerca de 15% mais lenta que a
decodificação inteira. Com o leitor concreto, ficou cerca de 10% mais rápida
numa foto, e a redução final passou a partir de 3000 pixels em vez de 6000.

No laboratório, perfil `cliente-ssd`, um Projeto de 300 fotos, abertura fria,
com as duas versões alternadas:

| Rodada | Tempo de cada foto no Processador de Imagens | Preparação | Editor na tela |
|---|---:|---:|---:|
| Em escala, 1ª | 339 ms | 67,3 s | 78,3 s |
| Inteira, 1ª | 359 ms | 68,8 s | 84,7 s |
| Em escala, 2ª | 328 ms | 64,0 s | 74,0 s |
| Inteira, 2ª | 393 ms | 79,5 s | 96,4 s |

Nas duas duplas, a versão em escala foi mais rápida: na média, 334 contra
376 ms por foto (−11%) e 76 contra 91 s até o editor. A carga do computador
hospedeiro subiu de 17% para 45% durante as rodadas, e a última rodada inteira
foi a mais afetada; o ganho real fica entre os 6% da primeira dupla e os 17% da
segunda. Ele é menor que o do programa de teste porque o tempo de cada foto no
Processador de Imagens também inclui ler e verificar o Original, gravar a
prévia e iniciar o processo.

## Arquivos truncados ou corrompidos

Um teste provisório passou pelo código do Processador de Imagens versões
danificadas de duas fotos de 6000 × 4000 pixels: uma foto real de câmera e uma
do álbum de teste. Cada versão foi aberta pelo caminho anterior (decodificação
inteira pelo `zune-jpeg`), só pelo `jpeg-decoder` em escala e pelo caminho
novo completo. A parte danificada é a fração de pixels que difere mais de 40
níveis da foto intacta.

| Dano | Caminho anterior | `jpeg-decoder` em escala | Caminho novo |
|---|---|---|---|
| Cortada em 25%, 50%, 75%, 95% ou 99,9% | Aceita | Recusa | Decodificação inteira, igual ao anterior |
| Cortada e com o marcador de fim (`FF D9`) | Aceita | Aceita | Em escala |
| 64 bytes trocados no meio | Aceita | Aceita | Em escala |
| 4 KB zerados no meio | Aceita | Aceita | Em escala |
| Só sem o marcador de fim | Aceita | Recusa | Decodificação inteira, igual ao anterior |

- O caminho novo não aceita nenhum arquivo que o anterior recusava. O
  `zune-jpeg` não recusou nenhuma das versões.
- Quando os dois aceitam, a parte danificada da prévia é a mesma, com
  diferença de até 0,4 ponto percentual. Por exemplo, cortada em 50% com o
  marcador de fim: 45,2% e 45,2% na foto real; 45,3% e 45,3% no álbum de teste.
- Quando o `jpeg-decoder` recusa, o arquivo é decodificado duas vezes. O custo
  extra só aparece em arquivo danificado.
- Já antes desta mudança, um JPEG truncado ou corrompido gerava uma prévia
  incompleta, com a parte que falta preenchida, e não um aviso de falha. Isso
  não mudou.

## Limites

- As fotos reais são 8, todas da mesma câmera. Outras câmeras, fotos
  progressivas, 4:4:4 ou com perfil de cor não foram medidas.
- O efeito na abertura foi medido no laboratório, cuja CPU é muito mais rápida
  que a de um PC de dois núcleos antigo
  ([medição de 2026-10-13](2026-10-13-progresso-de-abertura-em-pc-fraco.md)).
- O computador estava em uso, com carga variável. As razões por foto são mais
  confiáveis que os tempos absolutos.
- O programa de teste e os resultados por foto ficam fora do Git.
- Os danos testados foram cortes, bytes trocados e bytes zerados em duas
  fotos. Cabeçalhos corrompidos, tabelas de Huffman inválidas e JPEG
  progressivo danificado não foram testados.
