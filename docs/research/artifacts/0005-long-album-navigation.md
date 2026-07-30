---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-30
updated: 2026-07-30
---

# Navegação em Álbum longo com imagens reais

Coletado em UTC: `2026-07-30T18:20:04.7535451Z`.
[JSON bruto](0005-long-album-navigation.json).

| Medida | A — hosts independentes | B — host multiwindow |
|---|---:|---:|
| Hosts do Projeto | 2 | 1 |
| Janelas do Projeto | 2 | 2 |
| Processos na árvore | 14 | 8 |
| Working set agregado | 2.627,1 MiB | 2.134,6 MiB |
| Memória privada agregada | 2.086,8 MiB | 1.760,5 MiB |
| Memória gráfica compartilhada | 656,1 MiB | 626,2 MiB |
| Duas Janelas identificadas | 1493 ms | 593 ms |
| Duas Janelas com Cache pronto | 35693 ms | 30337 ms |
| Dois Canvas com texturas prontos | 35954 ms | 30693 ms |
| Duração de parede do Cache frio | 32766 ms | 29170 ms |
| Fotos processadas pelo Cache | 172 | 172 |
| Vazão agregada dos originais | 42,8 MiB/s | 48,0 MiB/s |
| Representações reduzidas | 49,3 MiB | 49,3 MiB |
| Pan: pior p95 entre Projetos | 13.6 ms | 13.9 ms |
| Pan: frames acima de 33 ms | 0 | 0 |
| Zoom: pior p95 entre Projetos | 20.9 ms | 15.1 ms |
| Zoom: frames acima de 33 ms | 1 | 0 |
| Navegação: pior p95 entre Projetos | 246.1 ms | 262.9 ms |
| Navegação: respostas acima de 33 ms | 60 | 60 |
| Navegação: pico de Lâminas residentes | 7 | 7 |
| Navegação: pico de texturas residentes | 14 | 14 |
| Exportação: duração | 1399 ms | 1419 ms |
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

- Commit do código: `afd446c0d24934039aa25ca3ee02f0d9df184ebf`
- Build concluída em UTC: `2026-07-30T18:18:25.8193940Z`
- Perfil: `release`
- Árvore de trabalho tinha mudanças alheias: sim
- Entradas da build tinham mudanças: sim
- Arquivos de entrada: 167
- Digest das entradas: `7da1d0ce31db9a3703b3c204930e7e70d853030e88e3ce4f5556a233706df75b`
- Hash do host: `a8e37ed567aace4df23f1680ed1c970748608b3f1de4d8329a5076d9415a2b16`
- Hash do Processador de Imagens: `349f348881a0958d263b573ec426b159623b2637d282cdc50655737e3e8dfea5`
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
- A navegação percorreu 10 vezes a primeira, a 50ª, a 100ª e de volta à primeira Lâmina, aguardando a textura real do destino e o frame renderizado pelo PixiJS.
- As duas Janelas iniciaram o probe pelo mesmo arquivo-gate, somente depois da conclusão do Cache frio.
- A Exportação mediu a primeira Lâmina do Álbum principal a 300 DPI, lendo e verificando os JPEGs originais.
- Cada uso valida o tamanho e o SHA-256 da Foto; o corpus completo foi recalculado depois das duas alternativas.
- A memória inclui o host e todos os processos descendentes observados.
- A Exportação foi liberada por um segundo gate somente depois dos dois probes de Canvas.
- Os dois hosts independentes foram iniciados antes da espera pelas Janelas, usando o mesmo marco inicial da alternativa multiwindow.
- A queda só é forçada depois de validar o caminho do executável do PID alvo.
