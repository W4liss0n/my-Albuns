---
status: current
document: research
date: 2026-10-14
platform: windows-11-x64
---

# Admissão por memória em programas consolidados

Num PC de 4 GB com um editor aberto, o segundo Processador de Imagens esperou
82 vezes, somando 32,6 s
([medição de 2026-10-13](2026-10-13-progresso-de-abertura-em-pc-fraco.md)).
A pergunta é como programas consolidados decidem quantas imagens processar ao
mesmo tempo quando a RAM é escassa. Para cada um, foram verificados cinco
pontos:

1. se estimam a memória por imagem, e como;
2. que margem guardam e que parte da RAM usam;
3. como reagem à pressão de memória;
4. se o paralelismo é limitado por núcleos, por memória ou pelos dois;
5. se contam duas vezes o trabalho que já está rodando.

Só entram fontes primárias: código-fonte numa versão fixa, manuais e páginas de
suporte oficiais e o Microsoft Learn. Quando a fonte não diz algo, o texto
afirma isso. Conclusões tiradas do código, sem documentação, aparecem marcadas
como inferência.

## Regra atual do MyAlbuns

Pela [revisão da admissão por memória](../design/0032-revisao-da-admissao-por-memoria.md)
e por [`resources.rs`](../../src-tauri/src/imaging_processor/resources.rs):

- A estimativa de um JPEG sequencial colorido é 6 bytes por pixel, mais duas
  vezes o tamanho do arquivo, mais 32 MiB. Nos demais casos, 16 bytes por
  pixel e 64 MiB.
- O teto agregado é o menor valor entre um quarto da RAM total e 4 GiB.
- Um trabalho paralelo só começa se a estimativa couber em
  `(RAM física livre − margem) / 2` e em `(commit livre − margem) / 2`,
  descontadas as reservas ativas. A margem é um oitavo da RAM total, entre
  512 MiB e 2 GiB.
- Se nada estiver rodando, um trabalho começa se couber no commit disponível
  e no teto. Essa é a execução serial.
- A fila reavalia os recursos a cada 100 ms.

No laboratório de 4 GB, o pico real de um Processador de Imagens foi 105 MiB
(24 MP) e 125 MiB (30 MP). As estimativas são 189 e 230 MiB. Para dois
trabalhos simultâneos, a regra exige de 1268 a 1432 MiB livres, e a RAM livre
medida ficou entre 926 e 1450 MiB. A RAM livre lida já desconta a memória
residente do primeiro Processador de Imagens, e a reserva dele continua
descontada inteira.

## darktable

Versão lida: [`release-5.6.2`](https://github.com/darktable-org/darktable/tree/release-5.6.2).

**Estimativa por imagem.** Cada módulo declara um fator e um custo fixo. O
pipeline testa se `fator × largura × altura × bytes por pixel + custo fixo`
cabe no orçamento ([`tiling.c` L2127–2138](https://github.com/darktable-org/darktable/blob/release-5.6.2/src/develop/tiling.c#L2127-L2138),
chamado em [`pixelpipe_hb.c` L1460–1468](https://github.com/darktable-org/darktable/blob/release-5.6.2/src/develop/pixelpipe_hb.c#L1460-L1468)).
Se não cabe, o módulo processa a imagem em blocos (tiling), com o tamanho do
bloco derivado do orçamento dividido pelo fator
([`tiling.c` L612–631](https://github.com/darktable-org/darktable/blob/release-5.6.2/src/develop/tiling.c#L612-L631)).

**Margem e uso da RAM.** O orçamento é uma fração da RAM **total**, definida
pelo nível de recursos, em unidades de 1/1024
([`darktable.c` L1852–1894](https://github.com/darktable-org/darktable/blob/release-5.6.2/src/common/darktable.c#L1852-L1894)):

| Nível | RAM para módulos | Buffer mínimo | Cache de miniaturas | Memória OpenCL |
| --- | ---: | ---: | ---: | ---: |
| small | 128/1024 (12,5%) | 4/1024 | 64/1024 | 400/1024 |
| default | 512/1024 (50%) | 8/1024 | 128/1024 | 700/1024 |
| large | 700/1024 (68%) | 16/1024 | 128/1024 | 900/1024 |
| unrestricted | 16384/1024 | 1024/1024 | 128/1024 | 900/1024 |

- Abaixo de 8 GiB, o darktable desconta 1 GiB da RAM total antes de aplicar a
  fração ([L1893](https://github.com/darktable-org/darktable/blob/release-5.6.2/src/common/darktable.c#L1893)).
- O orçamento nunca fica abaixo de 512 MiB
  ([`dt_get_available_mem`, L2516–2525](https://github.com/darktable-org/darktable/blob/release-5.6.2/src/common/darktable.c#L2516-L2525)).
- Na primeira execução, o nível é `small` se a RAM, já descontada, for menor
  que 4096 MB ou houver menos de 2 threads; senão é `default`
  ([L2538–2558](https://github.com/darktable-org/darktable/blob/release-5.6.2/src/common/darktable.c#L2538-L2558)).
  O usuário escolhe o nível nas preferências e pode mudar as frações no
  `darktablerc` ([L1883–1887](https://github.com/darktable-org/darktable/blob/release-5.6.2/src/common/darktable.c#L1883-L1887)).
- O orçamento de um pipeline desconta a memória de Cache dos pipelines do
  darkroom, não fica abaixo de 128 MiB e é dividido por 3 nos pipelines de
  miniatura ([`pixelpipe_hb.c` L330–345](https://github.com/darktable-org/darktable/blob/release-5.6.2/src/develop/pixelpipe_hb.c#L330-L345)).
- Na GPU, há margem fixa de 600 MB
  ([`opencl.h` L32](https://github.com/darktable-org/darktable/blob/release-5.6.2/src/common/opencl.h#L32)),
  e a fração do nível incide sobre a memória da placa menos essa margem
  ([`opencl.c` L3621–3663](https://github.com/darktable-org/darktable/blob/release-5.6.2/src/common/opencl.c#L3621-L3663)).

Inferência pelo código: num PC de 4 GB, o nível inicial é `small`. A conta dá
128/1024 de cerca de 3 GB, perto de 384 MiB, e o piso eleva o orçamento para
512 MiB.

O [manual de memória e desempenho](https://docs.darktable.org/usermanual/development/en/special-topics/mem-performance/)
descreve a mesma fórmula sobre a memória total. Já a
[página de preferências de processamento](https://docs.darktable.org/usermanual/development/en/preferences-settings/processing/)
dá números aproximados que não batem com o código: cerca de 20% para `small`,
60% para `default` e 75% para `large`. O manual também afirma que é impossível
saber quanta memória de GPU está livre num dado momento.

**Reação à pressão.** Nos arquivos lidos, o darktable consulta só a RAM total
(`ullTotalPhys` no Windows,
[`darktable.c` L545–588](https://github.com/darktable-org/darktable/blob/release-5.6.2/src/common/darktable.c#L545-L588)).
Não lê RAM livre nem sinais do sistema. Quando a memória não basta, ele divide
o trabalho em blocos. Um erro de OpenCL ou a falta de memória na GPU fazem o
módulo rodar na CPU, e o pipeline é refeito sem OpenCL
([`pixelpipe_hb.c` L2168–2176](https://github.com/darktable-org/darktable/blob/release-5.6.2/src/develop/pixelpipe_hb.c#L2168-L2176),
[L3248–3258](https://github.com/darktable-org/darktable/blob/release-5.6.2/src/develop/pixelpipe_hb.c#L3248-L3258)).

**Paralelismo.** Usa 7 threads de trabalho se a RAM total for de pelo menos
7,5 GiB e houver ao menos 4 threads de CPU; senão, 4
([`darktable.c` L2509–2513](https://github.com/darktable-org/darktable/blob/release-5.6.2/src/common/darktable.c#L2509-L2513),
[`jobs.c` L615](https://github.com/darktable-org/darktable/blob/release-5.6.2/src/control/jobs.c#L615)).
Dentro de cada pipeline, o OpenMP usa os núcleos. O número de trabalhos
depende da RAM total e dos núcleos. O orçamento de memória é fixo por pipeline.

**Dupla contagem.** Não ocorre: o orçamento é um número fixo por pipeline,
derivado da RAM total, e não uma leitura de RAM livre. Inferência pelos
arquivos lidos: também não há contador agregado da memória dos pipelines
simultâneos. A divisão por 3 nos pipelines de miniatura é a única folga
explícita para vários pipelines ao mesmo tempo.

## libvips

Versão lida: [`v8.18.7`](https://github.com/libvips/libvips/tree/v8.18.7).

**Estimativa por imagem.** A libvips não estima a memória antes de começar. O
processamento é por blocos, sob demanda. No acesso sequencial, a imagem é
lida uma vez, de cima para baixo, sem converter tudo de uma vez. No acesso
aleatório, imagens pequenas vão para a memória e imagens grandes vão para um
arquivo temporário em disco
([`image.c` L1895–1916](https://github.com/libvips/libvips/blob/v8.18.7/libvips/iofuncs/image.c#L1895-L1916)).
O limiar padrão é de 100 MB, ajustável por `VIPS_DISC_THRESHOLD`
([L2552–2566](https://github.com/libvips/libvips/blob/v8.18.7/libvips/iofuncs/image.c#L2552-L2566)).
O `thumbnail` abre a imagem em acesso sequencial e com redução já na leitura
([`thumbnail.c` L167–168, L1215](https://github.com/libvips/libvips/blob/v8.18.7/libvips/resample/thumbnail.c#L1215)).

**Margem e uso da RAM.** Não consulta a RAM do sistema. O Cache de operações
tem limites fixos: 100 operações, 100 MB e 100 arquivos abertos
([`cache.c` L75–94](https://github.com/libvips/libvips/blob/v8.18.7/libvips/iofuncs/cache.c#L75-L94)).
O comentário do código registra que o limite era de 1 GB e foi reduzido para
ferramentas em lote, como o `vipsthumbnail`.

**Reação à pressão.** Não reage a sinais do sistema. O Cache descarta
operações antigas quando a memória que a própria libvips rastreia passa do
limite ([`cache.c` L905–917](https://github.com/libvips/libvips/blob/v8.18.7/libvips/iofuncs/cache.c#L905-L917),
[`memory.c` L516](https://github.com/libvips/libvips/blob/v8.18.7/libvips/iofuncs/memory.c#L516)).

**Paralelismo.** O número de threads por pipeline é o número de processadores,
salvo `VIPS_CONCURRENCY` ou `vips_concurrency_set`, entre 1 e 1024
([`thread.c` L156–250](https://github.com/libvips/libvips/blob/v8.18.7/libvips/iofuncs/thread.c#L156-L250)).
Imagens pequenas usam menos threads, conforme o número de blocos
([`threadpool.c` L507–528](https://github.com/libvips/libvips/blob/v8.18.7/libvips/iofuncs/threadpool.c#L507-L528)).
Inferência pelo código: cada pipeline cria seu próprio grupo de threads, então
várias imagens simultâneas multiplicam as threads. O limite é só por núcleos.

**Dupla contagem.** Não se aplica: a libvips conta só as próprias alocações e
não lê a RAM livre.

## ImageMagick

Versão lida: [`7.1.2-33`](https://github.com/ImageMagick/ImageMagick/tree/7.1.2-33).

**Estimativa por imagem.** O tamanho do Cache de pixels é exato:
`pixels × (canais × tamanho do Quantum + metadados)`
([`cache.c` L3879–3884](https://github.com/ImageMagick/ImageMagick/blob/7.1.2-33/MagickCore/cache.c#L3879-L3884)).
A [documentação de arquitetura](https://imagemagick.org/architecture/) diz que
a versão Q16 consome o dobro da Q8.

**Margem e uso da RAM.** Os limites padrão derivam da RAM **total**
([`resource.c` L1218–1300](https://github.com/ImageMagick/ImageMagick/blob/7.1.2-33/MagickCore/resource.c#L1218-L1300)):

- `memory` é a RAM física total, dividida por 2 no Windows;
- `map` é 2 × `memory`, ou seja, a RAM total no Windows;
- `area` é 4 × esse valor, em pixels;
- `disk` não tem limite;
- `thread` é o máximo do OpenMP menos 1, com o comentário "reserve core for OS".

No Windows, as páginas físicas vêm de `ullTotalPhys`
([`nt-base.c` L2934–2946](https://github.com/ImageMagick/ImageMagick/blob/7.1.2-33/MagickCore/nt-base.c#L2934-L2946)).
O `policy.xml`, as variáveis `MAGICK_*_LIMIT` e `-limit` mudam os valores. A
[página de recursos](https://imagemagick.org/resources/) explica que a
política é um teto e que as variáveis de ambiente só podem restringir. O
modelo [`policy-limited.xml`](https://github.com/ImageMagick/ImageMagick/blob/7.1.2-33/config/policy-limited.xml#L52-L72)
sugere memory 768 MiB, map 2 GiB, area 32 MP, disk 2 GiB e 4 threads.

**Reação à pressão.** Não lê sinais do sistema. Quando a própria contagem
passa do limite de `memory`, o Cache de pixels vai para um arquivo em disco
mapeado na memória. Se passar de `map`, vai para E/S comum em disco. Passar de
`disk` gera erro
([`cache.c` L3885–3957](https://github.com/ImageMagick/ImageMagick/blob/7.1.2-33/MagickCore/cache.c#L3885-L3957),
[L4074–4096](https://github.com/ImageMagick/ImageMagick/blob/7.1.2-33/MagickCore/cache.c#L4074-L4096)).
A [arquitetura](https://imagemagick.org/architecture/) diz que o disco é até
1000 vezes mais lento que a memória e que os limites valem para cada
invocação, ou seja, para cada processo.

**Paralelismo.** É limitado por núcleos. O código usa núcleos − 1. A
documentação diz que o padrão é "usually the number of cores", o que
diverge do código.

**Dupla contagem.** Não ocorre. O ImageMagick soma as próprias reservas
([`resource.c` L299–309](https://github.com/ImageMagick/ImageMagick/blob/7.1.2-33/MagickCore/resource.c#L299-L309))
e as compara a um limite fixo. Não lê a RAM livre.

## RawTherapee

Versão lida: [`5.13`](https://github.com/Beep6581/RawTherapee/tree/5.13). A
página de preferências da RawPedia respondeu 404 e não foi consultada.

**Estimativa, margem e reação à pressão.** Nos arquivos lidos, não há
estimativa por imagem, fração de RAM nem reação a sinais do sistema. As
opções de desempenho são contagens fixas: `MaxInspectorBuffers = 2`,
comentado como "a rather conservative value for low specced systems";
`ClutCacheSize` igual ao número de núcleos; e `RgbDenoiseThreadLimit`
([`options.cc` L513–520](https://github.com/Beep6581/RawTherapee/blob/5.13/rtgui/options.cc#L513-L520)).

**Paralelismo.** Os grupos de threads de prévias e de miniaturas usam
`omp_get_num_procs()`
([`previewloader.cc` L75–81](https://github.com/Beep6581/RawTherapee/blob/5.13/rtgui/previewloader.cc#L75-L81),
[`thumbimageupdater.cc` L82–87](https://github.com/Beep6581/RawTherapee/blob/5.13/rtgui/thumbimageupdater.cc#L82-L87)).
Inferência pelo código: a fila de lote processa uma imagem por vez, porque
guarda um único item `processing`
([`batchqueue.cc` L667–695](https://github.com/Beep6581/RawTherapee/blob/5.13/rtgui/batchqueue.cc#L667-L695)).
O limite é só por núcleos.

**Dupla contagem.** Não se aplica.

## digiKam

Versão lida: [`v9.1.0`](https://invent.kde.org/graphics/digikam/-/tree/v9.1.0)
no repositório oficial da KDE. O espelho no GitHub está parado desde 2019.

**Estimativa por imagem.** Não foi encontrada nos arquivos lidos.

**Margem e uso da RAM.** O Cache de imagens carregadas usa 6% da RAM total,
entre 100 e 1024 MB
([`loadingcache.cpp` L165–166](https://invent.kde.org/graphics/digikam/-/blob/v9.1.0/core/libs/threadimageio/fileio/loadingcache.cpp#L165-166)).

**Reação à pressão.** Não foi encontrada. No [bug 351658](https://bugs.kde.org/show_bug.cgi?id=351658),
as ferramentas de manutenção esgotavam a memória. O mantenedor explicou que
não havia verificação da memória disponível. A correção passou a processar os
itens por fila e restringiu o número de threads
([commit 2f8ddd42](https://commits.kde.org/digikam/2f8ddd42ef62d7aea9e490cdb05ffcc644810c81)).
O bug foi fechado em 2019.

**Paralelismo.** Com "usar vários núcleos", a manutenção usa
`QThread::idealThreadCount()`; sem a opção, usa 1 thread. O código guarda,
comentada, uma linha que usaria metade disso
([`maintenancethread.cpp` L57–68](https://invent.kde.org/graphics/digikam/-/blob/v9.1.0/core/utilities/maintenance/manager/maintenancethread.cpp#L57-68),
[`actionthreadbase.cpp` L124–127](https://invent.kde.org/graphics/digikam/-/blob/v9.1.0/core/libs/threads/actionthreadbase.cpp#L124-127)).
O limite é por núcleos, com uma opção do usuário.

**Dupla contagem.** Não se aplica.

## Adobe Photoshop e Lightroom Classic

São programas de código fechado. Só a documentação oficial foi consultada.

**Photoshop.** A [página de preferências de desempenho](https://helpx.adobe.com/photoshop/desktop/get-started/settings-and-preferences/configure-photoshop-performance-preferences.html)
(atualizada em 04/10/2026) diz:

- o padrão usa 70% da "Available RAM" mostrada no diálogo, com uma faixa ideal
  sugerida;
- não se deve passar de 85% da memória do sistema, para sobrar memória ao
  Windows e a outros programas;
- a mudança exige reiniciar o Photoshop;
- o ajuste deve ser feito em passos de 5%, acompanhando o indicador
  Efficiency;
- níveis de Cache e tamanho de bloco trocam velocidade por memória;
- o padrão é de 50 estados de histórico.

A [página de lentidão](https://helpx.adobe.com/photoshop/desktop/troubleshoot/performance-stability-issues/fix-lag-freeze-slow-performance.html)
diz que pouca memória alocada aumenta o uso do disco de rascunho (scratch
disk). A Adobe não explica como calcula a "Available RAM". Também não
documenta concorrência por imagem nem reação a sinais do sistema.

**Lightroom Classic.** A [página de otimização](https://helpx.adobe.com/lightroom-classic/desktop/technical-support/performance-guidelines/optimize-performance-lightroom.html)
(atualizada em 17/06/2025) recomenda 12 GB de RAM e fechar outros programas,
porque eles reduzem a memória disponível ao Lightroom. Ela trata do Cache do
Camera Raw em disco, de 5 GB por padrão. As
[preferências](https://helpx.adobe.com/lightroom-classic/desktop/introduction-to-lightroom-classic/setting-preferences-lightroom.html)
(atualizadas em 05/09/2024) têm a opção "Generate preview in parallel", que
gera várias prévias em paralelo em vez de uma por vez. A Adobe não documenta
quantas prévias, nem se a memória entra na decisão.

## Capture One

O único artigo oficial encontrado sobre recursos é o de
[aceleração por hardware](https://support.captureone.com/hc/en-us/articles/360002412798-What-does-Hardware-Acceleration-do-and-how-do-I-use-it-in-Capture-One).
Ele diz que a aceleração reserva memória para algumas ações, pede ao menos
1 GB de memória de GPU e recomenda 2 GB. Não foi encontrada fonte primária
sobre limites de RAM, concorrência de exportação ou reação à pressão. Os
relatos sobre exportação paralela são do fórum de usuários e não entram aqui.

## Chromium

Commit lido: [`70089924`](https://chromium.googlesource.com/chromium/src/+/70089924d5795fe0a24ae91e244b9aa878a4e076),
de 14/10/2026.

**Estimativa por imagem.** Não há estimativa para admitir uma decodificação.
Os Caches têm orçamentos em bytes:

- O conjunto de imagens decodificadas tem 128 MiB por padrão. No renderer,
  sobe para 256 MiB com 4 GiB ou mais de RAM total e cai para 32 MiB em
  dispositivo fraco
  ([`image_decode_cache_utils.h` L17](https://chromium.googlesource.com/chromium/src/+/70089924d5795fe0a24ae91e244b9aa878a4e076/cc/tiles/image_decode_cache_utils.h#17),
  [`.cc` L23–50](https://chromium.googlesource.com/chromium/src/+/70089924d5795fe0a24ae91e244b9aa878a4e076/cc/tiles/image_decode_cache_utils.cc#23)).
- A memória descartável tem 512 MiB no desktop, um oitavo disso em
  dispositivo fraco, e nunca passa de 25% da RAM total. No Linux, também fica
  limitada à metade do espaço livre do diretório de memória compartilhada
  ([`discardable_shared_memory_manager.cc` L169–215](https://chromium.googlesource.com/chromium/src/+/70089924d5795fe0a24ae91e244b9aa878a4e076/components/discardable_memory/service/discardable_shared_memory_manager.cc#169)).

**Margem e uso da RAM.** No Windows, o avaliador de pressão lê
`GlobalMemoryStatusEx` a cada 5 s
([`system_memory_pressure_evaluator_win.h` L29](https://chromium.googlesource.com/chromium/src/+/70089924d5795fe0a24ae91e244b9aa878a4e076/components/memory_pressure/system_memory_pressure_evaluator_win.h#29)).
Os limiares são fixos e não dependem da RAM total:

- Moderada: até 1000 MiB de RAM física livre (`ullAvailPhys`).
- Crítica: até 400 MiB.

O comentário diz que o Windows mantinha cerca de 300 MB disponíveis,
paginando até chegar lá, e que por isso o nível crítico fica perto desse valor
([L37–48](https://chromium.googlesource.com/chromium/src/+/70089924d5795fe0a24ae91e244b9aa878a4e076/components/memory_pressure/system_memory_pressure_evaluator_win.h#37)).

Há limiares por commit disponível (`ullAvailPageFile`), de 500 e 200 MiB, mas
ficam atrás de um recurso desligado por padrão. A avaliação também termina cedo
se houver mais de 1000 MiB livres e mais de 2 GiB de commit
([`system_memory_pressure_evaluator_win.cc` L33–77, L225–272](https://chromium.googlesource.com/chromium/src/+/70089924d5795fe0a24ae91e244b9aa878a4e076/components/memory_pressure/system_memory_pressure_evaluator_win.cc#225)).

**Reação à pressão.**

- A pressão moderada é notificada na transição e repetida a cada 10 s. A
  crítica é notificada sempre
  ([`.cc` L180–212](https://chromium.googlesource.com/chromium/src/+/70089924d5795fe0a24ae91e244b9aa878a4e076/components/memory_pressure/system_memory_pressure_evaluator_win.cc#180)).
- No nível moderado, liberam-se buffers baratos de recriar. No crítico, toda a
  memória possível
  ([`memory_pressure_level.h` L19–28](https://chromium.googlesource.com/chromium/src/+/70089924d5795fe0a24ae91e244b9aa878a4e076/base/memory/memory_pressure_level.h#19)).
- Ao liberar memória, a memória descartável reduz o próprio teto e descarta
  itens até caber nele
  ([`discardable_shared_memory_manager.cc` L690–704](https://chromium.googlesource.com/chromium/src/+/70089924d5795fe0a24ae91e244b9aa878a4e076/components/discardable_memory/service/discardable_shared_memory_manager.cc#690)).

Não foi verificado como os Caches de decodificação recebem esse sinal nesta
versão.

**Paralelismo.** O renderer usa metade dos processadores para rasterizar,
entre 1 e 4 threads
([`compositor_util.cc` L49–50, L402–434](https://chromium.googlesource.com/chromium/src/+/70089924d5795fe0a24ae91e244b9aa878a4e076/content/browser/gpu/compositor_util.cc#402)).
O limite é por núcleos. A memória muda o tamanho dos Caches, não o número de
threads.

**Dupla contagem.** Não ocorre. O avaliador lê só a RAM livre do sistema, e os
Caches comparam os próprios bytes a um teto. Os dois mecanismos não se somam.

## Windows

**Contadores.** [PERFORMANCE_INFORMATION](https://learn.microsoft.com/en-us/windows/win32/api/psapi/ns-psapi-performance_information)
define `PhysicalAvailable` como a memória reutilizável de imediato, somando as
listas standby, free e zero. Ao reservar páginas com `MEM_COMMIT`, o
`CommitTotal` muda na hora, mas a memória física só é cobrada quando as
páginas são acessadas. O `CommitLimit` é um limite flexível se o arquivo de
paginação puder crescer. [MEMORYSTATUSEX](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/ns-sysinfoapi-memorystatusex)
lembra que os valores refletem o momento da chamada.

**Sinais de pressão.** Os objetos de
[CreateMemoryResourceNotification](https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-creatememoryresourcenotification)
avisam todo o sistema quando a memória física está baixa ou alta. A Microsoft
sugere reduzir o working set no sinal de pouca memória e alocar mais no de
muita. Entre os dois, nenhum sinal fica ativo, e o aplicativo deve tentar
manter o uso constante.
[QueryMemoryResourceNotification](https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-querymemoryresourcenotification)
consulta o estado sem bloquear, "before proceeding with an operation". O
Learn não publica os limiares.

**Limiares numéricos.** Só aparecem num comentário da própria Microsoft, no
código de referência do .NET Framework, que reproduz o `initsup.c` do kernel:
cerca de 32 MB por 4 GB de RAM, com teto de 64 MB. Numa máquina de 4 GB, o
limiar é de cerca de 56 MB
([`PhysicalMemoryMonitor.cs` L41–78](https://github.com/microsoft/referencesource/blob/ec9fa9ae770d522a5b5f0607898044b7478574a3/System.Runtime.Caching/System/Caching/PhysicalMemoryMonitor.cs#L41-L78)).
É código interno de 2009, não um contrato público.

O mesmo arquivo mostra o Cache do .NET reduzindo itens pela porcentagem de
memória em uso (`dwMemoryLoad`). A pressão alta fica entre 95% e 99%, conforme
a RAM total, e a baixa fica 9 pontos abaixo
([L80–123](https://github.com/microsoft/referencesource/blob/ec9fa9ae770d522a5b5f0607898044b7478574a3/System.Runtime.Caching/System/Caching/PhysicalMemoryMonitor.cs#L80-L123)).

## Comparação

| Programa | Estimativa por imagem | Base do orçamento | Reação à pressão | Paralelismo limitado por | Junta RAM livre e reservas? |
| --- | --- | --- | --- | --- | --- |
| darktable | fator × pixels × bytes + custo fixo, por módulo | 12,5% a 68% da RAM total; piso de 512 MiB; ÷3 em miniaturas | blocos; GPU volta para CPU | RAM total e núcleos (4 ou 7 trabalhos) | não; não lê RAM livre |
| libvips | nenhuma; blocos sob demanda | Cache fixo de 100 MB; disco acima de 100 MB | descarte do próprio Cache; disco | núcleos | não; conta só o que aloca |
| ImageMagick | tamanho exato do Cache de pixels | RAM total (metade no Windows); política do usuário | memória → mapeado → disco → erro | núcleos − 1 | não; conta só o que reserva |
| RawTherapee | não encontrada | contagens fixas | não encontrada | núcleos; lote serial | não se aplica |
| digiKam | não encontrada | Cache de 6% da RAM total (100–1024 MB) | não encontrada | núcleos ou 1 thread | não se aplica |
| Photoshop | não documentada | 70% da "Available RAM"; máximo de 85% | disco de rascunho | não documentado | não documentado |
| Lightroom Classic | não documentada | não documentada | não documentada | opção liga ou desliga | não documentado |
| Capture One | não documentada | não documentada | não documentada | não documentado | não documentado |
| Chromium | nenhuma; orçamentos de Cache | 128–256 MiB; ≤25% da RAM total | sinais a 1000 e 400 MiB livres; descarte | núcleos ÷ 2, de 1 a 4 | não; mecanismos separados |
| MyAlbuns | 6 ou 16 B/px + 2 × arquivo + fixo | (RAM livre − margem) ÷ 2 e (commit livre − margem) ÷ 2; teto de ¼ da RAM total | espera; serial pelo commit; `MemoryPressure` | núcleos (8 vagas) e memória | sim |

## O que isso indica para o MyAlbuns

**Base do orçamento.** Prática comum, nas fontes lidas, é um orçamento fixo:
uma fração da RAM total, calculada ao iniciar e ajustável pelo usuário,
comparada com a contagem das próprias alocações. É o caso do darktable, do
ImageMagick, do Cache do digiKam, da memória descartável do Chromium e, pela
documentação, do Photoshop. Nenhuma fonte lida admite trabalho paralelo pela
leitura da RAM livre antes de cada imagem. A leitura de RAM livre aparece no
Chromium e no Cache do .NET, mas para reduzir Caches, não para segurar tarefas.
A regra do MyAlbuns, que decide cada trabalho paralelo pela RAM e pelo commit
livres, é incomum entre os programas lidos. Isso não a torna errada: o Windows
recomenda ajustar o consumo à memória disponível.

**A divisão por dois.** Nenhuma fonte usa metade da RAM livre como critério de
admissão. As frações encontradas incidem sobre a RAM total: 50% no nível
`default` do darktable, metade da RAM no ImageMagick para Windows e ¼ para a
memória descartável do Chromium. A única "metade do livre" encontrada é a do
Chromium no Linux, sobre espaço livre de memória compartilhada em disco. As
fontes não sustentam nem contestam o `/2`. A forma mais próxima das fontes é
uma margem fixa sobre a RAM livre, como os limiares do Chromium.

**A margem.** A margem do MyAlbuns num PC de 4 GB é de 512 MiB. O Chromium
considera pressão moderada a partir de 1000 MiB livres e crítica a partir de
400 MiB, sem depender da RAM total. Sem o `/2` e com a margem de 512 MiB, o
segundo Processador de Imagens começaria com 890 a 972 MiB livres, um pouco
abaixo do limiar moderado do Chromium. No pior momento medido, descontando os
dois Processadores de Imagens inteiros, sobrariam cerca de 680 MiB
([medição de 2026-10-13](2026-10-13-progresso-de-abertura-em-pc-fraco.md)).
Isso fica entre os limiares moderado e crítico do Chromium e muito acima do
limiar de pouca memória do kernel, de cerca de 56 MB em 4 GB pelo comentário
da Microsoft.

Inferência não verificada: o WebView2 do Host é baseado no Chromium. Se ele
rodar esse avaliador com os mesmos limiares, essa faixa pode fazer o editor
liberar Caches.

**Dupla contagem.** Os programas lidos usam um de dois modelos, nunca os dois
juntos:

- contar as próprias reservas contra um limite fixo (ImageMagick, libvips,
  memória descartável do Chromium, darktable);
- ler a RAM livre do sistema sem descontar reservas próprias (avaliador do
  Chromium, Cache do .NET).

A regra do MyAlbuns lê a RAM livre, que já exclui a memória residente do
Processador de Imagens em execução, e ainda desconta a reserva inteira dele.
Essa combinação não aparece em nenhuma fonte. A
[revisão da admissão](../design/0032-revisao-da-admissao-por-memoria.md) tem
um motivo para descontar a reserva: um trabalho admitido pode ainda não ter
alocado seus buffers. As páginas do Windows confirmam que o commit é cobrado
na hora e a residência só no acesso. As fontes não mostram como outros
programas tratam esse intervalo, porque nenhuma combina os dois modelos.

**Reação à pressão.** As reações comuns são:

- dividir em blocos (darktable);
- mover dados para disco (ImageMagick, libvips, disco de rascunho do
  Photoshop);
- descartar Caches (Chromium, libvips, .NET);
- trocar GPU por CPU (darktable).

A execução serial existe por projeto no lote do RawTherapee e como opção no
digiKam. Admitir um único trabalho pelo commit quando nada roda não tem
equivalente direto nas fontes. Lembra os pisos do darktable (512 MiB de
orçamento, 128 MiB por pipeline), que também garantem que algum trabalho
avance. O MyAlbuns reavalia a cada 100 ms. O Chromium consulta a cada 5 s, e
o Windows oferece objetos de notificação que dispensam a consulta.

**Paralelismo e estimativa.** A maioria limita o paralelismo só por núcleos:
libvips, ImageMagick, RawTherapee, digiKam e Chromium. O darktable é o único
que também usa a memória, pela RAM total, como faz o MyAlbuns em conjunto com
as 8 vagas. A estimativa do MyAlbuns tem a mesma forma da fórmula do
darktable, bytes por pixel mais um custo fixo, mas foi calibrada por medição,
com folga de cerca de 1,8 vez sobre o pico.

## Limites

- Adobe e Capture One são de código fechado. As páginas oficiais não descrevem
  o mecanismo interno, e "não documentado" não quer dizer que o mecanismo não
  exista. As datas de atualização são as indicadas em cada página. O artigo do
  Capture One tem cerca de três anos.
- O manual do darktable consultado é o da versão em desenvolvimento e diverge
  do código 5.6.2 nas porcentagens dos níveis. O texto segue o código.
- "Não encontrado" vale só para os arquivos lidos de cada projeto, não para o
  repositório inteiro.
- Os limiares numéricos do Windows vêm de um comentário interno de 2009 no
  código de referência do .NET Framework, não do Microsoft Learn. Versões
  atuais do Windows podem ser diferentes.
- Não foi verificado se o WebView2 usa o avaliador de pressão do Chromium com
  os limiares acima.
- Nenhum programa foi executado nem medido. As comparações numéricas com o
  MyAlbuns usam a [medição de 2026-10-13](2026-10-13-progresso-de-abertura-em-pc-fraco.md).
- Versões consultadas: darktable `release-5.6.2`, libvips `v8.18.7`,
  ImageMagick `7.1.2-33`, RawTherapee `5.13`, digiKam `v9.1.0`, Chromium
  `70089924`, .NET reference source `ec9fa9ae`.
- Acesso em 14/10/2026.
