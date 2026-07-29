---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-29
updated: 2026-07-29
---

# Interações e Exportação com imagens reais

Coletado em UTC: `2026-07-29T20:53:54.9209603Z`.
[JSON bruto](0003-topology-interaction-export.json).

| Medida | A — hosts independentes | B — host multiwindow |
|---|---:|---:|
| Hosts do Projeto | 2 | 1 |
| Janelas do Projeto | 2 | 2 |
| Processos na árvore | 9 | 8 |
| Working set agregado | 1.505,7 MiB | 1.437,8 MiB |
| Memória privada agregada | 1.098,2 MiB | 1.078,3 MiB |
| Memória gráfica compartilhada | 644,6 MiB | 640,9 MiB |
| Primeiro host de A identificado | 1504 ms | não se aplica |
| Duas Janelas identificadas | 1774 ms | 327 ms |
| Duas Janelas com Cache pronto | 25409 ms | 24219 ms |
| Duração de parede do Cache frio | 23714 ms | 23572 ms |
| Fotos processadas pelo Cache | 172 | 172 |
| Vazão agregada dos originais | 59,1 MiB/s | 59,4 MiB/s |
| Representações reduzidas | 49,3 MiB | 49,3 MiB |
| Pan: pior p95 entre Projetos | 7.9 ms | 8 ms |
| Pan: frames acima de 33 ms | 0 | 0 |
| Zoom: pior p95 entre Projetos | 8 ms | 8 ms |
| Zoom: frames acima de 33 ms | 0 | 0 |
| Exportação: duração | 1113 ms | 1130 ms |
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

- Commit do código: `2a6559ce8b9ce180df61d1831d3650e1ca15e0ea`
- Build concluída em UTC: `2026-07-29T20:52:47.0890476Z`
- Perfil: `release`
- Árvore de trabalho tinha mudanças alheias: sim
- Entradas da build tinham mudanças: não
- Arquivos de entrada: 141
- Digest das entradas: `67b8154bfc49087f6c893aa4fa7c7e2111ad520cfaef472f41a0a33843d956f0`
- Hash do host: `da607cf69f4420d1fbeb9a4412ff93fcba7030bb607d2402777b64706b6d3c50`
- Hash do Processador de Imagens: `0c841813100914ad69f8cbd811c81dd08f6114e7065ba3e90be48d270cd64dbd`
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
- A queda só é forçada depois de validar o caminho do executável do PID alvo.
- Os hosts independentes são iniciados em sequência depois que uma tentativa simultânea deixou intermitentemente um host sem Janela visível.
