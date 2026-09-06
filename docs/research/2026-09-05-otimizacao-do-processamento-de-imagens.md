---
status: current
document: research
date: 2026-09-05
---

# Processamento de imagens: comparação antes e depois

O limite permanece em **duas imagens simultâneas por Projeto**. A geração do
Cache ficou aproximadamente **6% mais rápida nesta amostra**, com arquivos de
saída idênticos. O Painel de imagens agora apresenta os novos cartões juntos ao
terminar o lote, preservando as imagens anteriores durante a operação.

## Mudanças e resultados

| Aspecto | Antes | Depois |
| --- | --- | --- |
| Mediana para gerar o Cache de cinco JPEGs | 846,09 ms | 794,91 ms |
| Menor / maior duração observada | 795,60 / 892,81 ms | 757,48 / 829,43 ms |
| Concorrência | 2 imagens | 2 imagens |
| Buffers intermediários da prévia | Cópia RGBA após redução; cópia RGBA e conversão integral RGB antes de codificar | Consumo do buffer reduzido e codificação JPEG diretamente da prévia RGBA opaca |
| Três avisos do Monitor durante a importação, no teste de regressão | Até 3 recargas antecipadas do catálogo | 1 recarga após o lote |
| Grade durante a importação | Novos espaços apareciam antes de o Cache terminar | Apenas o conjunto anterior; cartões novos liberados juntos ao concluir |
| Comparação das prévias | Versão de referência | Mesmos bytes, dimensões e tamanho em todas as 14 execuções |

A redução medida foi de **51,19 ms por lote**, ou **6,05%**. Há sobreposição dos
intervalos e a amostra é pequena: esse resultado descreve esta máquina e estes
arquivos, sem prometer o mesmo percentual para outros lotes.

## O que foi revisado

O fluxo continua validando integralmente a origem, preparando ou reutilizando
o Cache, verificando a representação e publicando a projeção. Validação e Cache
usam o limite compartilhado de dois trabalhos. A ordem do catálogo, o progresso
por imagem e a política de falhas permanecem sob os mesmos componentes.

O Monitor podia observar o catálogo já vinculado enquanto a ação ainda
preparava o Cache. A recarga em `App` publicava esse estado intermediário e
iniciava demanda de prévias para os novos cartões. Agora ela aguarda a fila de
mutações; avisos intermediários são agrupados. Uma leitura já iniciada também
aguarda eventual ação posterior antes de publicar seu resultado. Continuam
valendo as verificações de Projeto, revisão e atualização mais recente. Quando
essa espera conclui uma ação, seu resultado prevalece também na mesma revisão;
isso preserva o estado salvo se Salvar termina enquanto a leitura aguardava.

No Processador, o resultado reduzido já era RGBA, mas `to_rgba8` copiava seus
pixels. `into_rgba8` reaproveita esse buffer. Para JPEG, o codificador aceita a
prévia RGBA opaca diretamente: desaparecem o clone completo e a conversão
intermediária RGB. A escolha PNG para transparência, o perfil sRGB, a qualidade,
a orientação e a verificação da origem permanecem iguais.

Para cada prévia medida de 1600 × 1233 pixels, essas três operações alocavam
aproximadamente **20,70 MiB de buffers intermediários** que foram eliminados.
Esse número é calculado pelos tamanhos dos buffers (4 + 4 + 3 bytes por pixel),
**não é uma medição da redução do pico de RAM do aplicativo**. A decodificação
do Original continua podendo determinar o pico.

## Método

- Referência: Processador compilado a partir de `80b50b8`, preservado antes da
  alteração; comparação com o Processador desta mudança.
- Máquina: Windows, Intel Core i5-13450HX, 10 núcleos e 16 processadores lógicos.
- Mesmos cinco JPEGs reais já usados no diagnóstico local; originais apenas
  lidos e SHA-256 conferido antes e depois.
- Sete rodadas por versão, alternando qual executa primeiro, com duas tarefas
  simultâneas e um namespace de Cache novo por execução.
- Mesmo perfil de compilação `dev` do projeto, que otimiza o Processador e
  dependências de imagem; sem mudança de parâmetros de qualidade.
- A execução final ocorreu sem compilação ou suíte de testes iniciada pelo
  agente em paralelo. O Cache de arquivos do Windows não foi esvaziado.
- Tempo inclui iniciar os processos e gerar seus artefatos; exclui a validação
  anterior à importação, publicação do índice pelo Host e apresentação na UI.
  Hashes e decodificação de conferência ficam fora do cronômetro.

O [comparador reproduzível](../../crates/myalbuns-imaging/examples/measure_cache_batch.rs)
recebe um JSON com a lista de caminhos, os dois executáveis e uma pasta nova de
saída. Pode ser compilado com:

```powershell
& ./scripts/Invoke-LocalCargo.ps1 -CargoArguments @(
  'build', '-p', 'myalbuns-imaging', '--example', 'measure_cache_batch'
)
& ./target/debug/examples/measure_cache_batch.exe inputs.json before.exe after.exe output
```

Os dados locais completos estão em
`.tools/image-processing-optimization/final-measurements/measurements.json`.
Cada execução retém as prévias, os logs e seus hashes na mesma pasta de evidência.

## Verificação e limites

O teste em `App` reproduziu os cartões prematuros antes da correção. Depois,
cobre sucesso, falha após vínculo, notificações repetidas e leitura do Monitor
iniciada antes da ação. O teste existente de respostas fora de ordem continua
verificando que uma observação antiga não substitui a mais recente.
Um teste adicional reproduziu e corrigiu a disputa entre essa espera e Salvar:
a leitura antiga não repõe o indicador de alterações não salvas.

A conferência nativa decodificou todas as prévias e comprovou igualdade binária
entre as versões nas 70 gerações. A validação geral usa `npm run validate`, com
resultado em `.tools/validation/report.json`; a revisão visual usa a lista
de cenários do repositório por `npm run ui:acceptance`.

A revisão também identificou trabalho futuro possível: validação inicial e
geração do Cache ainda decodificam o Original em momentos diferentes, e a
demanda posterior revalida o Cache e a origem. Compartilhar esses resultados
exigiria manter uma autoridade de validade entre processos e invalidações do
Monitor. Esse caminho não foi alterado, e seu ganho não foi medido neste ensaio.

## Contratos consultados

- [image 0.25.10 — DynamicImage::into_rgba8](https://docs.rs/image/0.25.10/image/enum.DynamicImage.html#method.into_rgba8).
- [image 0.25.10 — implementação de JpegEncoder](https://docs.rs/image/0.25.10/src/image/codecs/jpeg/encoder.rs.html),
  conferida também no código da versão instalada; `encode_image` aceita RGB8 e
  RGBA8 e percorre blocos de pixels sem exigir um buffer RGB completo.
