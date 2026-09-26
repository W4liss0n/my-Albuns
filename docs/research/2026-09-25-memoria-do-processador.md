---
status: current
document: research
date: 2026-09-25
platform: windows-11-x64
---

# Memória do Processador por formato

A admissão reservava 16 bytes por pixel, duas vezes o tamanho comprimido e
64 MiB para cada Original, cerca de 450 MB para uma foto de 24 MP. O pico real
do Processador foi medido por formato para calibrar essa estimativa sem
arriscar falta de memória.

## Resultado

Pico de memória comprometida de um processo que prepara uma prévia, com fotos
de 24 MP (6000 × 4000) geradas a partir da mesma foto real:

| Formato | Arquivo | Pico | Bytes por pixel | Estimativa anterior |
| --- | ---: | ---: | ---: | ---: |
| JPEG sequencial 4:2:0 | 5,4 MB | 98 MB | 4,3 | 441 MB |
| JPEG sequencial 4:4:4 | 6,4 MB | 98 MB | 4,3 | 443 MB |
| JPEG em tons de cinza | 4,9 MB | 220 MB | 9,6 | 440 MB |
| JPEG progressivo 4:2:0 | 5,0 MB | 220 MB | 9,6 | 440 MB |
| JPEG progressivo 4:4:4 | 6,1 MB | 220 MB | 9,6 | 442 MB |
| PNG RGB 8 bits | 30,0 MB | 220 MB | 9,6 | 490 MB |
| PNG RGBA 8 bits | 33,4 MB | 220 MB | 9,6 | 497 MB |
| PNG cinza 16 bits | 14,9 MB | 220 MB | 9,6 | 460 MB |
| PNG RGB 16 bits | 69,7 MB | 235 MB | 10,3 | 570 MB |
| PNG RGBA 16 bits | 79,1 MB | 281 MB | 12,3 | 588 MB |
| TIFF RGB 8 bits | 62,9 MB | 220 MB | 9,6 | 556 MB |
| TIFF RGBA 8 bits | 69,1 MB | 258 MB | 11,3 | 568 MB |
| TIFF RGB 16 bits | 137,3 MB | 418 MB | 18,3 | 705 MB |
| TIFF RGBA 16 bits | 183,1 MB | 556 MB | 24,3 | 796 MB |

As 155 fotos JPEG do corpus, entre 6 e 24 MP, confirmaram o primeiro caso:
cerca de 20 MiB fixos e 3,4 bytes por pixel acima deles.

O JPEG sequencial colorido é o único formato que o Processador decodifica
direto para RGB antes de reduzir; os demais passam pela normalização completa.
A estimativa passou a separar esse caso: 6 bytes por pixel, duas vezes o
tamanho comprimido e 32 MiB, cerca de 187 MB para 24 MP, o dobro do pico
medido. Os demais formatos mantêm a estimativa anterior, cuja folga sobre o
pico medido fica entre 1,4 e 2,4 vezes. O cabeçalho decide o caso; um arquivo
que não possa ser lido com segurança fica na estimativa geral.

## Efeito

Importação das 172 fotos do corpus pelo fluxo real, com a memória física
disponível simulada em 4,3 GB, como estava a máquina nas aberturas registradas
em 2026-09-24:

| Estimativa | Trabalhos simultâneos | Tempo |
| --- | ---: | ---: |
| Anterior | 2 | 31,6–32,7 s |
| Calibrada | 5 | 20,7–21,3 s |

Com memória sobrando, a admissão não limita e o tempo não muda: a abertura sem
Cache levou de 19,6 a 22,0 s nas duas versões, com média de 4,5 trabalhos
simultâneos. Nesse caso, o limite passa a ser a disputa por disco e CPU entre
os oito trabalhos.

## Método

- Windows 11, Intel Core i5-13450HX, 16 processadores lógicos, 24 GB de RAM.
- O Processador passou a registrar no evento `imaging_process_stopped` os
  picos de memória física (`peak_working_set_bytes`) e comprometida
  (`peak_commit_bytes`), lidos do próprio processo ao terminar.
- Cada formato foi importado sozinho pelo teste
  `photo_import::native_flow_tests::real_import_flow` com o Processador real;
  o pico vem do único processo daquela importação. As variantes foram geradas
  com PIL e, para 16 bits por canal, com o crate `image`.
- O JPEG CMYK ficou de fora: o Processador o recusa, e a importação usa a
  inspeção alternativa do Host.
- A comparação sob memória limitada alternou as duas estimativas na mesma
  sessão, duas rodadas cada.
