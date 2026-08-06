---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-29
updated: 2026-07-29
---

# Interações e Exportação com imagens reais

Coletado em UTC: `2026-07-29T22:01:09.7020294Z`.
[JSON bruto](0003-topology-interaction-export.json).

| Medida | A — hosts independentes | B — host multiwindow |
|---|---:|---:|
| Hosts do Projeto | 2 | 1 |
| Janelas do Projeto | 2 | 2 |
| Processos na árvore | 14 | 8 |
| Working set agregado | 1.795,9 MiB | 1.450,9 MiB |
| Memória privada agregada | 1.250,1 MiB | 1.075,7 MiB |
| Memória gráfica compartilhada | 651,8 MiB | 625,4 MiB |
| Duas Janelas identificadas | 1311 ms | 383 ms |
| Duas Janelas com Cache pronto | 26120 ms | 24500 ms |
| Dois Canvas com texturas prontos | 26396 ms | 24725 ms |
| Duração de parede do Cache frio | 24118 ms | 23546 ms |
| Fotos processadas pelo Cache | 172 | 172 |
| Vazão agregada dos originais | 58,1 MiB/s | 59,5 MiB/s |
| Representações reduzidas | 49,3 MiB | 49,3 MiB |
| Pan: pior p95 entre Projetos | 18 ms | 12.8 ms |
| Pan: frames acima de 33 ms | 0 | 0 |
| Zoom: pior p95 entre Projetos | 11.7 ms | 11.9 ms |
| Zoom: frames acima de 33 ms | 0 | 0 |
| Exportação: duração | 1299 ms | 1241 ms |
| Exportação: dimensões a 300 DPI | 7087 x 3543 px | 7087 x 3543 px |
| Exportação: volume dos originais | 22,6 MiB | 22,6 MiB |
| Exportação: tamanho do PNG | 27,0 MiB | 27,0 MiB |
| Janelas depois da queda forçada | 1 (outra Janela preservada) | 0 |

## Corpus real

- Álbuns: 2
- Fotos JPEG: 172
- Volume dos originais: 1.401,0 MiB
- Digest do corpus: `4da593ff7d12f861a8b3aba74249726e70131468a68af7a7d06200f5340871cc`
- Integridade antes/depois: confirmada por SHA-256
- Política da representação: uma prévia JPEG por Foto, com aresta máxima de 1600 px

## Build medida

- Commit do código: `8e8fe84a6d7e78a01b931b8b22853676c61870ce`
- Build concluída em UTC: `2026-07-29T21:59:57.4874616Z`
- Perfil: `release`
- Árvore de trabalho tinha mudanças alheias: sim
- Entradas da build tinham mudanças: não
- Arquivos de entrada: 142
- Digest das entradas: `5aeba9ff6a8d3f644a989d6cdb20377111840b08e49c0a900348ec9c7a703ad6`
- Hash do host: `73b6f6e01c7e3c064193435a4d2ac5058c3261093e8dadeafb315c5bd0c84e60`
- Hash do Processador de Imagens: `80bc0781ca59dabb5e016c5a0b671de4beadbddbe8aff0bf1c2f17367255f99f`
- Checkout atual corresponde ao manifesto: sim

## Ambiente registrado

- Sistema: Microsoft Windows 11 Pro `10.0.26200`
- Processador: 13th Gen Intel(R) Core(TM) i5-13450HX
- Memória física: 24.260,7 MiB

## Campos ainda não medidos

- recuperação persistida
- complexidade operacional da IPC

## Observações

- O Cache foi reconstruído a frio com uma representação JPEG de até 1600 px por Foto.
- Pan e Zoom foram medidos separadamente sobre uma textura real do Cache, depois de 24 frames de aquecimento.
- As duas Janelas iniciaram o probe pelo mesmo arquivo-gate, somente depois da conclusão do Cache frio.
- A Exportação mediu a primeira Lâmina do Álbum principal a 300 DPI, lendo e verificando os JPEGs originais.
- Cada uso valida o tamanho e o SHA-256 da Foto; o corpus completo foi recalculado depois das duas alternativas.
- A memória inclui o host e todos os processos descendentes observados.
- A Exportação foi liberada por um segundo gate somente depois dos dois probes de Canvas.
- Os dois hosts independentes foram iniciados antes da espera pelas Janelas, usando o mesmo marco inicial da alternativa multiwindow.
- A queda só é forçada depois de validar o caminho do executável do PID alvo.
