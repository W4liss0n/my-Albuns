---
status: current
document: research
date: 2026-09-23
updated: 2026-09-24
platform: windows-11-x64
---

# Otimização da importação, do Cache e da Exportação

A importação, a geração do Cache e a Exportação foram medidas com fotos reais
e otimizadas em sete mudanças independentes. Nenhuma altera os pixels
exportados. As prévias do Cache passam à representação versão `2`, com outro
filtro de redução, e são regeneradas uma vez. A referência de código é o
commit `25d394a2`.

## Resultado final

Na comparação final, a base e a versão atual alternaram na mesma sessão. A
importação teve três rodadas; cada formato de Exportação teve três rodadas,
com cinco fotos distribuídas por lâmina. A tabela mostra as faixas e, entre
parênteses, as medianas:

| Operação | Base | Atual | Redução da mediana | Saída |
| --- | ---: | ---: | ---: | --- |
| Importar 172 JPEGs (total) | 16,6–19,1 s | 12,7–14,0 s | ~24% | Prévias versão `2` |
| Exportar 6 lâminas em JPEG, por lâmina | 22,4–25,3 s (22,9) | 9,4–13,2 s (10,0) | ~56% | Bytes idênticos |
| Exportar 6 lâminas em JPEG, por página | 33,0–43,1 s (33,8) | 10,0–12,7 s (10,6) | ~69% | Bytes idênticos |
| Exportar 3 lâminas em PNG | 68,2–90,2 s (70,8) | 5,3–6,5 s (6,1) | ~91% | Pixels idênticos, +7% de bytes |
| Exportar 3 lâminas em PDF | 48,0–52,5 s (50,6) | 21,4–23,6 s (23,2) | ~54% | Raster idêntico, +7% de bytes |

A versão atual inclui a correção do limite de memória descrita em
[Limite encontrado](#limite-encontrado); ela não mudou os tempos das lâminas
que já cabiam no teto.

A suíte Rust do projeto (`scripts/Test-Rust.ps1`) passou com 1.029 testes,
inclusive os que exportam e importam pelo Processador real.

A primeira versão desta pesquisa, de 2026-09-23, mediu a Exportação com as
fotos de cada lâmina empilhadas na mesma posição, uma sobre a outra. As
comparações de bytes continuavam válidas, mas os tempos absolutos não
representavam uma diagramação real. Todas as medições de Exportação foram
refeitas em 2026-09-24 com as fotos distribuídas.

Esses números medem o Processador e o Host no perfil de desenvolvimento, sem
WebView, diálogos ou apresentação na janela. Não representam uma promessa de
tempo para qualquer álbum ou máquina.

## Mudanças e efeito medido

Cada linha compara a mudança com o estado imediatamente anterior, em rodadas
alternadas na mesma sessão. As linhas de Exportação vêm das mesmas três
rodadas da comparação final, com as fotos distribuídas.

| Mudança | Antes | Depois | Saída |
| --- | ---: | ---: | --- |
| Lotes de importação distribuídos pelo tamanho dos Originais | 13,2–15,0 s | 11,1–13,2 s | Igual |
| Composição da lâmina em faixas paralelas | 22,4–25,3 s (lâmina) / 33,0–43,1 s (página) | 17,8–18,4 s / 24,6–28,5 s | Bytes idênticos |
| Reuso e decode paralelo dos Originais entre unidades | 17,8–18,4 s / 24,6–28,5 s | 10,1–12,0 s / 9,8–12,1 s | Bytes idênticos |
| Compressão rápida sem perda em PNG | 52,8–66,7 s | 5,2–7,2 s | Pixels idênticos, +7% de bytes |
| Compressão rápida sem perda em PDF | 37,2–46,9 s | 20,1–25,3 s | Raster idêntico, +7% de bytes |
| Original lido uma vez para hash e decode | 9,7–10,7 s nativos | 9,9–11,2 s nativos | Igual |
| Confirmação final do Original por identidade, tamanho e datas | 9,2–12,8 s nativos | 9,1–11,1 s nativos | Igual |
| Redução das prévias por média de área, orientação depois | 12,4–14,0 s | 10,1–11,9 s | Prévias versão `2` |

As duas mudanças de leitura do Original ficam dentro do ruído em disco local
com o Cache de arquivos aquecido: ler de novo um JPEG já em memória custa
poucos milissegundos. Elas reduzem de três para uma as leituras completas por
foto, o que pesa quando os Originais estão em rede ou em um disco externo. O
teste de cada módulo comprova essa contagem.

### Importação

Os candidatos eram divididos em lotes contíguos na ordem da seleção. Como a
seleção costuma agrupar câmeras de resoluções muito diferentes, a importação
esperava pelo lote mais pesado. Os lotes agora recebem primeiro os maiores
Originais, pelo tamanho já observado na captura, sem nova leitura. O limite de
32 por processo e a ordem da seleção dentro de cada lote continuam.

### Exportação

A Exportação já reservava toda a capacidade do Processador, mas compunha cada
lâmina em uma única thread. Cada camada agora divide suas linhas entre os
trabalhadores reservados, na mesma ordem e com a mesma aritmética; o teste
compara 1, 2, 3 e 8 trabalhadores. A orientação EXIF deixou de acessar pixel
a pixel com verificação de limites.

Cada unidade de saída decodificava de novo todos os Originais da lâmina. O
modo Página decodificava a mesma lâmina duas vezes, e um Decorativo repetido
era decodificado em cada lâmina. Os rasters agora permanecem enquanto a
próxima unidade ainda os usa, sob o mesmo teto de pixels por unidade, e os que
faltam são decodificados em paralelo. JPEG progressivo mantém seu processo
isolado e continua sequencial.

PNG e PDF usavam o nível padrão de compressão, que dominava essas exportações.
Os níveis rápidos continuam sem perda, e a verificação por decodificação
permanece. O [Contrato do Renderizador final](../design/0019-contrato-do-renderizador-final.md)
registra a troca.

### Cache

O Processador lia o Original inteiro três vezes por foto: para o hash, para o
decode e para confirmar antes de publicar. Agora os bytes do hash são os
mesmos decodificados, e a confirmação final compara identidade física, tamanho
e datas, conforme a emenda ao [ADR 0001](../adr/0001-vincular-arquivos-externos.md).

A representação versão `2` troca `DynamicImage::thumbnail` pela redução Box do
`fast_image_resize`, com a mesma regra de dimensões, e orienta somente os
pixels já reduzidos nos JPEGs coloridos de linha de base. A comparação com a
versão `1` em dez fotos do corpus ficou entre 29 e 44 dB de PSNR; as maiores
diferenças aparecem em contornos finos das fotos de 6 MP, que a nova redução
deixa menos serrilhados. O codificador JPEG não mudou: o `jpeg-encoder`, mais
rápido, foi descartado por exigir a atribuição da licença IJG. O
[design de armazenamento e Cache](../design/0010-armazenamento-local-e-cache.md)
registra a versão.

## Decode reduzido no domínio DCT

Depois das mudanças, o decode do JPEG ocupa a maior parte do trabalho de cada
prévia. Um decode em escala 1/2 ou 1/4 evitaria materializar pixels que a
redução descarta. O `zune-jpeg`, usado pelo `image`, não oferece essa opção.

O `jpeg-decoder`, em Rust puro, oferece. No mesmo corpus, o decode reduzido
de fotos de 18 a 24 MP levou de 101 a 150 ms, contra 136 a 164 ms do decode
completo atual. Somada a redução, a prévia ficou de 10 a 25% mais rápida
nessas fotos e igual nas de 6 MP. A decodificação de entropia percorre todos
os coeficientes em qualquer escala e domina o custo.

A libjpeg-turbo tende a ganhar mais pela decodificação de entropia
otimizada, mas depende de uma biblioteca C, de CMake e de NASM para suas
rotinas SIMD. Sem NASM na máquina, esse caminho não foi medido. Ele também
exigiria refazer o controle de memória que hoje protege o decoder e o processo
isolado de JPEG progressivo.

**Recomendação:** não adotar agora. O ganho medido em Rust puro é pequeno e o
caminho com libjpeg-turbo traz uma dependência nativa sem medição que a
justifique. Uma nova avaliação só se paga se a importação de álbuns grandes
continuar lenta depois destas mudanças.

## Limite encontrado

Seis fotos de 24 MP na mesma lâmina somam 144 milhões de pixels e excediam o
teto de 134.217.728 pixels por unidade de Exportação. A Exportação falhava com
`resourceLimitExceeded` antes de compor.

Resolvido no mesmo dia: o teto passou a valer para cada fonte isolada e para os
rasters mantidos ao mesmo tempo, e não mais para a soma da lâmina. As fontes são
decodificadas perto da camada que as usa e descartadas depois do último uso,
conforme o [contrato do primeiro fluxo JPEG](../design/0014-contrato-jpeg-do-primeiro-fluxo.md#guardrail-provisório-de-recursos).
Lâminas com 6 e 10 fotos de 24 MP passaram a exportar, inclusive por página, e
o pico de memória do Processador ficou entre 774 e 838 MB com 5 ou com 10
fotos distribuídas. As lâminas que já cabiam no teto mantiveram bytes e tempos.

## Método

- Windows 11, Intel Core i5-13450HX, 16 processadores lógicos, 24 GB de RAM.
- Corpus `benchmark-data/albums`: 172 JPEGs reais de 6 a 24 MP em dois
  álbuns, 1,4 GB, fora do Git. Os Originais foram apenas lidos.
- Perfil `dev` do projeto, com `CARGO_PROFILE_DEV_DEBUG=0` para caber no disco;
  os pacotes de pixels continuam em `opt-level = 3`.
- Importação: teste `photo_import::native_flow_tests::real_import_flow` com o
  Processador real, 172 fotos em uma seleção, capacidade de 8 processos, em
  rodadas alternadas entre binários preservados antes e depois de cada mudança.
- Exportação: projeto criado pelo `ProjectCore` com 5 fotos por lâmina de
  600 × 300 mm a 300 DPI, colocadas como na interface (`PhotoPlacementMode::Normal`),
  em 5 posições distintas por lâmina. O pedido vai ao Processador real pelo
  protocolo de `RenderAlbum`. Cada versão do Processador foi preservada como
  binário e alternada com as demais nas mesmas rodadas. Bytes de saída
  comparados por SHA-256; PNG comparado também pelos pixels decodificados e
  PDF pelos rasters descomprimidos de cada página.
- Pico de memória: maior working set do processo do Processador, amostrado a
  cada 50 ms durante a exportação por página de 2 lâminas com 5 ou 10 fotos
  de 24 MP distribuídas.
- A máquina tinha outros programas ativos (navegador, catalogação de fotos,
  builds de outros agentes), com variação de até 2× entre sessões. Por isso
  cada comparação alternou as versões na mesma sessão; números de sessões
  diferentes não devem ser comparados entre si.
